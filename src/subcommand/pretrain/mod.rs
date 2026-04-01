use std::{ffi::OsStr, fs::File, path::PathBuf, time::Instant};

use clap::Args;
use futures::{StreamExt, stream};
use rig::{
    agent::Agent,
    client::{Client, CompletionClient, Nothing},
    completion::Chat,
    providers::ollama::{self, OllamaExt},
};
use serde_json::json;

use crate::{
    models::{Dataset, ValidationEntry},
    utils::find_first_float,
};

#[derive(Args)]
/// Test the pretrain model of cypher generation against a dataset
pub(crate) struct SubArgs {
    /// Path to the dataset of testing
    #[arg(short, long)]
    dataset: PathBuf,
    /// Path to the result folder
    #[arg(short, long)]
    output: PathBuf,
    /// Number of parallel tests
    #[arg(short, long)]
    threads: usize,
    /// Number of entries to skip
    #[arg(short, long, default_value_t = 0)]
    skip: usize,
    /// Validator model
    #[arg(short, long, default_value = "ministral-3:3b")]
    validator: String,
    /// Generator model
    #[arg(short, long, default_value = "text-to-cypher-gemma3-2025:4b")]
    generator: String,
}

pub(crate) async fn run(args: SubArgs) -> anyhow::Result<()> {
    let prefix: &OsStr =
        args.dataset.file_stem().expect("The dataset is not a file");

    let client: Client<OllamaExt> = ollama::Client::new(Nothing)?;
    let validator: Agent<_> = client
        .agent(args.validator)
        .temperature(0.0)
        .additional_params(json!({
            "num_ctx": 4096,
            "num_thread": 16,
        }))
        .preamble("\
**Role:** Your role is to compare a **Generated Cypher Query** against a **Ground Truth Cypher Query** based on a provided **Graph Schema**.

**Evaluation Criteria:**

1.  **Structural & Subgraph Equivalence (40%):** * Does the generated query extract the same subgraph or a valid subset of it?
    * **Subset Validation:** If the generated query is more restrictive but logically sound (e.g., more specific property filters), it should be penalized less than a query with wrong directions or labels.
2.  **Constraint & Result Set Alignment (40%):** * **Intersection:** Do the results of the generated query form a subset of the Ground Truth? 
    * **Over-filtering:** If the generated query adds extra `WHERE` clauses not present in the Ground Truth (reducing the result set), it is a \"Partial Match.\"
    * **Under-filtering:** If it misses filters (returning more data than requested), it is a \"Low Accuracy\" match.
3.  **Executable Integrity (20%):** Is it syntactically correct and consistent with the Schema?

**Scoring Scale:**
* **1.0:** Perfect semantic and structural match.
* **0.8:** **Subset Match.** The query is more restrictive than the Ground Truth but every returned record is guaranteed to be a valid result of the Ground Truth.
* **0.5 - 0.7:** Minor logic differences or missing non-critical relationships.
* **0.3 - 0.4:** **Superset/Over-inclusive.** The query returns the correct data plus significant \"noise\" or unrelated nodes.
* **0.1 - 0.2:** Wrong logic, directions, or labels.
* **0.0:** Hallucination or syntax error.

Output Instructions: Return exclusively the numerical score as a decimal (e.g., 0.8). No explanation, no justification, no additional text.\
")
        .build();

    let generator: Agent<_> = client
        .agent(args.generator)
        .temperature(0.0)
        .additional_params(json!({
            "num_ctx": 4096,
            "num_thread": 16,
        }))
        .preamble("\
You are a specialized Neo4j Cypher generator. Your sole purpose is to translate a natural language question into a precise Cypher query based **only** on the provided Graph Schema.

**STRICT OPERATIONAL RULES:**
1. **Schema Obsession:** Use ONLY the labels, relationship types, and properties defined in the provided Schema. If a label like `Foo` is not in the schema, you must use the correct one.
2. **Path-Based Connectivity:** You must represent the answer as a set of connected paths. Use path variables (e.g., `p1 = (a)-[r]->(b)`).
3. **Mandatory Return:** Your `RETURN` clause must ONLY contain the path variables (e.g., `RETURN p1, p2, p3`). Never return isolated properties, strings, or aliases like `AS Influence`.
4. **Literal Extraction:** Extract properties directly from the user's question and map them to the correct schema properties (e.g., `foo: 'Jhon Doe'`).
5. **No Hallucinations:** Do not invent relationships like `[:ConnectsTo]` or `[:DealsWith]` if they are not in the schema.
6. **Syntax Integrity:** Ensure the query is valid Cypher. No `keys()`, no `{param}` legacy syntax, and no semicolons.

**FORMATTING:**
- Output ONLY the raw Cypher code.
- No markdown blocks (```).
- No explanations.\
")
        .build();

    let dataset: Dataset = serde_json::from_reader(File::open(&args.dataset)?)?;

    stream::iter(dataset.into_iter().enumerate().skip(args.skip))
        .for_each_concurrent(args.threads, async |(k, response)| {
            loop {
                let start: Instant = Instant::now();
                let message = format!(
                    "{}\n\nQuestion:\n{}",
                    response.schema, response.question
                );

                let Ok(cypher) = generator.chat(
                    message,
                    vec![]
                ).await else {
                    eprintln!("Error on cypher generation: {}", k);
                    continue;
                };

                let prompt_content = format!(
                    "Schema: {}\nGround Truth: {}\nGenerated: {}\nScore from 0 to 1:",
                    response.schema,
                    response.cypher,
                    cypher
                );

                let score: String = match validator.chat(prompt_content, vec![]).await {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Error on metric generation: {k}\n{e}");
                        continue;
                    }
                };

                let score: f32 = find_first_float(&score)
                    .map(|s| if !(0.0..=1.0).contains(&s) { f32::NAN } else { s })
                    .unwrap_or_else(|| {
                        eprintln!("Error on metric parsing: {k}\n{score}");
                        f32::NAN
                    });

                let validation: ValidationEntry = ValidationEntry {
                    score,
                    ground: response.clone(),
                    cypher,
                };

                let mut path: PathBuf = args.output.clone();
                path.push(format!("{}-{k:08}.json", prefix.to_string_lossy()));

                let Ok(file) = File::create(path) else {
                    eprintln!("Error on creating the result file {}", k);
                    continue;
                };

                let Ok(_) = serde_json::to_writer_pretty(file, &validation) else {
                    eprintln!("Error on saving the result into the file {}", k);
                    continue;
                };

                let stop: Instant = Instant::now();

                println!(
                    "Finish {k} in {:#?} ({score})", 
                    stop.duration_since(start)
                );

                break;
            };
        })
        .await;

    Ok(())
}

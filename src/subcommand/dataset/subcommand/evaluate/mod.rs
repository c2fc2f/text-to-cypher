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
    models::{Dataset, EvaluationDatasetEntry},
    utils::find_first_float,
};

#[derive(Args)]
/// Evaluate the dataset to verify the correctness of generated Cypher against
/// the schema and natural language
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
    /// Validator model
    #[arg(short, long, default_value = "deepseek-r1:1.5b")]
    validator: String,
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
**ROLE**
You are a strict, uncompromising GraphRAG Validation Agent. Your sole purpose is to ruthlessly evaluate the validity and alignment of a generated Cypher query against a provided Graph Schema and a Natural Language (NL) Query.

**EVALUATION RULES (Strict Adherence Required)**
You must analyze the inputs against the following absolute constraints:

1. **Schema Adherence (No Hallucinations):** Every node label, relationship type, and property key used in the Cypher query MUST exist exactly as written in the provided Graph Schema.
2. **Sub-Graph Return Mandate:** The Cypher query MUST return graph elements (Nodes, Relationships, or Paths). It is strictly FORBIDDEN to return isolated scalar values, arrays of primitives, or extracted properties.
    * *Valid:* `RETURN n`, `RETURN n, r, m`, `RETURN path`
    * *Invalid (Automatic Failure):* `RETURN n.name`, `RETURN count(n)`
3. **Literal Value Grounding:** Any hardcoded literal value used for matching, filtering, or assigning in the Cypher query (e.g., `(n:Person {name: \"Alice\"})` or `WHERE n.age > 30`) MUST be explicitly mentioned or unambiguously implied in the NL Query. The Cypher query cannot invent or assume filter values.
4. **Intent Alignment:** The structural traversal and logic of the Cypher query must perfectly resolve the intent of the NL Query without introducing extraneous or missing operations.

**SCORING RUBRIC**
Assign a Confidence Score from `0.0` to `1.0`:
* **1.0:** Flawless. Returns a sub-graph, perfect schema match, all literal values are fully grounded in the NL Query, and the logic perfectly matches the user's intent.
* **0.1 - 0.9:** The query follows Rules 1, 2, and 3, but suffers from logic misalignments, suboptimal traversal paths, or partial intent resolution. Score lower for larger logical gaps.
* **0.0:** Automatic critical failure. Assign a 0.0 immediately if:
  - The query returns a scalar, property, or aggregation instead of a sub-graph.
  - A label, relationship, or property is used that does not exist in the Schema.
  - A matching literal in the Cypher query is missing from the NL Query.

**OUTPUT INSTRUCTIONS**
Return EXCLUSIVELY the numerical score as a single decimal (e.g., `1.0`, `0.0`, `0.7`). 
Do not output explanations, justifications, markdown formatting, or any additional text.\
")
        .build();

    let dataset: Dataset = serde_json::from_reader(File::open(&args.dataset)?)?;

    stream::iter(dataset.into_iter().enumerate())
        .for_each_concurrent(args.threads, async |(k, response)| {
            loop {
                let start: Instant = Instant::now();

                let prompt_content: String = format!(
                    "Graph Schema: {}\nCypher Query: {}\nNL Query{}\nScore from 0 to 1:",
                    response.schema, response.cypher, response.question,
                );

                let score: String =
                    match validator.chat(prompt_content, vec![]).await {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("Error on metric: {k}\n{e}");
                            continue;
                        }
                    };

                let score: f32 = find_first_float(&score)
                    .map(|s| if !(0.0..=1.0).contains(&s) { f32::NAN } else { s })
                    .unwrap_or_else(|| {
                        eprintln!("Error on metric parsing: {k}\n{score}");
                        f32::NAN
                    });

                let evaluation: EvaluationDatasetEntry = EvaluationDatasetEntry {
                    score
                };

                let mut path: PathBuf = args.output.clone();
                path.push(format!("{}-{k:08}.json", prefix.to_string_lossy()));

                let Ok(file) = File::create(path) else {
                    eprintln!("Error on creating the result file {}", k);
                    continue;
                };

                let Ok(_) = serde_json::to_writer_pretty(file, &evaluation) else {
                    eprintln!("Error on saving the result into the file {}", k);
                    continue;
                };

                let stop: Instant = Instant::now();

                println!(
                    "Finish {k} in {:#?} ({score})", 
                    stop.duration_since(start)
                );

                break;
            }
        })
        .await;

    Ok(())
}

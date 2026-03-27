use std::{fs::File, io::Seek, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Args;
use futures::{StreamExt, stream};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use rand::{rng, rngs::ThreadRng, seq::IndexedRandom};
use rig::{
    OneOrMany,
    agent::Agent,
    client::{Client, CompletionClient, Nothing},
    completion::Chat,
    message::{Message, UserContent},
    providers::ollama::{self, OllamaExt},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::subcommand::Response;

#[derive(Args)]
/// Generate a dataset of natural language queries and their Cypher requests
pub(crate) struct SubArgs {
    /// Path to the graph schema set
    #[arg(short, long)]
    schemas: PathBuf,
    /// Path to the result file
    #[arg(short, long)]
    result: PathBuf,
    /// Number of data entries to generate
    #[arg(short, long)]
    count: u64,
    /// Number of parallel generations
    #[arg(short, long)]
    thread: usize,
    /// Generator model
    #[arg(short, long, default_value = "ministral-3:3b")]
    generator: String,
    /// Temperature of the model
    #[arg(short, long, default_value_t = 0.5)]
    temperature: f64,
}

#[derive(Deserialize)]
struct Schema {
    schema: String,
}

struct Saver {
    data: Vec<Response>,
    file: File,
}

impl Saver {
    fn save(&mut self) -> anyhow::Result<()> {
        self.file.rewind()?;
        serde_json::to_writer_pretty(&mut self.file, &self.data)?;
        Ok(())
    }
}

pub(crate) async fn run(args: SubArgs) -> Result<()> {
    let schemas: Vec<Schema> =
        serde_json::from_reader(File::open(args.schemas)?)?;
    let client: Client<OllamaExt> = ollama::Client::new(Nothing)?;
    let saver: Arc<Mutex<Saver>> = Arc::new(Mutex::new(Saver {
        data: vec![],
        file: File::create(args.result)?,
    }));

    if schemas.is_empty() {
        return Err(anyhow::anyhow!("No given schema"));
    }

    let oss: Agent<_> = client
        .agent(args.generator)
        .temperature(args.temperature)
        .additional_params(json!({
            "num_ctx": 3072,
        }))
        .without_preamble()
        .build();

    let pb: ProgressBar = ProgressBar::new(args.count);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({duration})")
        .unwrap()
        .progress_chars("#>-"));

    pb.set_draw_target(ProgressDrawTarget::stdout());
    pb.enable_steady_tick(Duration::from_millis(1000));

    stream::iter(0..args.count).for_each_concurrent(args.thread, |_| async {
        let saver = Arc::clone(&saver);
        let mut rng: ThreadRng = rng();

        let schema: &str = &schemas.choose(&mut rng).unwrap().schema;
        let Ok(cypher) = oss
        .chat(
            Message::User {
                content: OneOrMany::one(UserContent::text(
                    schema
                )),
            },
            vec![
                Message::system("\
You are a Neo4j Cypher expert. Your task is to generate EXACTLY ONE Cypher query that extracts a multi-branched, cohesive subgraph based **STRICTLY** on the provided Graph Schema.

**SCHEMA ADHERENCE (CRITICAL):**
1. **NO HALLUCINATION:** Use ONLY the node labels and relationship types defined in the provided Schema.
2. **PROPERTY MATCHING:** Use only the properties (and correct data types) listed in the Schema for each label.
3. **LOGICAL PATHS:** Only connect nodes using relationships that are valid according to the Schema's structure.

**STRICT GRAPH ARCHITECTURE:**
1. **Multi-Path Subgraph:** Do NOT return a single linear chain. You must use multiple path variables (e.g., `p1`, `p2`) that intersect at a central node to form a \"star\" or \"tree\" structure.
2. **Intersection:** Ensure all paths share at least one common node variable so the result is a single connected component.
3. **Mandatory Return:** You MUST return all path variables (e.g., `RETURN p1, p2, p3`).

**STRICT TECHNICAL RULES:**
1. **NO PARAMETERS:** Use only hardcoded, realistic literal values (e.g., 'Project Alpha', '2024-01-10'). Never use `$param` or `{param}`.
2. **NO MARKDOWN:** Do not use backticks (```). Start your response directly with the word `MATCH`.
3. **NO PROSE:** No explanations, no introduction. Only raw Cypher code.

**SEMANTIC GOAL:**
The query should represent a \"360-degree view\" of an entity.
*Example logic: If the Schema allows, link a Person to a Document, that same Person to a Lab, and the Document to a Topic.*

**Expected Output Structure:**
MATCH
    p1 = (a:LabelFromSchema {prop: 'val'})-[r1:REL_FROM_SCHEMA]->(b:LabelFromSchema),
    p2 = (a)-[r2:REL_FROM_SCHEMA]->(c:LabelFromSchema),
    p3 = (b)-[r3:REL_FROM_SCHEMA]->(d:LabelFromSchema)
RETURN p1, p2, p3\
")
            ],
        )
        .await else {
            return
        };

        let Ok(nl)  = oss
        .chat(
            Message::User {
                content: OneOrMany::one(UserContent::text(
                    format!("Schema:\n{schema}\n\nCypher:\n{cypher}")
                )),
            },
            vec![
                Message::system("\
You are a professional Strategic Analyst. Your goal is to transform a Cypher query into a **single, realistic human question**. 

**The Core Shift:** Stop asking \"for details\" or \"to provide an overview\". Instead, ask questions that imply a **need to understand a relationship, an impact, or a context**.

**Strict Rules for Question Phrasing:**
1. NO MARKDOWN: Never use bold (**), italics (*), or underscores (_). Output only plain text.
2. **NO \"CAN YOU PROVIDE/SHOW ME\":** Forbidden phrases include \"Can you provide...\", \"Show me...\", \"Give me a list...\", \"I want to see...\".
3. **USER INTENT FIRST:** Start the question with \"What...\", \"How...\", \"Who...\", or \"In the context of...\".
4. **NATURAL SCENARIOS:** Imagine the user is a manager or a researcher.
    * *Weak:* \"Can you provide the lab and reports for Dr. Thorne?\"
    * *Strong:* \"Which research labs is Dr. Thorne affiliated with, and what specific breakthroughs has he published recently?\"
5. **INTEGRATE LITERALS SEAMLESSLY:** Use the names and values from the query (e.g., 'Nature', 'CUST123') as if they are part of a conversation.
6. **DO NOT EXPLAIN THE GRAPH:** Never mention \"IDs\", \"Tags\", \"Labels\", or \"Paths\".
7. **ONE SENTENCE PREFERRED:** Keep it punchy and professional.

**Example Transformations:**
* *Query:* `(Author {name:'Aris'})-[:WROTE]->(Paper {title:'AI'})-[:MENTIONS]->(Tech {name:'Quantum'})`
* *Bad:* \"Can you provide the paper by Aris and the technology it mentions?\"
* *Good:* \"What specific quantum technologies are discussed in Aris's latest AI research papers?\"

* *Query:* `(Bank {name:'SBI'})-[:HAS_ACCOUNT]->(c:Customer {id:'123'})-[:HAS_LOAN]->(l:Loan)`
* *Bad:* \"Show me the customer 123 at SBI and their loans.\"
* *Good:* \"Does customer 123 hold any active loans at the State Bank of India?\"\
")
            ],
        )
        .await else {
        return
        };

        let mut s = saver.lock().await;
        s.data.push(Response {
            schema: schema.to_string(),
            cypher,
            question: nl
        });
        let Ok(()) = s.save() else {
            return
        };
        pb.inc(1);
    }).await;

    Ok(())
}

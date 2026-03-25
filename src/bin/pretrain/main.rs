use anyhow::Result;
use mistralrs::{
    ChatCompletionResponse, IsqType, Model, ModelDType, TextMessageRole,
    TextMessages, VisionModelBuilder,
};

#[tokio::main]
async fn main() -> Result<()> {
    let model: Model = VisionModelBuilder::new(
        "neo4j/text-to-cypher-Gemma-3-4B-Instruct-2025.04.0",
    )
    .with_isq(IsqType::Q4K)
    .with_dtype(ModelDType::F32)
    .with_logging()
    .build()
    .await?;

    let messages: TextMessages = TextMessages::new()
        .add_message(
            TextMessageRole::System,
            r"\
You are a text-to-Cypher assistant.
RULES:
    - NEVER use property exact match like {title: '...'}.
    - ALWAYS use elementId(node) from the context to identify nodes.
    - Output ONLY the Cypher query, nothing else.\
",
        )
        .add_message(
        TextMessageRole::User,
        "\
Node properties:

Relationship properties:

The relationships:
(:Person)-[:DIRECTED]->(:Movie)

Context:
The node representing 'Inception' has elementId(node) = \"4:abc123:1\"

Question:
Find all directors of Inception\
",
        )
        .add_message(
        TextMessageRole::Assistant,
        "\
MATCH (m:Movie) WHERE elementId(m) = \"4:abc123:1\"
MATCH (p:Person)-[:DIRECTED]->(m)
RETURN p.name\
",
        )
        .add_message(
            TextMessageRole::User,
            "\
Node properties:
    - **Movie** 
    - `title` : STRING
    - `released` : INTEGER
    - `tagline` : STRING
    - **Person**
    - `name` : STRING
    - `born` : INTEGER
Relationship properties:
    - **ACTED_IN**
    - `roles` : LIST
    - **REVIEWED**
    - `summary` : STRING
    - `rating` : INTEGER
The relationships:
    (:Person)-[:ACTED_IN]->(:Movie)
    (:Person)-[:DIRECTED]->(:Movie)
    (:Person)-[:REVIEWED]->(:Movie)

Context:
The node representing the `Jhon Doe` concept has elementId(node) = \"4:187a07c3-0244-4a70-a90b-4dac8af66244:0\"

Question:
Find all actors who played with Jhon Doe\
",
        );

    let response: ChatCompletionResponse =
        model.send_chat_request(messages).await?;

    println!(
        "{}",
        response.choices[0]
            .message
            .content
            .as_deref()
            .unwrap_or("<empty>")
    );

    eprintln!(
        "prompt tok/s: {:.1}, completion tok/s: {:.1}",
        response.usage.avg_prompt_tok_per_sec,
        response.usage.avg_compl_tok_per_sec,
    );

    Ok(())
}

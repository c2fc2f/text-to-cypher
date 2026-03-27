pub mod dataset_gen;
pub mod pretrain;
pub mod stats;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    pub schema: String,
    pub cypher: String,
    pub question: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub score: f32,
    pub ground: Response,
    pub cypher: String,
}

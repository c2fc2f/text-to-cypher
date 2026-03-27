use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatasetEntry {
    pub schema: String,
    pub cypher: String,
    pub question: String,
}

pub type Dataset = Vec<DatasetEntry>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationEntry {
    pub score: f32,
    pub ground: DatasetEntry,
    pub cypher: String,
}

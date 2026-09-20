use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub alias: String,
    pub host: String,
    pub port: u16,
    pub service_name: String,
    pub username: String,
    pub password: String,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            alias: "FREEPDB1".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1521,
            service_name: "FREEPDB1".to_string(),
            username: "SCOTT".to_string(),
            password: "tiger".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub position: i32,
    pub operation: String,
    pub options: Option<String>,
    pub object_name: Option<String>,
    pub starts: u64,
    pub e_rows: u64,
    pub a_rows: u64,
    pub a_time_ms: f64,
    pub buffers: u64,
    pub reads: u64,
    pub cost: Option<i64>,
    pub access_predicates: Option<String>,
    pub filter_predicates: Option<String>,
    pub cardinality_ratio: f64,
    pub buffer_percentage: f64,
    pub is_bottleneck: bool,
    pub bottleneck_tags: Vec<String>,
    pub children: Vec<PlanNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub elapsed_ms: f64,
    pub row_count: usize,
    pub sql_id: Option<String>,
    pub child_number: Option<i32>,
    pub plan_hash_value: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SampleQuery {
    pub title: String,
    #[allow(dead_code)]
    pub description: String,
    pub sql: String,
}

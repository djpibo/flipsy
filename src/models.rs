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
    pub object_alias: Option<String>,
    pub outline_hints: Vec<String>,
    pub qblock_name: Option<String>,
    pub cardinality_ratio: f64,
    pub buffer_percentage: f64,
    pub is_bottleneck: bool,
    pub bottleneck_tags: Vec<String>,
    pub children: Vec<PlanNode>,
}

impl Default for PlanNode {
    fn default() -> Self {
        Self {
            id: 0,
            parent_id: None,
            position: 0,
            operation: String::new(),
            options: None,
            object_name: None,
            starts: 1,
            e_rows: 1,
            a_rows: 1,
            a_time_ms: 0.0,
            buffers: 0,
            reads: 0,
            cost: None,
            access_predicates: None,
            filter_predicates: None,
            object_alias: None,
            outline_hints: Vec::new(),
            qblock_name: None,
            cardinality_ratio: 1.0,
            buffer_percentage: 0.0,
            is_bottleneck: false,
            bottleneck_tags: Vec::new(),
            children: Vec::new(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
    pub join_type: String,
    pub join_condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryStructure {
    pub ctes: Vec<String>,
    pub tables: Vec<TableRef>,
    pub filters: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SampleQuery {
    pub title: String,
    #[allow(dead_code)]
    pub description: String,
    pub sql: String,
}

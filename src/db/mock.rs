use crate::models::{PlanNode, QueryResult};
use std::collections::HashMap;

pub struct MockEngine;

impl MockEngine {
    pub fn execute(sql: &str) -> (QueryResult, Vec<PlanNode>) {
        let trimmed = sql.trim().to_uppercase();

        if trimmed.contains("TB_CENTER_INVENTORY") || trimmed.contains("INVENTORY") || trimmed.contains("ORDER") {
            let columns = vec![
                "CENTER_CD".to_string(),
                "ITEM_CD".to_string(),
                "ALLOCATED_QTY".to_string(),
                "ORDER_DATE".to_string(),
                "MOVE_STATUS".to_string(),
            ];

            let rows = vec![
                vec!["HUB_01".into(), "SKU-882194".into(), "450".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882195".into(), "120".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882196".into(), "890".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882201".into(), "35".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882209".into(), "210".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882215".into(), "670".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882220".into(), "50".into(), "2026-09-20".into(), "READY".into()],
                vec!["HUB_01".into(), "SKU-882231".into(), "140".into(), "2026-09-20".into(), "READY".into()],
            ];

            let result = QueryResult {
                columns,
                rows,
                elapsed_ms: 1722.45,
                row_count: 150_000,
                sql_id: Some("8w7qm1zfk5u1x".to_string()),
                child_number: Some(1),
                plan_hash_value: Some(11223344),
                message: Some("150,000 rows fetched (Top 8 displayed in grid).".to_string()),
            };

            let total_buffers: u64 = 15_000_000;
            let flat_nodes = vec![
                PlanNode {
                    id: 0,
                    parent_id: None,
                    position: 0,
                    operation: "SELECT STATEMENT".to_string(),
                    options: None,
                    object_name: None,
                    starts: 1,
                    e_rows: 30,
                    a_rows: 150_000,
                    a_time_ms: 1722.11,
                    buffers: total_buffers,
                    reads: 1_420_000,
                    cost: Some(5124),
                    access_predicates: None,
                    filter_predicates: None,
                    cardinality_ratio: 5000.0,
                    buffer_percentage: 100.0,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
                PlanNode {
                    id: 1,
                    parent_id: Some(0),
                    position: 1,
                    operation: "NESTED LOOPS".to_string(),
                    options: None,
                    object_name: None,
                    starts: 1,
                    e_rows: 30,
                    a_rows: 150_000,
                    a_time_ms: 1722.08,
                    buffers: total_buffers,
                    reads: 1_420_000,
                    cost: Some(5124),
                    access_predicates: None,
                    filter_predicates: None,
                    cardinality_ratio: 5000.0,
                    buffer_percentage: 100.0,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
                PlanNode {
                    id: 2,
                    parent_id: Some(1),
                    position: 1,
                    operation: "NESTED LOOPS".to_string(),
                    options: None,
                    object_name: None,
                    starts: 1,
                    e_rows: 30,
                    a_rows: 150_000,
                    a_time_ms: 12.45,
                    buffers: 302_000,
                    reads: 28_000,
                    cost: Some(120),
                    access_predicates: None,
                    filter_predicates: None,
                    cardinality_ratio: 5000.0,
                    buffer_percentage: 2.01,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
                PlanNode {
                    id: 3,
                    parent_id: Some(2),
                    position: 1,
                    operation: "TABLE ACCESS".to_string(),
                    options: Some("BY INDEX ROWID".to_string()),
                    object_name: Some("TB_CENTER_INVENTORY".to_string()),
                    starts: 1,
                    e_rows: 30,
                    a_rows: 150_000,
                    a_time_ms: 8.12,
                    buffers: 152_000,
                    reads: 12_000,
                    cost: Some(31),
                    access_predicates: None,
                    filter_predicates: Some("\"I\".\"ALLOCATED_QTY\" IS NOT NULL".to_string()),
                    cardinality_ratio: 5000.0,
                    buffer_percentage: 1.01,
                    is_bottleneck: true,
                    bottleneck_tags: vec!["카디널리티 예측 왜곡 (5000x)".to_string(), "드라이빙 대량 레코드".to_string()],
                    children: vec![],
                },
                PlanNode {
                    id: 4,
                    parent_id: Some(3),
                    position: 1,
                    operation: "INDEX".to_string(),
                    options: Some("RANGE SCAN".to_string()),
                    object_name: Some("IX_CENTER_INV_01".to_string()),
                    starts: 1,
                    e_rows: 30,
                    a_rows: 150_000,
                    a_time_ms: 0.64,
                    buffers: 1_200,
                    reads: 310,
                    cost: Some(2),
                    access_predicates: Some("\"I\".\"CENTER_CD\"=:CENTER_CD".to_string()),
                    filter_predicates: None,
                    cardinality_ratio: 5000.0,
                    buffer_percentage: 0.01,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
                PlanNode {
                    id: 5,
                    parent_id: Some(2),
                    position: 2,
                    operation: "INDEX".to_string(),
                    options: Some("RANGE SCAN".to_string()),
                    object_name: Some("IX_INV_MOVE_ORDER_01".to_string()),
                    starts: 150_000,
                    e_rows: 1,
                    a_rows: 150_000,
                    a_time_ms: 494.22,
                    buffers: 150_000,
                    reads: 14_000,
                    cost: Some(2),
                    access_predicates: Some("\"I\".\"ITEM_CD\"=\"M\".\"ITEM_CD\" AND \"M\".\"ORDER_DATE\"=TRUNC(SYSDATE@!)".to_string()),
                    filter_predicates: None,
                    cardinality_ratio: 1.0,
                    buffer_percentage: 1.0,
                    is_bottleneck: true,
                    bottleneck_tags: vec!["Starts 폭증 (150K회 루프)".to_string()],
                    children: vec![],
                },
                PlanNode {
                    id: 6,
                    parent_id: Some(1),
                    position: 2,
                    operation: "TABLE ACCESS".to_string(),
                    options: Some("BY INDEX ROWID".to_string()),
                    object_name: Some("TB_INVENTORY_MOVE_ORDER".to_string()),
                    starts: 150_000,
                    e_rows: 1,
                    a_rows: 150_000,
                    a_time_ms: 1215.51,
                    buffers: 14_000_000,
                    reads: 1_370_000,
                    cost: Some(3),
                    access_predicates: None,
                    filter_predicates: Some("(\"M\".\"MOVE_STATUS\"='READY' AND \"M\".\"SRC_CENTER_CD\"=:CENTER_CD)".to_string()),
                    cardinality_ratio: 1.0,
                    buffer_percentage: 93.33,
                    is_bottleneck: true,
                    bottleneck_tags: vec![
                        "Buffers 쏠림 (전체의 93.3%)".to_string(),
                        "Single Block I/O 다발 (1.37M Reads)".to_string(),
                        "Starts 150K 반복 탐색".to_string(),
                    ],
                    children: vec![],
                },
            ];

            let tree = Self::build_tree(flat_nodes);
            (result, tree)
        } else {
            // General / Dual query simulation
            let result = QueryResult {
                columns: vec!["STATUS".to_string(), "SYSDATE".to_string(), "INSTANCE_NAME".to_string()],
                rows: vec![
                    vec!["CONNECTED".into(), "2026-09-20 09:35:00".into(), "ORCL19C_PDB1".into()],
                ],
                elapsed_ms: 0.85,
                row_count: 1,
                sql_id: Some("7gqf9a441n0x2".to_string()),
                child_number: Some(0),
                plan_hash_value: Some(1388707743),
                message: Some("1 row selected.".to_string()),
            };

            let flat_nodes = vec![
                PlanNode {
                    id: 0,
                    parent_id: None,
                    position: 0,
                    operation: "SELECT STATEMENT".to_string(),
                    options: None,
                    object_name: None,
                    starts: 1,
                    e_rows: 1,
                    a_rows: 1,
                    a_time_ms: 0.85,
                    buffers: 3,
                    reads: 0,
                    cost: Some(2),
                    access_predicates: None,
                    filter_predicates: None,
                    cardinality_ratio: 1.0,
                    buffer_percentage: 100.0,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
                PlanNode {
                    id: 1,
                    parent_id: Some(0),
                    position: 1,
                    operation: "FAST DUAL".to_string(),
                    options: None,
                    object_name: None,
                    starts: 1,
                    e_rows: 1,
                    a_rows: 1,
                    a_time_ms: 0.45,
                    buffers: 3,
                    reads: 0,
                    cost: Some(2),
                    access_predicates: None,
                    filter_predicates: None,
                    cardinality_ratio: 1.0,
                    buffer_percentage: 100.0,
                    is_bottleneck: false,
                    bottleneck_tags: vec![],
                    children: vec![],
                },
            ];

            let tree = Self::build_tree(flat_nodes);
            (result, tree)
        }
    }

    pub fn build_tree(mut nodes: Vec<PlanNode>) -> Vec<PlanNode> {
        let mut children_map: HashMap<i32, Vec<PlanNode>> = HashMap::new();
        let mut root_nodes: Vec<PlanNode> = Vec::new();

        nodes.sort_by(|a, b| b.id.cmp(&a.id));

        for mut node in nodes {
            if let Some(mut children) = children_map.remove(&node.id) {
                children.sort_by(|a, b| a.position.cmp(&b.position));
                node.children = children;
            }

            if let Some(parent_id) = node.parent_id {
                children_map.entry(parent_id).or_default().push(node);
            } else {
                root_nodes.push(node);
            }
        }

        root_nodes.sort_by(|a, b| a.id.cmp(&b.id));
        root_nodes
    }
}

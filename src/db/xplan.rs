use crate::models::PlanNode;
use std::collections::HashMap;
use std::sync::OnceLock;

static RE_SQL_ID: OnceLock<regex::Regex> = OnceLock::new();

// Oracle DBMS_XPLAN Output Parser
//
// Handles:
// - DISPLAY / DISPLAY_CURSOR output table parsing
// - Depth-based hierarchical indentation and parent_id reconstruction
// - Predicate parsing with balanced parentheses and quote normalization
// - Query Block and Object Alias resolution
// - Bottleneck detection based on Buffer I/O and Cardinality Skew
pub fn parse_xplan_output(xplan: &str) -> Vec<PlanNode> {
        let mut nodes = Vec::new();
        let mut raw_access_map: HashMap<i32, String> = HashMap::new();
        let mut raw_filter_map: HashMap<i32, String> = HashMap::new();
        let mut alias_map: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new(); // id -> (alias, qblock)
        let mut outline_hints: Vec<String> = Vec::new();
        let mut col_map: HashMap<String, usize> = HashMap::new();

        let mut in_alias_section = false;
        let mut in_outline_section = false;
        let mut in_pred_section = false;
        let mut current_pred_id: Option<i32> = None;
        let mut current_clause_type: Option<u8> = None; // 0: access, 1: filter

        let parse_val = |s: &str| -> u64 {
            let s = s.trim().to_uppercase();
            if s.is_empty() { return 0; }
            let mut mult: f64 = 1.0;
            let num_str = if s.ends_with('K') {
                mult = 1_000.0;
                &s[..s.len() - 1]
            } else if s.ends_with('M') {
                mult = 1_000_000.0;
                &s[..s.len() - 1]
            } else if s.ends_with('G') {
                mult = 1_000_000_000.0;
                &s[..s.len() - 1]
            } else {
                &s
            };
            num_str.parse::<f64>().map(|v| (v * mult) as u64).unwrap_or(0)
        };

        let parse_time = |s: &str| -> f64 {
            let s = s.trim();
            if s.is_empty() { return 0.0; }
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 3 {
                let h = parts[0].parse::<f64>().unwrap_or(0.0);
                let m = parts[1].parse::<f64>().unwrap_or(0.0);
                let sec = parts[2].parse::<f64>().unwrap_or(0.0);
                (h * 3600.0 + m * 60.0 + sec) * 1000.0
            } else if parts.len() == 2 {
                let m = parts[0].parse::<f64>().unwrap_or(0.0);
                let sec = parts[1].parse::<f64>().unwrap_or(0.0);
                (m * 60.0 + sec) * 1000.0
            } else {
                s.parse::<f64>().unwrap_or(0.0) * 1000.0
            }
        };

        for line in xplan.lines() {
            let trimmed = line.trim();

            // Detect section headers
            if trimmed.contains("Query Block Name / Object Alias") {
                in_alias_section = true;
                in_outline_section = false;
                in_pred_section = false;
                continue;
            } else if trimmed.contains("Outline Data") {
                in_alias_section = false;
                in_outline_section = true;
                in_pred_section = false;
                continue;
            } else if trimmed.contains("Predicate Information") {
                in_alias_section = false;
                in_outline_section = false;
                in_pred_section = true;
                continue;
            } else if trimmed.contains("Column Projection") || trimmed.contains("Note") {
                in_alias_section = false;
                in_outline_section = false;
                in_pred_section = false;
            }

            // Detect Plan Table Header
            if !in_alias_section && !in_outline_section && !in_pred_section {
                if trimmed.starts_with('|') && trimmed.contains("Id") && trimmed.contains("Operation") {
                    col_map.clear();
                    let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                    for (idx, name) in parts.iter().enumerate() {
                        if !name.is_empty() {
                            col_map.insert(name.to_string(), idx);
                        }
                    }
                    continue;
                }

                // 1. Parse Plan Table Rows
                if trimmed.starts_with('|') && !trimmed.contains("---") && !col_map.is_empty() {
                    let raw_parts: Vec<&str> = trimmed.split('|').collect();
                    let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                    if let Some(&id_idx) = col_map.get("Id") {
                        if id_idx < parts.len() {
                            let id_str = parts[id_idx].trim_start_matches('*').trim();
                            if let Ok(id) = id_str.parse::<i32>() {
                                let op_idx = col_map.get("Operation").copied().unwrap_or(2);
                                let op_raw = if op_idx < raw_parts.len() { raw_parts[op_idx] } else { "" };
                                let expanded_op = op_raw.replace('\t', "        ");
                                let leading_spaces = expanded_op.chars().take_while(|c| *c == ' ').count();
                                let depth = leading_spaces.saturating_sub(1);

                                let full_op = if op_idx < parts.len() { parts[op_idx].to_string() } else { "UNKNOWN".to_string() };
                                let (op, opt) = if full_op.contains("TABLE ACCESS") {
                                    let sub = full_op.replace("TABLE ACCESS", "").trim().to_string();
                                    ("TABLE ACCESS".to_string(), if sub.is_empty() { None } else { Some(sub) })
                                } else if full_op.contains("INDEX") {
                                    let sub = full_op.replace("INDEX", "").trim().to_string();
                                    ("INDEX".to_string(), if sub.is_empty() { None } else { Some(sub) })
                                } else {
                                    (full_op.clone(), None)
                                };

                                let name = col_map.get("Name").and_then(|&idx| {
                                    if idx < parts.len() && !parts[idx].is_empty() {
                                        Some(parts[idx].to_string())
                                    } else {
                                        None
                                    }
                                });

                                let starts = col_map.get("Starts").and_then(|&idx| {
                                    if idx < parts.len() { Some(parse_val(parts[idx])) } else { None }
                                }).unwrap_or(0);

                                let e_rows = col_map.get("E-Rows")
                                    .or_else(|| col_map.get("Rows"))
                                    .and_then(|&idx| if idx < parts.len() { Some(parse_val(parts[idx])) } else { None })
                                    .unwrap_or(1);

                                let a_rows = col_map.get("A-Rows").and_then(|&idx| {
                                    if idx < parts.len() { Some(parse_val(parts[idx])) } else { None }
                                }).unwrap_or(0);

                                let buffers = col_map.get("Buffers").and_then(|&idx| {
                                    if idx < parts.len() { Some(parse_val(parts[idx])) } else { None }
                                }).unwrap_or(0);

                                let reads = col_map.get("Reads").and_then(|&idx| {
                                    if idx < parts.len() { Some(parse_val(parts[idx])) } else { None }
                                }).unwrap_or(0);

                                let a_time_ms = col_map.get("A-Time")
                                    .or_else(|| col_map.get("Time"))
                                    .and_then(|&idx| if idx < parts.len() { Some(parse_time(parts[idx])) } else { None })
                                    .unwrap_or(0.0);

                                let cost = col_map.get("Cost (%CPU)")
                                    .or_else(|| col_map.get("Cost"))
                                    .and_then(|&idx| {
                                        if idx < parts.len() {
                                            parts[idx].split_whitespace().next().and_then(|s| s.parse::<i64>().ok())
                                        } else {
                                            None
                                        }
                                    });

                                nodes.push(PlanNode {
                                    id,
                                    parent_id: None,
                                    position: id + 1,
                                    operation: op,
                                    options: opt,
                                    object_name: name,
                                    starts,
                                    e_rows,
                                    a_rows,
                                    a_time_ms,
                                    buffers,
                                    reads,
                                    cost,
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
                                    depth,
                                });
                            }
                        }
                    }
                }
            }

            // 2. Parse Query Block Name / Object Alias
            if in_alias_section {
                if let Some((id, rest)) = parse_id_prefix_line(trimmed) {
                    let clean_right = rest.replace('"', "").trim().to_string();
                    let parts: Vec<&str> = clean_right.split('/').map(|s| s.trim()).collect();
                    let qblock = parts.first().filter(|s| !s.is_empty()).map(|s| s.to_string());
                    let alias = parts.get(1).filter(|s| !s.is_empty()).map(|s| s.to_string());
                    alias_map.insert(id, (alias, qblock));
                }
            }

            // 3. Parse Outline Hints
            if in_outline_section
                && !trimmed.starts_with("/*") && !trimmed.starts_with("*/") && !trimmed.contains("OUTLINE_DATA") && !trimmed.starts_with("---") && !trimmed.is_empty() {
                    outline_hints.push(trimmed.to_string());
            }

            // 4. Parse Predicate Information (Accurate line & multi-line parenthesis preserving)
            if in_pred_section && !trimmed.is_empty() && !trimmed.starts_with("---") {
                    let mut line_text = trimmed;
                    if let Some((id, rest)) = parse_id_prefix_line(trimmed) {
                        current_pred_id = Some(id);
                        line_text = rest;
                        current_clause_type = None;
                    }

                    if let Some(id) = current_pred_id {
                        if let Some(inner) = line_text.strip_prefix("access(") {
                            current_clause_type = Some(0);
                            let entry = raw_access_map.entry(id).or_default();
                            if !entry.is_empty() {
                                entry.push(' ');
                            }
                            entry.push_str(inner);
                        } else if let Some(inner) = line_text.strip_prefix("filter(") {
                            current_clause_type = Some(1);
                            let entry = raw_filter_map.entry(id).or_default();
                            if !entry.is_empty() {
                                entry.push(' ');
                            }
                            entry.push_str(inner);
                        } else if let Some(clause_type) = current_clause_type {
                            let entry = if clause_type == 0 {
                                raw_access_map.entry(id).or_default()
                            } else {
                                raw_filter_map.entry(id).or_default()
                            };
                            if !entry.is_empty() {
                                entry.push(' ');
                            }
                            entry.push_str(line_text);
                        }
                    }
                }
        }

        // Post-process Predicates: strip ONLY the single wrapper closing parenthesis ')' and double quotes
        let mut pred_map: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new();
        for (id, raw_s) in raw_access_map {
            let t = raw_s.trim();
            let clean = t.strip_suffix(')').unwrap_or(t).trim().replace('"', "");
            pred_map.entry(id).or_insert((None, None)).0 = Some(clean);
        }
        for (id, raw_s) in raw_filter_map {
            let t = raw_s.trim();
            let clean = t.strip_suffix(')').unwrap_or(t).trim().replace('"', "");
            pred_map.entry(id).or_insert((None, None)).1 = Some(clean);
        }

        // 5. Correlate All Information onto each PlanNode!
        let total_buffers = nodes.iter().map(|n| n.buffers).max().unwrap_or(0).max(1);
        for node in &mut nodes {
            // Predicates
            if let Some((acc, filt)) = pred_map.get(&node.id) {
                node.access_predicates = acc.clone();
                node.filter_predicates = filt.clone();
            }

            // Aliases & QBlock
            if let Some((alias, qb)) = alias_map.get(&node.id) {
                node.object_alias = alias.clone();
                node.qblock_name = qb.clone();
            }

            // Outline Hints (clean double quotes and match specific table/alias)
            let mut matched_hints = Vec::new();
            if let Some(alias) = &node.object_alias {
                let clean_alias = alias.replace('"', "");
                for hint in &outline_hints {
                    let clean_hint = hint.replace('"', "");
                    if clean_hint.contains(&clean_alias) || clean_hint.contains(alias) {
                        matched_hints.push(clean_hint);
                    }
                }
            } else if let Some(obj) = &node.object_name {
                for hint in &outline_hints {
                    let clean_hint = hint.replace('"', "");
                    if clean_hint.contains(obj) {
                        matched_hints.push(clean_hint);
                    }
                }
            }
            node.outline_hints = matched_hints;

            // Metrics Analysis
            if node.buffers > 0 {
                node.buffer_percentage = (node.buffers as f64 / total_buffers as f64) * 100.0;
            }
            if node.e_rows > 0 && node.a_rows > 0 {
                let ratio = if node.a_rows > node.e_rows {
                    node.a_rows as f64 / node.e_rows as f64
                } else {
                    node.e_rows as f64 / node.a_rows as f64
                };
                node.cardinality_ratio = ratio;
            }

            let is_full = node.operation.contains("FULL") || node.options.as_deref().unwrap_or("").contains("FULL");
            let is_cartesian = node.operation.contains("CARTESIAN");
            let is_heavy_buf = node.buffer_percentage >= 30.0;
            let is_heavy_skew = node.cardinality_ratio >= 10.0;
            node.is_bottleneck = is_full || is_cartesian || is_heavy_buf || is_heavy_skew;
            if is_full {
                node.bottleneck_tags.push("Table Full Scan".to_string());
            }
            if is_cartesian {
                node.bottleneck_tags.push("Cartesian Product".to_string());
            }
            if is_heavy_buf {
                node.bottleneck_tags.push("High Buffer I/O".to_string());
            }
            if is_heavy_skew {
                node.bottleneck_tags.push("Cardinality Skew".to_string());
            }
        }

        // Reconstruct hierarchical parent_id from depth
        let mut depth_stack: Vec<(usize, i32)> = Vec::new();
        for node in &mut nodes {
            while let Some(&(d, _)) = depth_stack.last() {
                if d >= node.depth {
                    depth_stack.pop();
                } else {
                    break;
                }
            }
            node.parent_id = depth_stack.last().map(|&(_, parent_id)| parent_id);
            depth_stack.push((node.depth, node.id));
        }

        nodes
    }
    fn parse_id_prefix_line(line: &str) -> Option<(i32, &str)> {
        if let Some(idx) = line.find('-') {
            let left = line[..idx].trim();
            if let Ok(id) = left.parse::<i32>() {
                return Some((id, line[idx + 1..].trim()));
            }
        }
        None
    }




    pub fn parse_xplan_with_meta(xplan: &str) -> (Vec<PlanNode>, Option<u64>, Option<String>) {
        let nodes = parse_xplan_output(xplan);
        let mut plan_hash = None;
        let mut sql_id = None;
        let re_sql_id = RE_SQL_ID.get_or_init(|| regex::Regex::new(r"(?i)sql_id\s*[:=]?\s*([a-z0-9]+)").unwrap());

        for line in xplan.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Plan hash value:") {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].trim().parse::<u64>() {
                        plan_hash = Some(val);
                    }
                }
            } else if trimmed.starts_with("SQL_ID") || trimmed.contains("sql_id") {
                if let Some(cap) = re_sql_id.captures(trimmed) {
                    sql_id = cap.get(1).map(|m| m.as_str().to_string());
                }
            }
        }

        (nodes, plan_hash, sql_id)
    }





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xplan_display_cursor() {
        let xplan = r#"
-----------------------------------------------------------------------------------------------------------
| Id  | Operation          | Name | Starts | E-Rows | Cost (%CPU)| A-Rows |   A-Time   | Buffers | Reads  |
-----------------------------------------------------------------------------------------------------------
|   0 | SELECT STATEMENT   |      |      1 |        |     3 (100)|      1 |00:00:00.01 |       6 |      6 |
|   1 |  SORT AGGREGATE    |      |      1 |      1 |            |      1 |00:00:00.01 |       6 |      6 |
|*  2 |   TABLE ACCESS FULL| EMP  |      1 |     14 |     3   (0)|     14 |00:00:00.01 |       6 |      6 |
-----------------------------------------------------------------------------------------------------------
"#;
        let nodes = parse_xplan_output(xplan);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].id, 0);
        assert_eq!(nodes[0].starts, 1);
        assert_eq!(nodes[0].a_rows, 1);
        assert_eq!(nodes[0].buffers, 6);
        assert_eq!(nodes[0].reads, 6);
        assert_eq!(nodes[0].cost, Some(3));
        assert_eq!(nodes[2].id, 2);
        assert_eq!(nodes[2].operation, "TABLE ACCESS");
        assert_eq!(nodes[2].object_name, Some("EMP".to_string()));
        assert_eq!(nodes[2].a_rows, 14);
    }

    #[test]
    fn test_predicate_balanced_parentheses() {
        let xplan = r#"
-----------------------------------------------------------------------------------------------------------
| Id  | Operation          | Name        | Starts | E-Rows | Cost (%CPU)| A-Rows |   A-Time   | Buffers |
-----------------------------------------------------------------------------------------------------------
|   0 | SELECT STATEMENT   |             |      1 |        |     3 (100)|      1 |00:00:00.01 |       6 |
|* 15 |  TABLE ACCESS FULL | TB_ORD_MST  |      1 |   4923 |   608   (0)|   1964 |00:00:00.06 |    1493 |
|* 23 |  TABLE ACCESS FULL | TB_CUST_MST |      1 |  40000 |   262   (1)|  40000 |00:00:00.01 |     935 |
-----------------------------------------------------------------------------------------------------------

Predicate Information (identified by operation id):
---------------------------------------------------

  15 - filter("O"."ORD_DATE">=TO_DATE(' 2025-06-01 00:00:00', 'syyyy-mm-dd hh24:mi:ss') AND
              ("O"."ORD_STATUS"='DELIVERED' OR "O"."ORD_STATUS"='PAY_COMPLETED' OR "O"."ORD_STATUS"='SHIPPING'))
  23 - filter(("C"."CUST_GRADE"='GOLD' OR "C"."CUST_GRADE"='VIP' OR "C"."CUST_GRADE"='VVIP'))
"#;
        let nodes = parse_xplan_output(xplan);
        assert_eq!(nodes.len(), 3);

        let node23 = nodes.iter().find(|n| n.id == 23).expect("node 23 must exist");
        let filt23 = node23.filter_predicates.as_ref().expect("node 23 must have filter");
        assert_eq!(filt23, r#"(C.CUST_GRADE='GOLD' OR C.CUST_GRADE='VIP' OR C.CUST_GRADE='VVIP')"#);
        assert_eq!(filt23.matches('(').count(), filt23.matches(')').count(), "Parentheses must be balanced!");

        let node15 = nodes.iter().find(|n| n.id == 15).expect("node 15 must exist");
        let filt15 = node15.filter_predicates.as_ref().expect("node 15 must have filter");
        assert!(filt15.contains("syyyy-mm-dd hh24:mi:ss"));
        assert!(filt15.contains(r#"(O.ORD_STATUS='DELIVERED' OR O.ORD_STATUS='PAY_COMPLETED' OR O.ORD_STATUS='SHIPPING')"#));
        assert_eq!(filt15.matches('(').count(), filt15.matches(')').count(), "Parentheses must be balanced!");
    }

    #[test]
    fn test_parse_xplan_explain_plan() {
        let xplan = r#"
Plan hash value: 2083865914

---------------------------------------------------------------------------
| Id  | Operation          | Name | Rows  | Bytes | Cost (%CPU)| Time     |
---------------------------------------------------------------------------
|   0 | SELECT STATEMENT   |      |     1 |     3 |     3   (0)| 00:00:01 |
|   1 |  SORT AGGREGATE    |      |     1 |     3 |            |          |
|*  2 |   TABLE ACCESS FULL| EMP  |    14 |    42 |     3   (0)| 00:00:01 |
---------------------------------------------------------------------------
"#;
        let (nodes, hash, _) = parse_xplan_with_meta(xplan);
        assert_eq!(hash, Some(2083865914));
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].e_rows, 1);
        assert_eq!(nodes[0].cost, Some(3));
        assert_eq!(nodes[0].starts, 0);
        assert_eq!(nodes[0].a_rows, 0);
        assert_eq!(nodes[0].buffers, 0);
        assert_eq!(nodes[2].e_rows, 14);
        assert_eq!(nodes[2].object_name, Some("EMP".to_string()));
    }
}


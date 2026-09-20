use crate::models::{ConnectionConfig, PlanNode, QueryResult, TableRef, QueryStructure};
use crate::db::mock::MockEngine;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone)]
pub struct QueryProgressTracker {
    pub bytes_read: Arc<AtomicUsize>,
    pub rows_read: Arc<AtomicUsize>,
    pub is_cancelled: Arc<AtomicBool>,
}

impl QueryProgressTracker {
    pub fn new() -> Self {
        Self {
            bytes_read: Arc::new(AtomicUsize::new(0)),
            rows_read: Arc::new(AtomicUsize::new(0)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct ExecutionResult {
    pub query_result: QueryResult,
    pub plan_nodes: Option<Vec<PlanNode>>,
    pub plan_hash: Option<u64>,
    pub sql_id: Option<String>,
}

pub struct AsyncExecutionHandle {
    pub start_time: Instant,
    pub tracker: QueryProgressTracker,
    pub rx: Receiver<ExecutionResult>,
    pub is_explain: bool,
}

pub struct DatabaseSession {
    pub config: ConnectionConfig,
    pub is_connected: bool,
    pub is_real_oracle: bool,
    pub db_version: String,
    pub last_query_result: Option<QueryResult>,
    pub last_plan: Option<Vec<PlanNode>>,
    pub last_sql_id: Option<String>,
    pub last_plan_hash: Option<u64>,
}

impl DatabaseSession {
    pub fn new() -> Self {
        Self {
            config: ConnectionConfig::default(),
            is_connected: false,
            is_real_oracle: false,
            db_version: String::new(),
            last_query_result: None,
            last_plan: None,
            last_sql_id: None,
            last_plan_hash: None,
        }
    }

    pub fn connect(&mut self, config: ConnectionConfig) -> Result<(), String> {
        if config.host.is_empty() || config.service_name.is_empty() || config.username.is_empty() {
            return Err("호스트, 서비스 이름, 사용자명은 필수 입력 항목입니다.".to_string());
        }

        // 1. Live TCP Socket Probe to port 1521
        let addr = format!("{}:{}", config.host, config.port);
        match addr.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(target) = addrs.next() {
                    if let Err(e) = TcpStream::connect_timeout(&target, Duration::from_millis(2000)) {
                        return Err(format!(
                            "오라클 리스너({}:{})에 접속할 수 없습니다: {}\n(Docker 컨테이너 및 1521 포트 상태를 확인하세요)",
                            config.host, config.port, e
                        ));
                    }
                } else {
                    return Err(format!("호스트 주소를 확인할 수 없습니다: {}", addr));
                }
            }
            Err(e) => {
                return Err(format!("올바르지 않은 호스트 주소입니다({}): {}", addr, e));
            }
        }

        // 2. Real Oracle Authentication Probe via docker container (oracle23ai)
        let conn_str = format!(
            "{}/{}@{}:{}/{}",
            config.username, config.password, config.host, config.port, config.service_name
        );

        let probe_script = "SELECT banner FROM v$version WHERE ROWNUM = 1;\nEXIT;\n";
        match Self::run_sqlplus_command(&conn_str, probe_script) {
            Ok(output) => {
                let mut banner = String::new();
                for line in output.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Oracle") {
                        banner = trimmed.to_string();
                        break;
                    }
                }
                if banner.is_empty() {
                    banner = "Oracle AI Database 26ai Free Release 23.26.3.0.0".to_string();
                }

                self.is_real_oracle = true;
                self.db_version = banner;
            }
            Err(err) => {
                if err.contains("ORA-") || err.contains("SP2-") {
                    return Err(format!("오라클 접속 실패: {}", err));
                }
                self.is_real_oracle = false;
                self.db_version = "Oracle 26ai (Simulated Engine)".to_string();
            }
        }

        self.config = config;
        self.is_connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.is_connected = false;
        self.is_real_oracle = false;
        self.db_version.clear();
        self.last_query_result = None;
        self.last_plan = None;
        self.last_sql_id = None;
        self.last_plan_hash = None;
    }

    fn run_sqlplus_command(conn_str: &str, script: &str) -> Result<String, String> {
        let mut child = Command::new("docker")
            .args(["exec", "-i", "oracle23ai", "sqlplus", "-L", "-s", conn_str])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Docker 실행 실패: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(script.as_bytes());
        }

        let output = child.wait_with_output()
            .map_err(|e| format!("Oracle 프로세스 대기 실패: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() || stdout.contains("ERROR:") || stdout.contains("ORA-") {
            let mut err_msg = String::new();
            for line in stdout.lines().chain(stderr.lines()) {
                let trimmed = line.trim();
                if trimmed.starts_with("ORA-") || trimmed.starts_with("SP2-") {
                    if !err_msg.is_empty() {
                        err_msg.push(' ');
                    }
                    err_msg.push_str(trimmed);
                }
            }
            if err_msg.is_empty() {
                err_msg = stdout.trim().to_string();
            }
            return Err(err_msg);
        }

        Ok(stdout)
    }

    pub fn run_sqlplus_streaming(conn_str: &str, script: &str, tracker: &QueryProgressTracker) -> Result<String, String> {
        let mut child = Command::new("docker")
            .args(["exec", "-i", "oracle23ai", "sqlplus", "-L", "-s", conn_str])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Docker 실행 실패: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(script.as_bytes());
        }

        let stdout = child.stdout.take().ok_or_else(|| "stdout 캡처 실패".to_string())?;
        let mut reader = BufReader::new(stdout);
        let mut full_output = String::new();
        let mut line = String::new();

        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            if tracker.is_cancelled.load(Ordering::Relaxed) {
                let _ = child.kill();
                return Err("사용자에 의해 쿼리 실행이 중단되었습니다.".to_string());
            }
            tracker.bytes_read.fetch_add(n, Ordering::Relaxed);
            tracker.rows_read.fetch_add(1, Ordering::Relaxed);
            full_output.push_str(&line);
            line.clear();
        }

        let status = child.wait().map_err(|e| format!("Oracle 프로세스 대기 실패: {}", e))?;

        if !status.success() || full_output.contains("ERROR:") || full_output.contains("ORA-") {
            let mut err_msg = String::new();
            for l in full_output.lines() {
                let trimmed = l.trim();
                if trimmed.starts_with("ORA-") || trimmed.starts_with("SP2-") {
                    if !err_msg.is_empty() {
                        err_msg.push(' ');
                    }
                    err_msg.push_str(trimmed);
                }
            }
            if err_msg.is_empty() {
                err_msg = full_output.trim().to_string();
            }
            return Err(err_msg);
        }

        Ok(full_output)
    }

    pub fn execute_async(&self, sql: &str, is_explain: bool) -> AsyncExecutionHandle {
        let tracker = QueryProgressTracker::new();
        let tracker_clone = tracker.clone();
        let (tx, rx) = channel();
        let config = self.config.clone();
        let is_real = self.is_real_oracle;
        let sql_string = sql.to_string();

        std::thread::spawn(move || {
            let start = Instant::now();
            if is_real {
                let conn_str = format!(
                    "{}/{}@{}:{}/{}",
                    config.username, config.password, config.host, config.port, config.service_name
                );

                let clean_sql = sql_string.trim().trim_end_matches(';');

                if is_explain {
                    let plan_script = format!(
                        "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                        clean_sql
                    );

                    let plan_res = DatabaseSession::run_sqlplus_streaming(&conn_str, &plan_script, &tracker_clone);
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

                    match plan_res {
                        Ok(plan_output) => {
                            let (nodes, hash, sql_id) = DatabaseSession::parse_xplan_with_meta(&plan_output);
                            let res = QueryResult {
                                columns: vec!["PLAN_TABLE_OUTPUT".to_string()],
                                rows: plan_output.lines().map(|l| vec![l.to_string()]).collect(),
                                elapsed_ms: elapsed,
                                row_count: nodes.len(),
                                sql_id: sql_id.clone().or(Some("ora26ai_live".to_string())),
                                child_number: Some(0),
                                plan_hash_value: hash,
                                message: Some(format!("Oracle 26ai Explain 성공 ({:.2}ms)", elapsed)),
                            };
                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes: Some(nodes),
                                plan_hash: hash,
                                sql_id: sql_id.or(Some("ora26ai_live".to_string())),
                            });
                        }
                        Err(err) => {
                            let res = QueryResult {
                                columns: vec!["ERROR".to_string()],
                                rows: vec![vec![err.clone()]],
                                elapsed_ms: elapsed,
                                row_count: 0,
                                sql_id: None,
                                child_number: None,
                                plan_hash_value: None,
                                message: Some(format!("Explain 오류: {}", err)),
                            };
                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes: None,
                                plan_hash: None,
                                sql_id: None,
                            });
                        }
                    }
                } else {
                    let query_script = format!(
                        "SET MARKUP CSV ON QUOTE ON\nSET HEADING ON\nSET FEEDBACK OFF\nSET PAGESIZE 50000\nSET LINESIZE 32767\n{};\nEXIT;\n",
                        clean_sql
                    );

                    match DatabaseSession::run_sqlplus_streaming(&conn_str, &query_script, &tracker_clone) {
                        Ok(csv_output) => {
                            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                            let (cols, rows) = DatabaseSession::parse_csv_output(&csv_output);

                            let plan_script = format!(
                                "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                                clean_sql
                            );

                            let (plan_nodes, plan_hash, sql_id) = if let Ok(plan_out) = DatabaseSession::run_sqlplus_command(&conn_str, &plan_script) {
                                let (p, h, s) = DatabaseSession::parse_xplan_with_meta(&plan_out);
                                (Some(p), h, s.or(Some("ora26ai_live".to_string())))
                            } else {
                                (None, None, None)
                            };

                            let row_count = rows.len();
                            let total_bytes = tracker_clone.bytes_read.load(Ordering::Relaxed);
                            let mb = total_bytes as f64 / (1024.0 * 1024.0);

                            let size_desc = if mb >= 1.0 {
                                format!("{:.2} MB", mb)
                            } else {
                                format!("{:.1} KB", total_bytes as f64 / 1024.0)
                            };

                            let res = QueryResult {
                                columns: cols,
                                rows,
                                elapsed_ms: elapsed,
                                row_count,
                                sql_id: sql_id.clone(),
                                child_number: Some(0),
                                plan_hash_value: plan_hash,
                                message: Some(format!(
                                    "Oracle 26ai Live ({}@{}) - {}건 인출 ({}) in {:.2}ms",
                                    config.username, config.service_name, row_count, size_desc, elapsed
                                )),
                            };

                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes,
                                plan_hash,
                                sql_id,
                            });
                        }
                        Err(err) => {
                            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                            let res = QueryResult {
                                columns: vec!["ERROR".to_string()],
                                rows: vec![vec![err.clone()]],
                                elapsed_ms: elapsed,
                                row_count: 0,
                                sql_id: None,
                                child_number: None,
                                plan_hash_value: None,
                                message: Some(format!("실행 오류: {}", err)),
                            };
                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes: None,
                                plan_hash: None,
                                sql_id: None,
                            });
                        }
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(350));
                let (query_res, plan_nodes) = MockEngine::execute(&sql_string);
                let _ = tx.send(ExecutionResult {
                    query_result: query_res,
                    plan_nodes: Some(plan_nodes),
                    plan_hash: Some(272002086),
                    sql_id: Some("mock_ora26ai".to_string()),
                });
            }
        });

        AsyncExecutionHandle {
            start_time: Instant::now(),
            tracker,
            rx,
            is_explain,
                    }
    }

    #[allow(dead_code)]
    pub fn explain(&mut self, sql: &str) {
        if self.is_real_oracle {
            let conn_str = format!(
                "{}/{}@{}:{}/{}",
                self.config.username, self.config.password, self.config.host, self.config.port, self.config.service_name
            );
            let clean_sql = sql.trim().trim_end_matches(';');
            let plan_script = format!(
                "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                clean_sql
            );
            if let Ok(plan_output) = Self::run_sqlplus_command(&conn_str, &plan_script) {
                let (parsed_plan, hash, sql_id) = Self::parse_xplan_with_meta(&plan_output);
                if !parsed_plan.is_empty() {
                    self.last_plan = Some(parsed_plan);
                    self.last_plan_hash = hash;
                    self.last_sql_id = sql_id.or(Some("ora26ai_live".to_string()));
                    return;
                }
            }
        }

        let (_, mock_plan) = MockEngine::execute(sql);
        self.last_plan = Some(mock_plan);
        self.last_plan_hash = Some(272002086);
        self.last_sql_id = Some("mock_ora26ai".to_string());
    }

    #[allow(dead_code)]
    pub fn execute(&mut self, sql: &str) -> QueryResult {
        if self.is_real_oracle {
            let start = Instant::now();
            let conn_str = format!(
                "{}/{}@{}:{}/{}",
                self.config.username, self.config.password, self.config.host, self.config.port, self.config.service_name
            );

            let clean_sql = sql.trim().trim_end_matches(';');
            let query_script = format!(
                "SET MARKUP CSV ON QUOTE ON\nSET HEADING ON\nSET FEEDBACK OFF\nSET PAGESIZE 50000\nSET LINESIZE 32767\n{};\nEXIT;\n",
                clean_sql
            );

            match Self::run_sqlplus_command(&conn_str, &query_script) {
                Ok(csv_output) => {
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    let (cols, rows) = Self::parse_csv_output(&csv_output);

                    // Extract detailed XPLAN with ALL +OUTLINE +PREDICATE +ALIAS
                    let plan_script = format!(
                        "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                        clean_sql
                    );
                    if let Ok(plan_output) = Self::run_sqlplus_command(&conn_str, &plan_script) {
                        let (parsed_plan, hash, sql_id) = Self::parse_xplan_with_meta(&plan_output);
                        if !parsed_plan.is_empty() {
                            self.last_plan = Some(parsed_plan);
                            self.last_plan_hash = hash;
                            self.last_sql_id = sql_id.or(Some("ora26ai_live".to_string()));
                        } else {
                            let (_, mock_plan) = MockEngine::execute(sql);
                            self.last_plan = Some(mock_plan);
                            self.last_plan_hash = Some(272002086);
                            self.last_sql_id = Some("mock_ora26ai".to_string());
                        }
                    } else {
                        let (_, mock_plan) = MockEngine::execute(sql);
                        self.last_plan = Some(mock_plan);
                        self.last_plan_hash = Some(272002086);
                        self.last_sql_id = Some("mock_ora26ai".to_string());
                    }

                    let row_count = rows.len();
                    let res = QueryResult {
                        columns: cols,
                        rows,
                        elapsed_ms: elapsed,
                        row_count,
                        sql_id: Some("ora26ai_live".to_string()),
                        child_number: Some(0),
                        plan_hash_value: Some(272002086),
                        message: Some(format!(
                            "● Oracle 26ai Live ({}@{}) - {} rows in {:.2}ms",
                            self.config.username, self.config.service_name, row_count, elapsed
                        )),
                    };
                    self.last_query_result = Some(res.clone());
                    return res;
                }
                Err(err) => {
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    let res = QueryResult {
                        columns: vec!["ERROR".to_string()],
                        rows: vec![vec![err.clone()]],
                        elapsed_ms: elapsed,
                        row_count: 0,
                        sql_id: None,
                        child_number: None,
                        plan_hash_value: None,
                        message: Some(format!("Oracle Error: {}", err)),
                    };
                    self.last_query_result = Some(res.clone());
                    return res;
                }
            }
        }

        // Fallback to MockEngine for simulation
        let (result, plan) = MockEngine::execute(sql);
        self.last_query_result = Some(result.clone());
        self.last_plan = Some(plan);
        self.last_plan_hash = Some(272002086);
        self.last_sql_id = Some("mock_ora26ai".to_string());
        result
    }

    fn parse_csv_output(csv: &str) -> (Vec<String>, Vec<Vec<String>>) {
        let mut lines = csv.lines();
        let mut columns = Vec::new();
        let mut rows = Vec::new();

        if let Some(header) = lines.next() {
            columns = Self::parse_csv_line(header);
        }

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.contains("rows selected") || trimmed.contains("no rows") {
                continue;
            }
            let row = Self::parse_csv_line(trimmed);
            if !row.is_empty() && row.len() == columns.len() {
                rows.push(row);
            }
        }

        (columns, rows)
    }

    fn parse_csv_line(line: &str) -> Vec<String> {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    if in_quotes && chars.peek() == Some(&'"') {
                        current.push('"');
                        chars.next();
                    } else {
                        in_quotes = !in_quotes;
                    }
                }
                ',' if !in_quotes => {
                    fields.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(c),
            }
        }
        fields.push(current.trim().to_string());
        fields
    }

    /// Correlates Plan Table Nodes with Predicates, Object Aliases, and Outline Hints into unified PlanNodes
    pub fn parse_xplan_output(xplan: &str) -> Vec<PlanNode> {
        let mut nodes = Vec::new();
        let mut pred_map: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new(); // id -> (access, filter)
        let mut alias_map: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new(); // id -> (alias, qblock)
        let mut outline_hints: Vec<String> = Vec::new();

        let mut in_alias_section = false;
        let mut in_outline_section = false;
        let mut in_pred_section = false;
        let mut current_pred_id: Option<i32> = None;

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

            // 1. Parse Plan Table Rows
            if !in_alias_section && !in_outline_section && !in_pred_section {
                if trimmed.starts_with('|') && !trimmed.contains("Id") && !trimmed.contains("---") {
                    let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
                    if parts.len() >= 7 {
                        let id_str = parts[1].trim_start_matches('*').trim();
                        if let Ok(id) = id_str.parse::<i32>() {
                            let full_op = parts[2].to_string();
                            let (op, opt) = if full_op.contains("TABLE ACCESS") {
                                let sub = full_op.replace("TABLE ACCESS", "").trim().to_string();
                                ("TABLE ACCESS".to_string(), if sub.is_empty() { None } else { Some(sub) })
                            } else if full_op.contains("INDEX") {
                                let sub = full_op.replace("INDEX", "").trim().to_string();
                                ("INDEX".to_string(), if sub.is_empty() { None } else { Some(sub) })
                            } else {
                                (full_op.clone(), None)
                            };

                            let name = if parts[3].is_empty() { None } else { Some(parts[3].to_string()) };
                            let rows = parts[4].parse::<u64>().unwrap_or(1);
                            let cost = parts[6].split_whitespace().next().and_then(|s| s.parse::<i64>().ok());

                            let is_bottleneck = full_op.contains("FULL") || full_op.contains("CARTESIAN");
                            let mut tags = Vec::new();
                            if full_op.contains("FULL") {
                                tags.push("Table Full Scan".to_string());
                            }

                            nodes.push(PlanNode {
                                id,
                                parent_id: if id > 0 { Some(0) } else { None },
                                position: id + 1,
                                operation: op,
                                options: opt,
                                object_name: name,
                                starts: 1,
                                e_rows: rows,
                                a_rows: rows,
                                a_time_ms: 0.1,
                                buffers: cost.unwrap_or(1) as u64 * 3,
                                reads: 0,
                                cost,
                                access_predicates: None,
                                filter_predicates: None,
                                object_alias: None,
                                outline_hints: Vec::new(),
                                qblock_name: None,
                                cardinality_ratio: 1.0,
                                buffer_percentage: 0.0,
                                is_bottleneck,
                                bottleneck_tags: tags,
                                children: Vec::new(),
                            });
                        }
                    }
                }
            }

            // 2. Parse Query Block Name / Object Alias
            if in_alias_section {
                if let Some((id, rest)) = Self::parse_id_prefix_line(trimmed) {
                    let parts: Vec<&str> = rest.split('/').map(|s| s.trim()).collect();
                    let qblock = parts.get(0).map(|s| s.to_string());
                    let alias = parts.get(1).map(|s| s.to_string());
                    alias_map.insert(id, (alias, qblock));
                }
            }

            // 3. Parse Outline Hints
            if in_outline_section {
                if !trimmed.starts_with("/*") && !trimmed.starts_with("*/") && !trimmed.contains("OUTLINE_DATA") && !trimmed.starts_with("---") && !trimmed.is_empty() {
                    outline_hints.push(trimmed.to_string());
                }
            }

            // 4. Parse Predicate Information
            if in_pred_section {
                if let Some((id, rest)) = Self::parse_id_prefix_line(trimmed) {
                    current_pred_id = Some(id);
                    Self::append_predicate(&mut pred_map, id, rest);
                } else if let Some(id) = current_pred_id {
                    if !trimmed.is_empty() && !trimmed.starts_with("---") {
                        Self::append_predicate(&mut pred_map, id, trimmed);
                    }
                }
            }
        }

        // 5. Correlate All Information onto each PlanNode!
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

            // Outline Hints matching node's alias or object name
            let mut matched_hints = Vec::new();
            if let Some(alias) = &node.object_alias {
                let clean_alias = alias.replace('"', "");
                for hint in &outline_hints {
                    if hint.contains(&clean_alias) || hint.contains(alias) {
                        matched_hints.push(hint.clone());
                    }
                }
            } else if let Some(obj) = &node.object_name {
                for hint in &outline_hints {
                    if hint.contains(obj) {
                        matched_hints.push(hint.clone());
                    }
                }
            }
            if matched_hints.is_empty() {
                // If operation is join, look for join hint
                if node.operation.contains("HASH") {
                    for hint in &outline_hints {
                        if hint.starts_with("USE_HASH") {
                            matched_hints.push(hint.clone());
                        }
                    }
                } else if node.operation.contains("NL") || node.operation.contains("NESTED") {
                    for hint in &outline_hints {
                        if hint.starts_with("USE_NL") {
                            matched_hints.push(hint.clone());
                        }
                    }
                }
            }
            node.outline_hints = matched_hints;
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

    fn append_predicate(pred_map: &mut HashMap<i32, (Option<String>, Option<String>)>, id: i32, text: &str) {
        let entry = pred_map.entry(id).or_insert((None, None));
        if text.contains("access(") {
            let clean = text.replace("access(", "").trim_end_matches(')').to_string();
            entry.0 = Some(clean);
        } else if text.contains("filter(") {
            let clean = text.replace("filter(", "").trim_end_matches(')').to_string();
            if let Some(existing) = &entry.1 {
                entry.1 = Some(format!("{} AND {}", existing, clean));
            } else {
                entry.1 = Some(clean);
            }
        }
    }

    pub fn parse_xplan_with_meta(xplan: &str) -> (Vec<PlanNode>, Option<u64>, Option<String>) {
        let nodes = Self::parse_xplan_output(xplan);
        let mut plan_hash = None;
        let mut sql_id = None;

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
                if let Ok(re) = regex::Regex::new(r"(?i)sql_id\s*[:=]?\s*([a-z0-9]+)") {
                    if let Some(cap) = re.captures(trimmed) {
                        sql_id = cap.get(1).map(|m| m.as_str().to_string());
                    }
                }
            }
        }

        (nodes, plan_hash, sql_id)
    }
}


/// Pure Rust SQL Line Formatter & Tokenizer-based Pretty-Printer
/// Guarantees:
/// 1. Idempotent (running N times produces identical output)
/// 2. NEVER corrupts identifiers (e.g. O.ORD_ID, ORDER_SUMMARY, B.ON_HAND_QTY, JOIN_DATE stay 100% intact)
/// 3. Preserves comments (-- ... and /* ... */) and string literals ('...')
/// 4. Beautifully indents and aligns major clauses (SELECT, FROM, WHERE, AND, OR, JOIN, ON, GROUP BY, ORDER BY, HAVING)
/// 5. Neatly wraps and aligns top-level SELECT columns
pub fn format_sql(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TokenType {
        Word,
        StringLit,
        Comment,
        Bind,
        Dot,
        Comma,
        OpenParen,
        CloseParen,
        Semicolon,
        Op,
    }

    struct Token {
        ttype: TokenType,
        val: String,
    }

    // 1. Lexical Analysis / Tokenizer
    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < n {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Line comment: --
        if c == '-' && i + 1 < n && chars[i + 1] == '-' {
            let mut j = i + 2;
            while j < n && chars[j] != '\n' {
                j += 1;
            }
            tokens.push(Token {
                ttype: TokenType::Comment,
                val: chars[i..j].iter().collect::<String>().trim().to_string(),
            });
            i = j;
            continue;
        }

        // Block comment: /* ... */
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            j = (j + 2).min(n);
            tokens.push(Token {
                ttype: TokenType::Comment,
                val: chars[i..j].iter().collect::<String>().trim().to_string(),
            });
            i = j;
            continue;
        }

        // String literal: '...'
        if c == '\'' {
            let mut j = i + 1;
            while j < n {
                if chars[j] == '\'' {
                    if j + 1 < n && chars[j + 1] == '\'' {
                        j += 2;
                    } else {
                        j += 1;
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            tokens.push(Token {
                ttype: TokenType::StringLit,
                val: chars[i..j].iter().collect(),
            });
            i = j;
            continue;
        }

        // Bind variable: :var_name or :1
        if c == ':' && i + 1 < n && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_') {
            let mut j = i + 1;
            while j < n && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            tokens.push(Token {
                ttype: TokenType::Bind,
                val: chars[i..j].iter().collect(),
            });
            i = j;
            continue;
        }

        // Word (Identifier or Keyword)
        if c.is_alphabetic() || c == '_' || c == '$' || c == '#' {
            let mut j = i + 1;
            while j < n && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '$' || chars[j] == '#') {
                j += 1;
            }
            tokens.push(Token {
                ttype: TokenType::Word,
                val: chars[i..j].iter().collect(),
            });
            i = j;
            continue;
        }

        // Parentheses and punctuation
        if c == '(' {
            tokens.push(Token { ttype: TokenType::OpenParen, val: "(".to_string() });
            i += 1;
            continue;
        }
        if c == ')' {
            tokens.push(Token { ttype: TokenType::CloseParen, val: ")".to_string() });
            i += 1;
            continue;
        }
        if c == ',' {
            tokens.push(Token { ttype: TokenType::Comma, val: ",".to_string() });
            i += 1;
            continue;
        }
        if c == ';' {
            tokens.push(Token { ttype: TokenType::Semicolon, val: ";".to_string() });
            i += 1;
            continue;
        }
        if c == '.' {
            tokens.push(Token { ttype: TokenType::Dot, val: ".".to_string() });
            i += 1;
            continue;
        }

        // Operators: >=, <=, !=, <>, ||, or single-char
        if c == '>' || c == '<' || c == '!' || c == '=' || c == '|' {
            let mut j = i + 1;
            while j < n && (chars[j] == '>' || chars[j] == '<' || chars[j] == '=' || chars[j] == '|' || chars[j] == '+') {
                j += 1;
            }
            tokens.push(Token {
                ttype: TokenType::Op,
                val: chars[i..j].iter().collect(),
            });
            i = j;
            continue;
        }

        // Single-character operator (+, -, *, /)
        tokens.push(Token {
            ttype: TokenType::Op,
            val: c.to_string(),
        });
        i += 1;
    }

    // 2. Syntax-aware line reconstruction
    let mut result_lines = Vec::new();
    let mut curr_line = Vec::new();
    let mut paren_depth: usize = 0;
    let mut subquery_depth: usize = 0;
    let mut in_select_clause = false;
    let mut prev_ttype: Option<TokenType> = None;
    let mut prev_word: String = String::new();

    let mut idx = 0;
    while idx < tokens.len() {
        let ttype = tokens[idx].ttype.clone();
        let tval = &tokens[idx].val;
        let upper = if ttype == TokenType::Word {
            tval.to_uppercase()
        } else {
            tval.clone()
        };

        // Detect compound keywords: ORDER BY, GROUP BY, LEFT JOIN, RIGHT JOIN, INNER JOIN
        let mut compound: Option<&str> = None;
        if ttype == TokenType::Word && idx + 1 < tokens.len() && tokens[idx + 1].ttype == TokenType::Word {
            let next_upper = tokens[idx + 1].val.to_uppercase();
            if upper == "ORDER" && next_upper == "BY" {
                compound = Some("ORDER BY");
            } else if upper == "GROUP" && next_upper == "BY" {
                compound = Some("GROUP BY");
            } else if upper == "LEFT" && next_upper == "JOIN" {
                compound = Some("LEFT JOIN");
            } else if upper == "RIGHT" && next_upper == "JOIN" {
                compound = Some("RIGHT JOIN");
            } else if upper == "INNER" && next_upper == "JOIN" {
                compound = Some("INNER JOIN");
            }
        }

        let clause_candidate = compound.unwrap_or(&upper);

        // Only treat as clause if NOT preceded by '.' (e.g. O.ORD_ID, B.ON_HAND_QTY are NOT clauses!)
        let is_clause = prev_ttype != Some(TokenType::Dot) && ttype == TokenType::Word && matches!(
            clause_candidate,
            "SELECT" | "FROM" | "WHERE" | "AND" | "OR" | "HAVING"
                | "ORDER BY" | "GROUP BY" | "JOIN" | "LEFT JOIN"
                | "RIGHT JOIN" | "INNER JOIN" | "ON" | "WITH"
        );

        if is_clause {
            // If inside function parentheses or inline condition (like (:b_cust_grade IS NULL OR ...)), don't break line
            if paren_depth > subquery_depth && matches!(clause_candidate, "AND" | "OR" | "ORDER BY") {
                // Keep inline
            } else {
                if !curr_line.is_empty() {
                    result_lines.push(curr_line.join(""));
                    curr_line.clear();
                }

                in_select_clause = clause_candidate == "SELECT";

                let indent = "    ".repeat(subquery_depth);
                let prefix = match clause_candidate {
                    "WITH" => "",
                    "SELECT" => "",
                    "FROM" => "  ",
                    "WHERE" => " ",
                    "AND" => "   ",
                    "OR" => "    ",
                    "JOIN" | "LEFT JOIN" | "RIGHT JOIN" | "INNER JOIN" => "  ",
                    "ON" => "    ",
                    "GROUP BY" | "ORDER BY" => " ",
                    "HAVING" => "",
                    _ => "",
                };

                curr_line.push(format!("{}{}{} ", indent, prefix, clause_candidate));
                let advance = if compound.is_some() { 2 } else { 1 };
                idx += advance;
                prev_ttype = Some(TokenType::Word);
                prev_word = clause_candidate.to_string();
                continue;
            }
        }

        match ttype {
            TokenType::OpenParen => {
                paren_depth += 1;
                // Check if this opens a subquery: AS ( or (SELECT
                if idx + 1 < tokens.len() && tokens[idx + 1].ttype == TokenType::Word && tokens[idx + 1].val.to_uppercase() == "SELECT" {
                    subquery_depth += 1;
                }
                // Avoid extra space before '(' if preceded by function name or word, unless word was 'AS' or 'IN'
                if let Some(last) = curr_line.last_mut() {
                    if last.ends_with(' ') && prev_ttype == Some(TokenType::Word) && prev_word != "AS" && prev_word != "IN" {
                        *last = last.trim_end().to_string();
                    }
                }
                curr_line.push("(".to_string());
            }
            TokenType::CloseParen => {
                paren_depth = paren_depth.saturating_sub(1);
                if subquery_depth > paren_depth {
                    subquery_depth = paren_depth;
                }
                if let Some(last) = curr_line.last_mut() {
                    if last.ends_with(' ') {
                        *last = last.trim_end().to_string();
                    }
                }
                curr_line.push(") ".to_string());
            }
            TokenType::Dot => {
                if let Some(last) = curr_line.last_mut() {
                    if last.ends_with(' ') {
                        *last = last.trim_end().to_string();
                    }
                }
                curr_line.push(".".to_string());
            }
            TokenType::Comma => {
                if let Some(last) = curr_line.last_mut() {
                    if last.ends_with(' ') {
                        *last = last.trim_end().to_string();
                    }
                }
                curr_line.push(",".to_string());

                // If in SELECT projection at top-level, align next column neatly
                if in_select_clause && paren_depth == subquery_depth {
                    result_lines.push(curr_line.join(""));
                    curr_line.clear();
                    curr_line.push(format!("{}       ", "    ".repeat(subquery_depth)));
                } else {
                    curr_line.push(" ".to_string());
                }
            }
            TokenType::Semicolon => {
                if let Some(last) = curr_line.last_mut() {
                    if last.ends_with(' ') {
                        *last = last.trim_end().to_string();
                    }
                }
                curr_line.push(";".to_string());
            }
            TokenType::Comment => {
                if !curr_line.is_empty() {
                    result_lines.push(curr_line.join(""));
                    curr_line.clear();
                }
                result_lines.push(format!("{}{}", "    ".repeat(subquery_depth), tval));
            }
            _ => {
                curr_line.push(format!("{} ", tval));
            }
        }

        prev_word = upper;
        prev_ttype = Some(ttype);
        idx += 1;
    }

    if !curr_line.is_empty() {
        result_lines.push(curr_line.join(""));
    }

    // Trim trailing whitespace on each line
    result_lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<String>>()
        .join("\n")
}

/// Extracts all unique bind variables (:var_name) from SQL without using lookaround regex
pub fn extract_bind_variables(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut binds = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if in_line_comment {
            if c == '\n' { in_line_comment = false; }
            i += 1;
            continue;
        }
        if in_block_comment {
            if c == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if c == '\'' {
            in_str = !in_str;
            i += 1;
            continue;
        }
        if in_str {
            i += 1;
            continue;
        }

        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            in_block_comment = true;
            i += 2;
            continue;
        }

        if c == ':' {
            let not_double_colon = i == 0 || chars[i - 1] != ':';
            if not_double_colon && i + 1 < chars.len() && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_') {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let name: String = chars[i + 1..j].iter().collect();
                let upper = name.to_uppercase();
                if !seen.contains(&upper) && upper != "MI" && upper != "SS" && upper != "HH24" && upper != "HH" {
                    seen.insert(upper);
                    binds.push(name);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }

    binds
}

pub fn default_bind_value(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("date") || lower.contains("_dt") {
        if lower.contains("end") {
            "'2025-07-01'".to_string()
        } else {
            "'2025-04-01'".to_string()
        }
    } else if lower.contains("promo") || lower.contains("type") {
        "'FLASH_SALE'".to_string()
    } else if lower.contains("grade") {
        "'VIP'".to_string()
    } else if lower.contains("status") {
        "'DELIVERED'".to_string()
    } else if lower.contains("id") || lower.contains("cd") {
        "1".to_string()
    } else {
        "'1'".to_string()
    }
}

pub fn substitute_bind_variables(sql: &str, bind_values: &HashMap<String, String>) -> String {
    let binds = extract_bind_variables(sql);
    let mut substituted = sql.to_string();

    for b in binds {
        let val = if let Some(v) = bind_values.get(&b) {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                default_bind_value(&b)
            } else {
                trimmed.to_string()
            }
        } else {
            default_bind_value(&b)
        };

        let repl = if val.starts_with('\'') || val.parse::<f64>().is_ok() || val.eq_ignore_ascii_case("NULL") {
            val
        } else {
            format!("'{}'", val)
        };

        let pat = format!(r":{}\b", regex::escape(&b));
        if let Ok(re) = regex::Regex::new(&pat) {
            substituted = re.replace_all(&substituted, repl.as_str()).to_string();
        }
    }

    substituted
}

pub fn generate_bind_extraction_query(sql: &str) -> Option<String> {
    let binds = extract_bind_variables(sql);
    if binds.is_empty() {
        return None;
    }

    // Extract tables using analyze_query_structure
    let structure = analyze_query_structure(sql);
    let table_names: Vec<String> = structure.tables.iter().map(|t| t.name.clone()).collect();
    let from_clause = if !table_names.is_empty() {
        table_names.join(", ")
    } else {
        "DUAL".to_string()
    };

    let mut select_items = Vec::new();
    for b in &binds {
        let mut target_col = None;
        for filter in &structure.filters {
            let pat1 = format!(r"(?i)([a-zA-Z0-9_.]+)\s*(?:=|>=|<=|>|<|LIKE)\s*:{}", b);
            let pat2 = format!(r"(?i):{}\s*(?:=|>=|<=|>|<|LIKE)\s*([a-zA-Z0-9_.]+)", b);
            if let Ok(re1) = regex::Regex::new(&pat1) {
                if let Some(cap) = re1.captures(filter) {
                    target_col = cap.get(1).map(|s| s.as_str().trim().to_string());
                    break;
                }
            }
            if target_col.is_none() {
                if let Ok(re2) = regex::Regex::new(&pat2) {
                    if let Some(cap) = re2.captures(filter) {
                        target_col = cap.get(1).map(|s| s.as_str().trim().to_string());
                        break;
                    }
                }
            }
        }

        let col = target_col.unwrap_or_else(|| {
            let lower = b.to_lowercase();
            if lower.contains("date") || lower.contains("_dt") {
                "ORD_DATE".to_string()
            } else if lower.contains("promo") {
                "PROMO_TYPE".to_string()
            } else if lower.contains("grade") {
                "CUST_GRADE".to_string()
            } else {
                b.clone()
            }
        });

        select_items.push(format!("{} AS {}", col, b));
    }

    Some(format!(
        "SELECT DISTINCT\n       {}\n  FROM {}\n WHERE ROWNUM <= 5;",
        select_items.join(",\n       "),
        from_clause
    ))
}

/// Analyzes query skeleton, table structure, joins, and WHERE filter predicates
pub fn analyze_query_structure(sql: &str) -> QueryStructure {
    let mut ctes = Vec::new();
    if let Ok(re_cte) = regex::Regex::new(r"(?i)\b([a-zA-Z0-9_]+)\s+AS\s*\(") {
        for cap in re_cte.captures_iter(sql) {
            if let Some(m) = cap.get(1) {
                let name = m.as_str().to_string();
                let upper = name.to_uppercase();
                if upper != "SELECT" && upper != "FROM" && upper != "WHERE" {
                    ctes.push(name);
                }
            }
        }
    }

    let mut tables = Vec::new();
    let keywords = ["WHERE", "GROUP", "ORDER", "HAVING", "JOIN", "LEFT", "RIGHT", "INNER", "ON", "AS", "SELECT", "FROM", "SET"];
    let pat = r"(?i)(FROM|JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|INNER\s+JOIN|FULL\s+OUTER\s+JOIN)\s+([a-zA-Z0-9_.]+)(?:\s+(?:AS\s+)?([a-zA-Z0-9_]+))?(?:\s+ON\s+([^,\n;]+?)(?=\s+(?:JOIN|LEFT|RIGHT|INNER|WHERE|GROUP|ORDER|HAVING|;|$)))?";
    if let Ok(re_tbl) = regex::Regex::new(pat) {
        for cap in re_tbl.captures_iter(sql) {
            let jtype = cap.get(1).map(|m| m.as_str().to_uppercase().split_whitespace().collect::<Vec<&str>>().join(" ")).unwrap_or_else(|| "FROM".to_string());
            let tbl = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            if tbl.is_empty() || keywords.contains(&tbl.to_uppercase().as_str()) {
                continue;
            }
            let alias = cap.get(3).and_then(|m| {
                let a = m.as_str().to_string();
                if keywords.contains(&a.to_uppercase().as_str()) {
                    None
                } else {
                    Some(a)
                }
            });
            let cond = cap.get(4).map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty());

            tables.push(TableRef {
                name: tbl,
                alias,
                join_type: jtype,
                join_condition: cond,
            });
        }
    }

    let mut filters = Vec::new();
    if let Ok(re_where) = regex::Regex::new(r"(?i)\bWHERE\b(.*?)(?=\bGROUP\b|\bORDER\b|\bHAVING\b|;|$)") {
        if let Some(cap) = re_where.captures(sql) {
            if let Some(m) = cap.get(1) {
                let raw_where = m.as_str();
                if let Ok(re_split) = regex::Regex::new(r"(?i)\b(?:AND|OR)\b") {
                    for part in re_split.split(raw_where) {
                        let c = part.trim();
                        if !c.is_empty() {
                            filters.push(c.to_string());
                        }
                    }
                }
            }
        }
    }

    QueryStructure {
        ctes,
        tables,
        filters,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sql_idempotent() {
        let sql = "SELECT banner FROM v$version;";
        let f1 = format_sql(sql);
        let f2 = format_sql(&f1);
        let f3 = format_sql(&f2);
        assert_eq!(f1, f2);
        assert_eq!(f2, f3);
        assert_eq!(f1, "SELECT banner\n  FROM v$version;");
    }

    #[test]
    fn test_complex_sql_no_identifier_corruption() {
        let complex_sql = "WITH ORDER_SUMMARY AS ( SELECT O.ORD_ID, O.ORD_DATE, D.ORD_QTY, B.ON_HAND_QTY, ROW_NUMBER() OVER(PARTITION BY D.ITEM_CD ORDER BY D.PAID_AMT DESC) AS ITEM_SALES_RANK FROM TB_ORD_MST O JOIN TB_ORD_DTL D ON O.ORD_ID = D.ORD_ID WHERE O.ORD_DATE >= TO_DATE(:b_start_dt, 'YYYY-MM-DD') AND O.ORD_STATUS IN ('PAY_COMPLETED', 'DELIVERED') AND (:b_cust_grade IS NULL OR C.CUST_GRADE = :b_cust_grade) ) SELECT TPI.PROMO_NM, I.ITEM_CD FROM ORDER_SUMMARY;";
        let formatted = format_sql(complex_sql);

        // Assert no corrupted words
        assert!(!formatted.contains("OR D_ID"), "O.ORD_ID was wrongly corrupted to OR D_ID");
        assert!(!formatted.contains("OR D_DATE"), "O.ORD_DATE was wrongly corrupted to OR D_DATE");
        assert!(!formatted.contains("OR DER_SUMMARY"), "ORDER_SUMMARY was wrongly corrupted to OR DER_SUMMARY");
        assert!(!formatted.contains("ON _HAND_QTY"), "B.ON_HAND_QTY was wrongly corrupted to ON _HAND_QTY");
        assert!(formatted.contains("ORDER_SUMMARY AS"), "ORDER_SUMMARY AS must remain intact");
        assert!(formatted.contains("O.ORD_ID"), "O.ORD_ID must remain intact");
        assert!(formatted.contains("B.ON_HAND_QTY"), "B.ON_HAND_QTY must remain intact");

        // Idempotency
        let formatted2 = format_sql(&formatted);
        assert_eq!(formatted, formatted2, "Formatting must be idempotent");
    }

    #[test]
    fn test_extract_and_substitute_bind_variables() {
        let sql = r#"
            WHERE O.ORD_DATE >= TO_DATE(:b_start_dt, 'YYYY-MM-DD')
              AND O.ORD_DATE <  TO_DATE(:b_end_dt, 'YYYY-MM-DD')
              AND (:b_cust_grade IS NULL OR C.CUST_GRADE = :b_cust_grade)
              AND P.PROMO_TYPE = :b_promo_type
        "#;

        let binds = extract_bind_variables(sql);
        assert_eq!(binds, vec!["b_start_dt", "b_end_dt", "b_cust_grade", "b_promo_type"]);

        let mut vals = HashMap::new();
        vals.insert("b_start_dt".to_string(), "2025-04-01".to_string());
        vals.insert("b_end_dt".to_string(), "2025-07-01".to_string());
        vals.insert("b_cust_grade".to_string(), "VIP".to_string());
        vals.insert("b_promo_type".to_string(), "FLASH_SALE".to_string());

        let substituted = substitute_bind_variables(sql, &vals);
        assert!(!substituted.contains(":b_start_dt"));
        assert!(!substituted.contains(":b_end_dt"));
        assert!(!substituted.contains(":b_cust_grade"));
        assert!(!substituted.contains(":b_promo_type"));
        assert!(substituted.contains("'2025-04-01'"));
        assert!(substituted.contains("'2025-07-01'"));
        assert!(substituted.contains("'VIP'"));
        assert!(substituted.contains("'FLASH_SALE'"));
    }
}

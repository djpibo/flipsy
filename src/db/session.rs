pub use super::sql_utils::*;

use crate::models::{ConnectionConfig, PlanNode, QueryResult};
use crate::db::mock::MockEngine;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
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
    // 1번 방식: EXPLAIN PLAN FOR (F10)
    pub last_explain_plan: Option<Vec<PlanNode>>,
    pub last_explain_sql_id: Option<String>,
    pub last_explain_hash: Option<u64>,
    // 2번 방식: DBMS.XPLAN (Ctrl+Enter)
    pub last_xplan: Option<Vec<PlanNode>>,
    pub last_xplan_sql_id: Option<String>,
    pub last_xplan_hash: Option<u64>,
    // Generic / Backward Compatibility
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
            last_explain_plan: None,
            last_explain_sql_id: None,
            last_explain_hash: None,
            last_xplan: None,
            last_xplan_sql_id: None,
            last_xplan_hash: None,
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
        self.last_explain_plan = None;
        self.last_explain_sql_id = None;
        self.last_explain_hash = None;
        self.last_xplan = None;
        self.last_xplan_sql_id = None;
        self.last_xplan_hash = None;
        self.last_plan = None;
        self.last_sql_id = None;
        self.last_plan_hash = None;
    }

    fn build_sqlplus_command(conn_str: &str) -> Command {
        // 1. If user prefers local sqlplus CLI directly (e.g. Instant Client)
        if std::env::var("FLIPSY_USE_LOCAL_SQLPLUS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            let mut cmd = Command::new("sqlplus");
            cmd.args(["-L", "-s", conn_str]);
            return cmd;
        }

        // 2. Default: Docker exec into container (container name configurable via env)
        let container = std::env::var("FLIPSY_DOCKER_CONTAINER")
            .or_else(|_| std::env::var("FLIPSY_ORACLE_CONTAINER"))
            .unwrap_or_else(|_| "oracle23ai".to_string());

        let mut cmd = Command::new("docker");
        cmd.args(["exec", "-i", &container, "sqlplus", "-L", "-s", conn_str]);
        cmd
    }

    fn run_sqlplus_command(conn_str: &str, script: &str) -> Result<String, String> {
        let mut child = Self::build_sqlplus_command(conn_str)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Oracle 프로세스 실행 실패: {}", e))?;

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
        let mut child = Self::build_sqlplus_command(conn_str)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Oracle 프로세스 실행 실패: {}", e))?;

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

                let clean_sql = sql_string.trim().trim_end_matches(';').trim();

                if is_explain {
                    // 1번 방식: EXPLAIN PLAN FOR (옵티마이저 예측 실행계획)
                    let plan_script = format!(
                        "SET TAB OFF\nEXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'TYPICAL +COST +PREDICATE +ALIAS'));\nEXIT;\n",
                        clean_sql
                    );

                    let plan_res = DatabaseSession::run_sqlplus_streaming(&conn_str, &plan_script, &tracker_clone);
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

                    let (computed_id, computed_hash) = compute_oracle_sql_id(&sql_string);
                    match plan_res {
                        Ok(plan_output) => {
                            let (nodes, hash, sql_id) = DatabaseSession::parse_xplan_with_meta(&plan_output);
                            let final_hash = hash.or(Some(computed_hash as u64));
                            let final_sql_id = sql_id.unwrap_or(computed_id);
                            let res = QueryResult {
                                columns: vec!["PLAN_TABLE_OUTPUT".to_string()],
                                rows: plan_output.lines().map(|l| vec![l.to_string()]).collect(),
                                elapsed_ms: elapsed,
                                row_count: nodes.len(),
                                sql_id: Some(final_sql_id.clone()),
                                child_number: Some(0),
                                plan_hash_value: final_hash,
                                message: Some(format!("Oracle EXPLAIN PLAN FOR 완료 ({:.2}ms)", elapsed)),
                            };
                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes: Some(nodes),
                                plan_hash: final_hash,
                                sql_id: Some(final_sql_id),
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
                                message: Some(format!("EXPLAIN PLAN FOR 오류: {}", err)),
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
                    // 2번 방식: DBMS.XPLAN (동일 세션 단일 파이프라인으로 쿼리 실행 및 DISPLAY_CURSOR 일괄 수집)
                    let combined_script = format!(
                        "SET TAB OFF\nSET FEEDBACK OFF\nALTER SESSION SET STATISTICS_LEVEL = ALL;\nSET MARKUP CSV ON QUOTE ON\nSET HEADING ON\nSET FEEDBACK ON SQL_ID\nSET PAGESIZE 50000\nSET LINESIZE 32767\n{};\nSET MARKUP CSV OFF\nSET FEEDBACK OFF\nPROMPT ===FLIPSY_XPLAN_START===\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY_CURSOR(NULL, NULL, 'ALLSTATS LAST +COST +OUTLINE +PREDICATE +ALIAS'));\nPROMPT ===FLIPSY_XPLAN_END===\nEXIT;\n",
                        clean_sql
                    );

                    match DatabaseSession::run_sqlplus_streaming(&conn_str, &combined_script, &tracker_clone) {
                        Ok(full_output) => {
                            let elapsed = start.elapsed().as_secs_f64() * 1000.0;

                            // Split CSV output and XPLAN section
                            let (csv_part, xplan_part) = if let Some(idx) = full_output.find("===FLIPSY_XPLAN_START===") {
                                let csv = &full_output[..idx];
                                let rest = &full_output[idx + "===FLIPSY_XPLAN_START===".len()..];
                                let xplan = if let Some(end_idx) = rest.find("===FLIPSY_XPLAN_END===") {
                                    &rest[..end_idx]
                                } else {
                                    rest
                                };
                                (csv, Some(xplan))
                            } else {
                                (full_output.as_str(), None)
                            };

                            let (cols, rows, parsed_sql_id) = DatabaseSession::parse_csv_output_with_sql_id(csv_part);

                            let (computed_id, computed_hash) = compute_oracle_sql_id(clean_sql);
                            let real_sql_id = parsed_sql_id.unwrap_or(computed_id);

                            let mut plan_nodes = None;
                            let mut plan_hash = None;

                            if let Some(xplan_text) = xplan_part {
                                let (p, h, _) = DatabaseSession::parse_xplan_with_meta(xplan_text);
                                if !p.is_empty() {
                                    plan_nodes = Some(p);
                                    plan_hash = h.or(Some(computed_hash as u64));
                                }
                            }

                            // Fallback to EXPLAIN PLAN only if DISPLAY_CURSOR had no entries
                            if plan_nodes.is_none() {
                                let fallback_script = format!(
                                    "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                                    clean_sql
                                );
                                if let Ok(fb_out) = DatabaseSession::run_sqlplus_command(&conn_str, &fallback_script) {
                                    let (p_fb, h_fb, _) = DatabaseSession::parse_xplan_with_meta(&fb_out);
                                    if !p_fb.is_empty() {
                                        plan_nodes = Some(p_fb);
                                        plan_hash = h_fb.or(Some(computed_hash as u64));
                                    }
                                }
                            }

                            if plan_hash.is_none() {
                                plan_hash = Some(computed_hash as u64);
                            }

                            let row_count = rows.len();
                            let total_bytes = tracker_clone.bytes_read.load(Ordering::Relaxed);
                            let mb = total_bytes as f64 / (1024.0 * 1024.0);

                            let size_desc = if mb >= 1.0 {
                                format!("{:.2} MB", mb)
                            } else {
                                format!("{:.1} KB", total_bytes as f64 / 1024.0)
                            };

                            let limit_notice = if row_count >= 10_000 { " (메모리 보호 상한 10,000건 적용)" } else { "" };
                            let message_text = format!(
                                "Oracle Live ({}@{}) - {}건 인출{} ({}) in {:.2}ms",
                                config.username, config.service_name, row_count, limit_notice, size_desc, elapsed
                            );

                            let res = QueryResult {
                                columns: cols,
                                rows,
                                elapsed_ms: elapsed,
                                row_count,
                                sql_id: Some(real_sql_id.clone()),
                                child_number: Some(0),
                                plan_hash_value: plan_hash,
                                message: Some(message_text),
                            };

                            let _ = tx.send(ExecutionResult {
                                query_result: res,
                                plan_nodes,
                                plan_hash,
                                sql_id: Some(real_sql_id),
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
                let (computed_id, computed_hash) = compute_oracle_sql_id(&sql_string);
                let nodes = if is_explain {
                    plan_nodes.into_iter().map(|mut n| {
                        n.starts = 0;
                        n.a_rows = 0;
                        n.buffers = 0;
                        n.reads = 0;
                        n.a_time_ms = 0.0;
                        n
                    }).collect()
                } else {
                    plan_nodes
                };
                let _ = tx.send(ExecutionResult {
                    query_result: query_res,
                    plan_nodes: Some(nodes),
                    plan_hash: Some(computed_hash as u64),
                    sql_id: Some(computed_id),
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
        let (computed_id, computed_hash) = compute_oracle_sql_id(sql);
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
                    self.last_plan_hash = hash.or(Some(computed_hash as u64));
                    self.last_sql_id = Some(sql_id.unwrap_or(computed_id));
                    return;
                }
            }
        }

        let (_, mock_plan) = MockEngine::execute(sql);
        self.last_plan = Some(mock_plan);
        self.last_plan_hash = Some(computed_hash as u64);
        self.last_sql_id = Some(computed_id);
    }

    #[allow(dead_code)]
    pub fn execute(&mut self, sql: &str) -> QueryResult {
        let (computed_id, computed_hash) = compute_oracle_sql_id(sql);
        if self.is_real_oracle {
            let start = Instant::now();
            let conn_str = format!(
                "{}/{}@{}:{}/{}",
                self.config.username, self.config.password, self.config.host, self.config.port, self.config.service_name
            );

            let clean_sql = sql.trim().trim_end_matches(';');
            let combined_script = format!(
                "SET TAB OFF\nSET FEEDBACK OFF\nALTER SESSION SET STATISTICS_LEVEL = ALL;\nSET MARKUP CSV ON QUOTE ON\nSET HEADING ON\nSET FEEDBACK ON SQL_ID\nSET PAGESIZE 50000\nSET LINESIZE 32767\n{};\nSET MARKUP CSV OFF\nSET FEEDBACK OFF\nPROMPT ===FLIPSY_XPLAN_START===\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY_CURSOR(NULL, NULL, 'ALLSTATS LAST +COST +OUTLINE +PREDICATE +ALIAS'));\nPROMPT ===FLIPSY_XPLAN_END===\nEXIT;\n",
                clean_sql
            );

            match Self::run_sqlplus_command(&conn_str, &combined_script) {
                Ok(full_output) => {
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    let (csv_part, xplan_part) = if let Some(idx) = full_output.find("===FLIPSY_XPLAN_START===") {
                        let csv = &full_output[..idx];
                        let rest = &full_output[idx + "===FLIPSY_XPLAN_START===".len()..];
                        let xplan = if let Some(end_idx) = rest.find("===FLIPSY_XPLAN_END===") {
                            &rest[..end_idx]
                        } else {
                            rest
                        };
                        (csv, Some(xplan))
                    } else {
                        (full_output.as_str(), None)
                    };

                    let (cols, rows, parsed_sql_id) = Self::parse_csv_output_with_sql_id(csv_part);
                    let real_sql_id = parsed_sql_id.unwrap_or(computed_id);

                    let mut plan_nodes = None;
                    let mut plan_hash = None;

                    if let Some(xplan_text) = xplan_part {
                        let (p, h, _) = Self::parse_xplan_with_meta(xplan_text);
                        if !p.is_empty() {
                            plan_nodes = Some(p);
                            plan_hash = h.or(Some(computed_hash as u64));
                        }
                    }

                    if plan_nodes.is_none() {
                        let fallback_script = format!(
                            "EXPLAIN PLAN FOR {};\nSET PAGESIZE 50000\nSET LINESIZE 32767\nSELECT PLAN_TABLE_OUTPUT FROM TABLE(DBMS_XPLAN.DISPLAY('PLAN_TABLE', NULL, 'ALL +OUTLINE +PREDICATE +ALIAS'));\nEXIT;\n",
                            clean_sql
                        );
                        if let Ok(fb_out) = Self::run_sqlplus_command(&conn_str, &fallback_script) {
                            let (p_fb, h_fb, _) = Self::parse_xplan_with_meta(&fb_out);
                            if !p_fb.is_empty() {
                                plan_nodes = Some(p_fb);
                                plan_hash = h_fb.or(Some(computed_hash as u64));
                            }
                        }
                    }

                    self.last_plan = plan_nodes;
                    self.last_plan_hash = plan_hash.or(Some(computed_hash as u64));
                    self.last_sql_id = Some(real_sql_id.clone());

                    let row_count = rows.len();
                    let res = QueryResult {
                        columns: cols,
                        rows,
                        elapsed_ms: elapsed,
                        row_count,
                        sql_id: Some(real_sql_id.clone()),
                        child_number: Some(0),
                        plan_hash_value: self.last_plan_hash,
                        message: Some(format!(
                            "Oracle 26ai Live ({}@{}) - {} rows in {:.2}ms",
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
        let (computed_id, computed_hash) = compute_oracle_sql_id(sql);
        self.last_query_result = Some(result.clone());
        self.last_plan = Some(plan);
        self.last_plan_hash = Some(computed_hash as u64);
        self.last_sql_id = Some(computed_id);
        result
    }

    

    // --- Compatibility Delegators to Specialized Submodules ---

    #[inline]
    pub fn parse_csv_output_with_sql_id(csv: &str) -> (Vec<String>, Vec<Vec<String>>, Option<String>) {
        super::csv_parser::parse_csv_output_with_sql_id(csv)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn parse_csv_output(csv: &str) -> (Vec<String>, Vec<Vec<String>>) {
        super::csv_parser::parse_csv_output(csv)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn parse_xplan_output(xplan: &str) -> Vec<crate::models::PlanNode> {
        super::xplan::parse_xplan_output(xplan)
    }

    #[inline]
    pub fn parse_xplan_with_meta(xplan: &str) -> (Vec<crate::models::PlanNode>, Option<u64>, Option<String>) {
        super::xplan::parse_xplan_with_meta(xplan)
    }
}


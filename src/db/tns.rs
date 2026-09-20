use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TnsEntry {
    pub alias: String,
    pub host: String,
    pub port: u16,
    pub service_name: String,
    #[allow(dead_code)]
    pub raw_block: String,
}

pub struct TnsManager {
    pub file_path: PathBuf,
    pub entries: Vec<TnsEntry>,
    pub raw_content: String,
}

impl TnsManager {
    pub fn new() -> Self {
        let file_path = PathBuf::from(r"C:\project\flipsy\tnsnames.ora");
        let mut manager = Self {
            file_path,
            entries: Vec::new(),
            raw_content: String::new(),
        };
        manager.reload();
        manager
    }

    pub fn reload(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.file_path) {
            self.raw_content = content.clone();
            self.entries = Self::parse_tns(&content);
        } else {
            // Default content
            self.raw_content = String::new();
            self.entries.clear();
        }
    }

    pub fn save(&mut self, new_content: &str) -> Result<(), String> {
        fs::write(&self.file_path, new_content)
            .map_err(|e| format!("tnsnames.ora 저장 실패: {}", e))?;
        self.raw_content = new_content.to_string();
        self.entries = Self::parse_tns(new_content);
        Ok(())
    }

    pub fn parse_tns(text: &str) -> Vec<TnsEntry> {
        let mut entries = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with('#') || line.is_empty() {
                i += 1;
                continue;
            }

            if line.contains('=') && !line.starts_with('(') {
                if let Some(eq_idx) = line.find('=') {
                    let alias = line[..eq_idx].trim().to_string();
                    if !alias.is_empty() && !alias.starts_with('(') {
                        // Collect lines until next top-level entry
                        let mut block_lines = vec![lines[i]];
                        i += 1;
                        let mut depth = 0;
                        for ch in line.chars() {
                            if ch == '(' { depth += 1; }
                            if ch == ')' { depth -= 1; }
                        }

                        while i < lines.len() {
                            let next_line = lines[i];
                            let next_trim = next_line.trim();
                            if next_trim.is_empty() || next_trim.starts_with('#') {
                                block_lines.push(next_line);
                                i += 1;
                                continue;
                            }

                            // If next line starts an alias at column 0 and depth <= 0
                            if !next_line.starts_with(' ') && !next_line.starts_with('\t') && next_trim.contains('=') && depth <= 0 {
                                break;
                            }

                            for ch in next_trim.chars() {
                                if ch == '(' { depth += 1; }
                                if ch == ')' { depth -= 1; }
                            }
                            block_lines.push(next_line);
                            i += 1;
                            if depth == 0 && next_trim.ends_with(')') {
                                break;
                            }
                        }

                        let raw_block = block_lines.join("\n");
                        let (host, port, service_name) = Self::extract_params(&raw_block, &alias);
                        entries.push(TnsEntry {
                            alias,
                            host,
                            port,
                            service_name,
                            raw_block,
                        });
                        continue;
                    }
                }
            }
            i += 1;
        }

        entries
    }

    fn extract_params(block: &str, default_svc: &str) -> (String, u16, String) {
        let block_upper = block.to_uppercase();
        
        let host = if let Some(idx) = block_upper.find("HOST") {
            Self::extract_value(&block[idx..])
        } else {
            "127.0.0.1".to_string()
        };

        let port_str = if let Some(idx) = block_upper.find("PORT") {
            Self::extract_value(&block[idx..])
        } else {
            "1521".to_string()
        };
        let port = port_str.parse::<u16>().unwrap_or(1521);

        let service_name = if let Some(idx) = block_upper.find("SERVICE_NAME") {
            Self::extract_value(&block[idx..])
        } else if let Some(idx) = block_upper.find("SID") {
            Self::extract_value(&block[idx..])
        } else {
            default_svc.to_string()
        };

        (host, port, service_name)
    }

    fn extract_value(sub: &str) -> String {
        if let Some(eq) = sub.find('=') {
            let after = &sub[eq + 1..];
            let mut val = String::new();
            for ch in after.chars() {
                if ch.is_whitespace() {
                    if !val.is_empty() { break; }
                    continue;
                }
                if ch == ')' || ch == '(' { break; }
                val.push(ch);
            }
            val
        } else {
            String::new()
        }
    }
}

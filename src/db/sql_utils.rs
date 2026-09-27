use crate::models::{QueryStructure, TableRef};
use std::collections::HashMap;

// Pure Rust SQL Utilities: Tokenizer-based Pretty-Printer, Bind Variable Processing,
// Query Analysis, and Oracle SQL_ID MD5 Hasher.
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
    let upper = sql.to_uppercase();
    if let Some(w_idx) = upper.find("WHERE") {
        let after_where = &sql[w_idx + 5..];
        let after_upper = &upper[w_idx + 5..];

        let mut end_pos = after_where.len();
        for keyword in &["GROUP BY", "ORDER BY", "HAVING", ";"] {
            if let Some(pos) = after_upper.find(keyword) {
                if pos < end_pos {
                    end_pos = pos;
                }
            }
        }

        let raw_where = &after_where[..end_pos];
        if let Ok(re_split) = regex::Regex::new(r"(?i)(?:AND|OR)") {
            for part in re_split.split(raw_where) {
                let c = part.trim();
                if !c.is_empty() {
                    filters.push(c.to_string());
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





fn md5_digest(data: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x67452301;
    let mut b: u32 = 0xefcdab89;
    let mut c: u32 = 0x98badcfe;
    let mut d: u32 = 0x10325476;

    let s = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let orig_len_bits = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&orig_len_bits.to_le_bytes());

    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let start = i * 4;
            *word = u32::from_le_bytes([chunk[start], chunk[start + 1], chunk[start + 2], chunk[start + 3]]);
        }

        let mut aa = a;
        let mut bb = b;
        let mut cc = c;
        let mut dd = d;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((bb & cc) | (!bb & dd), i),
                16..=31 => ((dd & bb) | (!dd & cc), (5 * i + 1) % 16),
                32..=47 => (bb ^ cc ^ dd, (3 * i + 5) % 16),
                _ => (cc ^ (bb | !dd), (7 * i) % 16),
            };

            let temp = dd;
            dd = cc;
            cc = bb;
            let sum = aa.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            bb = bb.wrapping_add(sum.rotate_left(s[i]));
            aa = temp;
        }

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
}

pub fn compute_oracle_sql_id(sql: &str) -> (String, u32) {
    let clean = sql.trim().trim_end_matches(';');
    let mut bytes = clean.as_bytes().to_vec();
    bytes.push(0);

    let digest = md5_digest(&bytes);
    let w3 = u32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
    let w4 = u32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);

    let val = ((w3 as u64) << 32) | (w4 as u64);
    let alphabet = b"0123456789abcdfghjkmnpqrstuvwxyz";

    let mut sql_id = String::with_capacity(13);
    for i in 0..13 {
        let shift = 5 * (12 - i);
        let idx = ((val >> shift) & 31) as usize;
        sql_id.push(alphabet[idx] as char);
    }

    (sql_id, w4)
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

    #[test]
    fn test_compute_oracle_sql_id() {
        let sql = "SELECT banner FROM v$version";
        let (sql_id, hash) = compute_oracle_sql_id(sql);
        assert_eq!(sql_id, "40qgt0yy470cn");
        assert_eq!(hash, 3158540692);
    }
}


// SQL*Plus CSV Output Parser & SQL_ID Extractor
//
// Features:
// - Strips SQL*Plus session alter banners, banners, empty lines, row counts.
// - Parses RFC 4180 style CSV with quotes and escapes.
// - Extracts captured SQL_ID from SQL*Plus feedback stream.
// - Applies an in-memory safety limit (10,000 rows max) to prevent client OOM.
pub fn parse_csv_output_with_sql_id(csv: &str) -> (Vec<String>, Vec<Vec<String>>, Option<String>) {
    let mut lines = csv.lines();
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    let mut captured_sql_id = None;

    // 1. Scan for true column header, skipping empty lines, session alter messages, and banners
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if is_metadata_line(trimmed) {
            continue;
        }
        if trimmed.starts_with("SQL_ID:") || trimmed.starts_with("SQL_ID ") {
            if let Some(id) = trimmed.split_whitespace().last() {
                let clean = id.trim_matches(':').trim();
                if clean.len() == 13 {
                    captured_sql_id = Some(clean.to_string());
                }
            }
            continue;
        }

        let parsed_cols = parse_csv_line(trimmed);
        if !parsed_cols.is_empty() && parsed_cols.iter().any(|c| !c.is_empty()) {
            columns = parsed_cols;
            break;
        }
    }

    // 2. Parse data rows following the header
    for line in lines {
        let trimmed = line.trim();
        if is_metadata_line(trimmed) {
            continue;
        }
        if trimmed.starts_with("SQL_ID:") || trimmed.starts_with("SQL_ID ") {
            if let Some(id) = trimmed.split_whitespace().last() {
                let clean = id.trim_matches(':').trim();
                if clean.len() == 13 {
                    captured_sql_id = Some(clean.to_string());
                }
            }
            continue;
        }

        let row = parse_csv_line(trimmed);
        if !row.is_empty() && (columns.is_empty() || row.len() == columns.len()) && rows.len() < 10_000 {
            rows.push(row);
        }
    }

    (columns, rows, captured_sql_id)
}

#[inline]
fn is_metadata_line(trimmed: &str) -> bool {
    trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("Session altered.")
        || trimmed.contains("Session altered")
        || trimmed.contains("PL/SQL procedure successfully completed")
        || trimmed.starts_with("Connected")
        || trimmed.contains("rows selected")
        || trimmed.contains("row selected")
        || trimmed.contains("no rows")
}

#[allow(dead_code)]
pub fn parse_csv_output(csv: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let (cols, rows, _) = parse_csv_output_with_sql_id(csv);
    (cols, rows)
}

pub fn parse_csv_line(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = trimmed.chars().peekable();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_output_with_session_altered() {
        let sample = "
Session altered.

\"CUST_ID\",\"ORD_STATUS\",\"FINAL_PAY_AMT\",\"CUST_NM\",\"CUST_GRADE\",\"CITY\"
\"C001\",\"DELIVERED\",150000,\"Kim\",\"VIP\",\"Seoul\"
\"C002\",\"PAY_COMPLETED\",80000,\"Lee\",\"GOLD\",\"Busan\"

2 rows selected.

SQL_ID: 6ykqdm2dgrdp7
";
        let (cols, rows, sql_id) = parse_csv_output_with_sql_id(sample);
        assert_eq!(cols, vec!["CUST_ID", "ORD_STATUS", "FINAL_PAY_AMT", "CUST_NM", "CUST_GRADE", "CITY"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["C001", "DELIVERED", "150000", "Kim", "VIP", "Seoul"]);
        assert_eq!(rows[1], vec!["C002", "PAY_COMPLETED", "80000", "Lee", "GOLD", "Busan"]);
        assert_eq!(sql_id, Some("6ykqdm2dgrdp7".to_string()));

        // Also test 0 rows selected case
        let sample_zero = "
Session altered.

no rows selected

SQL_ID: fu8vxya07qrfq
";
        let (cols_z, rows_z, sql_id_z) = parse_csv_output_with_sql_id(sample_zero);
        assert!(cols_z.is_empty());
        assert!(rows_z.is_empty());
        assert_eq!(sql_id_z, Some("fu8vxya07qrfq".to_string()));
    }
}

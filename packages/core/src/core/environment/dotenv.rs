//! `.env` parsing and formatting, and merging of env entries.
//!
//! Parser accepts: blank lines, `#` comments (attached to the following
//! variable), an optional `export ` prefix, unquoted values (inline ` #`
//! comments stripped, trimmed), single-quoted literal values, and
//! double-quoted values with escapes (`\n \r \t \" \\ \$ \``) that may span
//! multiple lines.

use crate::core::error::{Error, Result};
use crate::core::format::EnvDoc;
use crate::protocol::{EnvFormat, EnvPair, EnvVar};

/// Merge env documents in order; later documents override earlier keys
/// (the key keeps its first position).
pub fn merge<'a>(docs: impl IntoIterator<Item = &'a EnvDoc>) -> Vec<EnvPair> {
    let mut out: Vec<EnvPair> = Vec::new();
    for d in docs {
        for v in &d.vars {
            if let Some(p) = out.iter_mut().find(|p| p.key == v.key) {
                p.value = v.value.clone();
            } else {
                out.push(EnvPair {
                    key: v.key.clone(),
                    value: v.value.clone(),
                });
            }
        }
    }
    out
}

pub fn is_valid_key(k: &str) -> bool {
    let mut it = k.chars();
    matches!(it.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && it.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn bare_ok(v: &str) -> bool {
    !v.is_empty()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@%+,-=~^".contains(c))
}

fn dq(v: &str) -> String {
    let mut s = String::with_capacity(v.len() + 2);
    s.push('"');
    for c in v.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '$' => s.push_str("\\$"),
            '`' => s.push_str("\\`"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

fn sq_shell(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\\''"))
}

/// Format variables. Comments are emitted for dotenv/export only.
pub fn format(vars: &[EnvVar], fmt: EnvFormat) -> String {
    let mut out = String::new();
    match fmt {
        EnvFormat::Json => {
            out.push('{');
            for (i, v) in vars.iter().enumerate() {
                out.push_str(if i == 0 { "\n  " } else { ",\n  " });
                out.push_str(&serde_json::to_string(&v.key).expect("string"));
                out.push_str(": ");
                out.push_str(
                    &serde_json::to_string(v.value.as_deref().unwrap_or("")).expect("string"),
                );
            }
            out.push_str(if vars.is_empty() { "}\n" } else { "\n}\n" });
        }
        EnvFormat::Dotenv | EnvFormat::Export => {
            for v in vars {
                if let Some(c) = v.comment.as_deref().filter(|c| !c.is_empty()) {
                    for line in c.lines() {
                        out.push_str("# ");
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                let val = v.value.as_deref().unwrap_or("");
                if fmt == EnvFormat::Export {
                    out.push_str("export ");
                    out.push_str(&v.key);
                    out.push('=');
                    out.push_str(&sq_shell(val));
                } else {
                    out.push_str(&v.key);
                    out.push('=');
                    if val.is_empty() {
                    } else if bare_ok(val) {
                        out.push_str(val);
                    } else {
                        out.push_str(&dq(val));
                    }
                }
                out.push('\n');
            }
        }
    }
    out
}

pub fn format_pairs(pairs: &[EnvPair], fmt: EnvFormat) -> String {
    let vars: Vec<EnvVar> = pairs
        .iter()
        .map(|p| EnvVar {
            key: p.key.clone(),
            value: Some(p.value.clone()),
            hidden: true,
            comment: None,
        })
        .collect();
    format(&vars, fmt)
}

/// Parse dotenv text. Values are returned (the user just provided them);
/// every parsed variable is marked hidden.
pub fn parse(text: &str) -> Result<Vec<EnvVar>> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut line_no = 1usize;
    let mut out: Vec<EnvVar> = Vec::new();
    let mut pending_comment: Vec<String> = Vec::new();
    let n = chars.len();

    let skip_inline_ws = |i: &mut usize| {
        while *i < n && (chars[*i] == ' ' || chars[*i] == '\t') {
            *i += 1;
        }
    };

    while i < n {
        skip_inline_ws(&mut i);
        if i >= n {
            break;
        }
        match chars[i] {
            '\n' => {
                pending_comment.clear();
                line_no += 1;
                i += 1;
                continue;
            }
            '\r' => {
                i += 1;
                continue;
            }
            '#' => {
                let start = i + 1;
                while i < n && chars[i] != '\n' {
                    i += 1;
                }
                let c: String = chars[start..i].iter().collect();
                let c = c.trim_end_matches('\r');
                pending_comment.push(c.strip_prefix(' ').unwrap_or(c).to_owned());
                if i < n {
                    // Consume the newline so the comment sticks to the next var.
                    i += 1;
                    line_no += 1;
                }
                continue;
            }
            _ => {}
        }
        // key (with optional `export `)
        let ks = i;
        while i < n && chars[i] != '=' && chars[i] != '\n' {
            i += 1;
        }
        let mut key: String = chars[ks..i].iter().collect::<String>().trim().to_owned();
        if let Some(rest) = key.strip_prefix("export")
            && rest.starts_with([' ', '\t'])
        {
            key = rest.trim().to_owned();
        }
        if i >= n || chars[i] != '=' {
            return Err(Error::invalid(format!(
                "line {line_no}: expected KEY=VALUE"
            )));
        }
        if !is_valid_key(&key) {
            return Err(Error::invalid(format!(
                "line {line_no}: invalid variable name"
            )));
        }
        i += 1; // '='
        skip_inline_ws(&mut i);
        let start_line = line_no;
        let mut value = String::new();
        let mut inline_comment: Option<String> = None;
        if i < n && chars[i] == '"' {
            i += 1;
            let mut closed = false;
            while i < n {
                let c = chars[i];
                if c == '"' {
                    closed = true;
                    i += 1;
                    break;
                }
                if c == '\\' && i + 1 < n {
                    let e = chars[i + 1];
                    match e {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '"' | '\\' | '$' | '`' => value.push(e),
                        '\n' => {
                            line_no += 1;
                            value.push('\\');
                            value.push('\n');
                        }
                        other => {
                            value.push('\\');
                            value.push(other);
                        }
                    }
                    i += 2;
                    continue;
                }
                if c == '\n' {
                    line_no += 1;
                }
                value.push(c);
                i += 1;
            }
            if !closed {
                return Err(Error::invalid(format!(
                    "line {start_line}: unterminated double-quoted value"
                )));
            }
        } else if i < n && chars[i] == '\'' {
            i += 1;
            let mut closed = false;
            while i < n {
                if chars[i] == '\'' {
                    closed = true;
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    line_no += 1;
                }
                value.push(chars[i]);
                i += 1;
            }
            if !closed {
                return Err(Error::invalid(format!(
                    "line {start_line}: unterminated single-quoted value"
                )));
            }
        } else {
            let vs = i;
            while i < n && chars[i] != '\n' {
                if chars[i] == '#' && i > vs && (chars[i - 1] == ' ' || chars[i - 1] == '\t') {
                    break;
                }
                i += 1;
            }
            value = chars[vs..i].iter().collect::<String>().trim().to_owned();
        }
        // Rest of the line: whitespace and an optional comment.
        skip_inline_ws(&mut i);
        if i < n && chars[i] == '#' {
            let s = i + 1;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            let c: String = chars[s..i].iter().collect();
            let c = c.trim();
            if !c.is_empty() {
                inline_comment = Some(c.to_owned());
            }
        } else if i < n && chars[i] == '\r' {
            i += 1;
        }
        if i < n && chars[i] != '\n' {
            return Err(Error::invalid(format!(
                "line {line_no}: unexpected characters after value"
            )));
        }
        let comment = if pending_comment.is_empty() {
            inline_comment
        } else {
            Some(pending_comment.join("\n"))
        };
        pending_comment.clear();
        let var = EnvVar {
            key,
            value: Some(value),
            hidden: true,
            comment,
        };
        if let Some(existing) = out.iter_mut().find(|v| v.key == var.key) {
            *existing = var;
        } else {
            out.push(var);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::format::EnvVarDoc;

    fn kv(v: &[EnvVar]) -> Vec<(String, String)> {
        v.iter()
            .map(|v| (v.key.clone(), v.value.clone().unwrap()))
            .collect()
    }

    #[test]
    fn parse_basics() {
        let text = r#"
# database
DATABASE_URL=postgres://u:p@h/db?sslmode=require
export API_KEY = 'sk_test_ab#c'
EMPTY=
SPACED = hello world   # trailing comment
QUOTED="a \"b\"\n\$HOME"
MULTI="line1
line2"
WIN=crlf
"#;
        let v = parse(&text.replace("WIN=crlf\n", "WIN=crlf\r\n")).unwrap();
        assert_eq!(
            kv(&v),
            vec![
                (
                    "DATABASE_URL".into(),
                    "postgres://u:p@h/db?sslmode=require".into()
                ),
                ("API_KEY".into(), "sk_test_ab#c".into()),
                ("EMPTY".into(), "".into()),
                ("SPACED".into(), "hello world".into()),
                ("QUOTED".into(), "a \"b\"\n$HOME".into()),
                ("MULTI".into(), "line1\nline2".into()),
                ("WIN".into(), "crlf".into()),
            ]
        );
        assert_eq!(v[0].comment.as_deref(), Some("database"));
        assert_eq!(v[3].comment.as_deref(), Some("trailing comment"));
        assert!(v.iter().all(|v| v.hidden));
    }

    #[test]
    fn parse_errors_do_not_echo() {
        let e = parse("NOEQUALS_supersecret").unwrap_err();
        assert!(e.to_string().contains("line 1"));
        assert!(!e.to_string().contains("supersecret"));
        assert!(parse("K=\"unterminated supersecret").is_err());
        assert!(parse("K='x").is_err());
        assert!(parse("1BAD=x").is_err());
        assert!(parse("K=\"a\" junk").is_err());
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let v = parse("A=1\nB=2\nA=3\n").unwrap();
        assert_eq!(
            kv(&v),
            vec![("A".into(), "3".into()), ("B".into(), "2".into())]
        );
    }

    #[test]
    fn format_round_trip() {
        let vars = vec![
            EnvVar {
                key: "A".into(),
                value: Some("plain".into()),
                hidden: true,
                comment: Some("first\nsecond".into()),
            },
            EnvVar {
                key: "B".into(),
                value: Some("has space & 'quotes' \"dq\" $x `y` \\ #".into()),
                hidden: true,
                comment: None,
            },
            EnvVar {
                key: "C".into(),
                value: Some("multi\nline\r\n\ttab".into()),
                hidden: false,
                comment: None,
            },
            EnvVar {
                key: "D".into(),
                value: Some(String::new()),
                hidden: false,
                comment: None,
            },
        ];
        let text = format(&vars, EnvFormat::Dotenv);
        let back = parse(&text).unwrap();
        assert_eq!(kv(&back), kv(&vars));
        assert_eq!(back[0].comment.as_deref(), Some("first\nsecond"));
        assert!(text.contains("A=plain\n"));
    }

    #[test]
    fn export_format_is_shell_safe() {
        let vars = vec![EnvVar {
            key: "K".into(),
            value: Some("it's $HOME".into()),
            hidden: true,
            comment: None,
        }];
        let text = format(&vars, EnvFormat::Export);
        assert_eq!(text, "export K='it'\\''s $HOME'\n");
        if std::path::Path::new("/bin/sh").exists() {
            let out = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("{text}printf %s \"$K\""))
                .output()
                .unwrap();
            assert_eq!(String::from_utf8(out.stdout).unwrap(), "it's $HOME");
        }
    }

    #[test]
    fn json_format() {
        let vars = vec![
            EnvVar {
                key: "A".into(),
                value: Some("1\"".into()),
                hidden: true,
                comment: None,
            },
            EnvVar {
                key: "B".into(),
                value: Some("2".into()),
                hidden: true,
                comment: None,
            },
        ];
        let text = format(&vars, EnvFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["A"], "1\"");
        assert!(text.find("\"A\"").unwrap() < text.find("\"B\"").unwrap());
        assert_eq!(format(&[], EnvFormat::Json), "{}\n");
    }

    #[test]
    fn merging() {
        let d = |pairs: &[(&str, &str)]| EnvDoc {
            vars: pairs
                .iter()
                .map(|(k, v)| EnvVarDoc {
                    key: (*k).into(),
                    value: (*v).into(),
                    hidden: true,
                    comment: None,
                })
                .collect(),
        };
        let a = d(&[("A", "1"), ("B", "2")]);
        let b = d(&[("B", "3"), ("C", "4")]);
        let m = merge([&a, &b]);
        let got: Vec<_> = m
            .iter()
            .map(|p| (p.key.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(got, vec![("A", "1"), ("B", "3"), ("C", "4")]);
        let pairs_text = format_pairs(&m, EnvFormat::Dotenv);
        assert_eq!(pairs_text, "A=1\nB=3\nC=4\n");
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Extract plain string literals from Rust source text (skipping comments, attributes,
/// `extern "..."`, byte/c strings, and `include*!` / `env!` / `format!` templates).
pub fn extract_ui_strings_from_source(src: &str, out: &mut Vec<String>) {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut prev_ident = String::new();
    let mut prev_punct = '\0';
    let mut attr_bracket_depth = 0usize;

    while i < len {
        let b = bytes[i];

        // Line comment
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // Block comment (nested)
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1usize;
            while i < len && depth > 0 {
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Attribute detection: #[ ... ] or #![ ... ]
        if b == b'#' {
            let mut j = i + 1;
            while j < len && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < len && bytes[j] == b'!' {
                j += 1;
                while j < len && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
            }
            if j < len && bytes[j] == b'[' {
                attr_bracket_depth += 1;
                i = j + 1;
                prev_punct = '[';
                continue;
            }
        }

        if attr_bracket_depth > 0 {
            if b == b'[' {
                attr_bracket_depth += 1;
            } else if b == b']' {
                attr_bracket_depth -= 1;
            } else if b == b'"' {
                let (_, next_i) = parse_quoted_string(src, i);
                i = next_i;
                continue;
            }
            i += 1;
            continue;
        }

        // Byte or C strings: b"...", br"...", c"...", cr"..."
        if (b == b'b' || b == b'c') && i + 1 < len {
            if bytes[i + 1] == b'"' {
                let (_, next_i) = parse_quoted_string(src, i + 1);
                i = next_i;
                continue;
            }
            if bytes[i + 1] == b'r' {
                if let Some((_, next_i)) = try_parse_raw_string(src, i + 1) {
                    i = next_i;
                    continue;
                }
            }
        }

        // Raw string r"..." or r#"..."#
        if b == b'r' {
            if let Some((text, next_i)) = try_parse_raw_string(src, i) {
                if should_keep_string(&prev_ident, prev_punct, &text) {
                    out.push(text);
                }
                prev_ident.clear();
                prev_punct = '\0';
                i = next_i;
                continue;
            }
        }

        // Normal string literal "..."
        if b == b'"' {
            let (text, next_i) = parse_quoted_string(src, i);
            if should_keep_string(&prev_ident, prev_punct, &text) {
                out.push(text);
            }
            prev_ident.clear();
            prev_punct = '\0';
            i = next_i;
            continue;
        }

        // Char literal 'x'
        if b == b'\'' {
            // Distinguish char literal from lifetime `'a`
            if i + 2 < len && bytes[i + 1] == b'\\' {
                i += 2;
                while i < len && bytes[i] != b'\'' {
                    i += 1;
                }
                i += 1;
                continue;
            } else if i + 2 < len && bytes[i + 2] == b'\'' {
                i += 3;
                continue;
            }
        }

        // Identifier
        if (b as char).is_ascii_alphabetic() || b == b'_' {
            let start = i;
            i += 1;
            while i < len && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            prev_ident = src[start..i].to_string();
            prev_punct = '\0';
            continue;
        }

        if !b.is_ascii_whitespace() {
            prev_punct = b as char;
        }
        i += 1;
    }
}

fn should_keep_string(prev_ident: &str, prev_punct: char, text: &str) -> bool {
    if prev_ident == "extern" {
        return false;
    }
    if prev_punct == '!' {
        // e.g. `include_str!("...")` — skip single literal right after `!`? Not tracked deeply here,
        // but is_translatable_ui_text filters file paths and format-only strings.
    }
    lang_translator::is_translatable_ui_text(text)
}

pub fn parse_quoted_string(src: &str, quote_pos: usize) -> (String, usize) {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = quote_pos + 1;
    let mut out = String::new();

    while i < len {
        let b = bytes[i];
        if b == b'"' {
            return (out, i + 1);
        }
        if b == b'\\' && i + 1 < len {
            let esc = bytes[i + 1];
            match esc {
                b'n' => {
                    out.push('\n');
                    i += 2;
                }
                b'r' => {
                    out.push('\r');
                    i += 2;
                }
                b't' => {
                    out.push('\t');
                    i += 2;
                }
                b'\\' => {
                    out.push('\\');
                    i += 2;
                }
                b'"' => {
                    out.push('"');
                    i += 2;
                }
                b'\'' => {
                    out.push('\'');
                    i += 2;
                }
                b'0' => {
                    out.push('\0');
                    i += 2;
                }
                b'x' if i + 3 < len => {
                    if let Ok(v) = u8::from_str_radix(&src[i + 2..i + 4], 16) {
                        out.push(v as char);
                    }
                    i += 4;
                }
                b'u' if i + 2 < len && bytes[i + 2] == b'{' => {
                    if let Some(rel_end) = src[i + 3..].find('}') {
                        let hex = &src[i + 3..i + 3 + rel_end];
                        if let Ok(code) = u32::from_str_radix(hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                out.push(ch);
                            }
                        }
                        i = i + 3 + rel_end + 1;
                    } else {
                        i += 2;
                    }
                }
                b'\n' | b'\r' => {
                    i += 2;
                    while i < len && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                }
                _ => {
                    out.push(esc as char);
                    i += 2;
                }
            }
            continue;
        }

        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, len)
}

pub fn try_parse_raw_string(src: &str, r_pos: usize) -> Option<(String, usize)> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    if bytes.get(r_pos).copied() != Some(b'r') {
        return None;
    }
    let mut hashes = 0usize;
    let mut i = r_pos + 1;
    while i < len && bytes[i] == b'#' {
        hashes += 1;
        i += 1;
    }
    if i >= len || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let content_start = i;
    while i < len {
        if bytes[i] == b'"' {
            let mut matched = 0usize;
            while matched < hashes && i + 1 + matched < len && bytes[i + 1 + matched] == b'#' {
                matched += 1;
            }
            if matched == hashes {
                let text = src[content_start..i].to_string();
                return Some((text, i + 1 + hashes));
            }
        }
        i += 1;
    }
    None
}

/// Recursively scan a directory of `.rs` files and produce deduplicated, hash-sorted catalog entries.
pub fn scan_directory_entries(dir: &Path) -> Vec<(u64, String, [String; lang_translator::Language::COUNT])> {
    let mut strings = Vec::new();
    collect_rs_files(dir, &mut strings);

    let mut by_hash: BTreeMap<u64, (String, [String; lang_translator::Language::COUNT])> = BTreeMap::new();
    for s in strings {
        let h = lang_translator::fnv1a_64(s.as_bytes());
        by_hash.entry(h).or_insert_with(|| {
            let translations = lang_translator::translate_all(&s);
            (s, translations)
        });
    }

    by_hash
        .into_iter()
        .map(|(h, (orig, trans))| (h, orig, trans))
        .collect()
}

fn collect_rs_files(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
            }
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(&path) {
                extract_ui_strings_from_source(&content, out);
            }
        }
    }
}

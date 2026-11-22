//! Build-script helper for `lang`.
//!
//! Add `lang-build` under `[build-dependencies]` and call `lang_build::generate_catalog()`
//! inside `build.rs` to automatically scan `src/**/*.rs`, pre-translate all discovered
//! UI texts offline, and emit `OUT_DIR/lang_catalog.rs` for `lang::include_catalog!()`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Scans `CARGO_MANIFEST_DIR/src` and writes `OUT_DIR/lang_catalog.rs`.
pub fn generate_catalog() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    let src_dir = PathBuf::from(&manifest_dir).join("src");
    generate_catalog_from_dir(&src_dir);
}

/// Scans a custom directory and writes `OUT_DIR/lang_catalog.rs`.
pub fn generate_catalog_from_dir(dir: &Path) {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let dest = PathBuf::from(out_dir).join("lang_catalog.rs");

    println!("cargo:rerun-if-changed={}", dir.display());

    let mut texts = Vec::new();
    collect_rs_files(dir, &mut texts);

    let mut by_hash: BTreeMap<u64, (String, [String; lang_translator::Language::COUNT])> = BTreeMap::new();
    for s in texts {
        let h = lang_translator::fnv1a_64(s.as_bytes());
        by_hash.entry(h).or_insert_with(|| {
            let translations = lang_translator::translate_all(&s);
            (s, translations)
        });
    }

    let mut code = String::from("pub static LANG_CATALOG: ::lang::LangCatalog = ::lang::LangCatalog::new(&[\n");
    for (hash, (orig, trans)) in by_hash {
        code.push_str("    ::lang::CatalogEntry::new(");
        code.push_str(&format!("{hash}u64, "));
        code.push_str(&escape_rust_str(&orig));
        code.push_str(", [");
        for (idx, tr) in trans.iter().enumerate() {
            if idx > 0 {
                code.push_str(", ");
            }
            code.push_str(&escape_rust_str(tr));
        }
        code.push_str("]),\n");
    }
    code.push_str("]);\n");

    fs::write(&dest, code).expect("failed to write generated lang_catalog.rs");
}

fn escape_rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
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
            println!("cargo:rerun-if-changed={}", path.display());
            if let Ok(content) = fs::read_to_string(&path) {
                extract_strings(&content, out);
            }
        }
    }
}

fn extract_strings(src: &str, out: &mut Vec<String>) {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut attr_depth = 0usize;

    while i < len {
        let b = bytes[i];
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            let mut d = 1usize;
            while i < len && d > 0 {
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    d += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    d -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
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
                attr_depth += 1;
                i = j + 1;
                continue;
            }
        }
        if attr_depth > 0 {
            if b == b'[' {
                attr_depth += 1;
            } else if b == b']' {
                attr_depth -= 1;
            } else if b == b'"' {
                let (_, next_i) = parse_quoted(src, i);
                i = next_i;
                continue;
            }
            i += 1;
            continue;
        }
        if (b == b'b' || b == b'c') && i + 1 < len && bytes[i + 1] == b'"' {
            let (_, next_i) = parse_quoted(src, i + 1);
            i = next_i;
            continue;
        }
        if b == b'"' {
            let (s, next_i) = parse_quoted(src, i);
            if lang_translator::is_translatable_ui_text(&s) {
                out.push(s);
            }
            i = next_i;
            continue;
        }
        i += 1;
    }
}

fn parse_quoted(src: &str, quote_pos: usize) -> (String, usize) {
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
            match bytes[i + 1] {
                b'n' => { out.push('\n'); i += 2; }
                b'r' => { out.push('\r'); i += 2; }
                b't' => { out.push('\t'); i += 2; }
                b'\\' => { out.push('\\'); i += 2; }
                b'"' => { out.push('"'); i += 2; }
                b'\'' => { out.push('\''); i += 2; }
                b'0' => { out.push('\0'); i += 2; }
                other => { out.push(other as char); i += 2; }
            }
            continue;
        }
        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, len)
}

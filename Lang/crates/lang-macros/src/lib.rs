use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use std::path::PathBuf;

mod scanner;

/// Attribute macro `#[lang::auto]` (also aliased as `#[lang::app]` and `#[lang::translate]`).
///
/// Place it on a module, `impl` block, or `fn`. It scans all UI text literals inside function
/// bodies at compile time, translates them offline via `lang-translator`, and rewrites each
/// literal into `::lang::select(&[es, en, pt, fr, de, it])`.
#[proc_macro_attribute]
pub fn auto(_attr: TokenStream, item: TokenStream) -> TokenStream {
    rewrite_item_stream(item)
}

#[proc_macro_attribute]
pub fn app(_attr: TokenStream, item: TokenStream) -> TokenStream {
    rewrite_item_stream(item)
}

#[proc_macro_attribute]
pub fn translate(_attr: TokenStream, item: TokenStream) -> TokenStream {
    rewrite_item_stream(item)
}

/// Function-like macro `lang::t!("Texto")` that pre-translates a single literal at compile time
/// into a zero-alloc `::lang::select(&[...])` call.
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    if tokens.len() == 1 {
        if let TokenTree::Literal(lit) = &tokens[0] {
            if let Some(s) = parse_str_literal(&lit.to_string()) {
                return emit_select_call(&s, lit.span());
            }
        }
    }
    TokenStream::from_iter(tokens)
}

/// Scans `CARGO_MANIFEST_DIR/src` at compile time and emits a static `::lang::LangCatalog`.
///
/// Usage:
/// ```ignore
/// static CATALOG: lang::LangCatalog = lang::scan_project!();
/// ```
#[proc_macro]
pub fn scan_project(input: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let custom_subdir = if tokens.len() == 1 {
        if let TokenTree::Literal(lit) = &tokens[0] {
            parse_str_literal(&lit.to_string())
        } else {
            None
        }
    } else {
        None
    };

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let target_dir = if let Some(sub) = custom_subdir {
        PathBuf::from(manifest_dir).join(sub)
    } else {
        PathBuf::from(manifest_dir).join("src")
    };

    let entries = scanner::scan_directory_entries(&target_dir);
    let mut code = String::from("::lang::LangCatalog::new(&[\n");
    for (hash, orig, trans) in entries {
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
    code.push_str("])");

    code.parse().expect("valid generated LangCatalog tokens")
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

fn parse_str_literal(raw: &str) -> Option<String> {
    if raw.starts_with('"') {
        let (s, _) = scanner::parse_quoted_string(raw, 0);
        return Some(s);
    }
    if raw.starts_with('r') {
        if let Some((s, _)) = scanner::try_parse_raw_string(raw, 0) {
            return Some(s);
        }
    }
    None
}

fn emit_select_call(text: &str, span: Span) -> TokenStream {
    let translations = lang_translator::translate_all(text);
    let mut inner = Vec::new();
    for (idx, tr) in translations.iter().enumerate() {
        if idx > 0 {
            let mut comma = Punct::new(',', Spacing::Alone);
            comma.set_span(span);
            inner.push(TokenTree::Punct(comma));
        }
        let mut lit = Literal::string(tr);
        lit.set_span(span);
        inner.push(TokenTree::Literal(lit));
    }

    let mut bracket = Group::new(Delimiter::Bracket, TokenStream::from_iter(inner));
    bracket.set_span(span);

    let mut amp = Punct::new('&', Spacing::Alone);
    amp.set_span(span);

    let mut args = Group::new(
        Delimiter::Parenthesis,
        TokenStream::from_iter([TokenTree::Punct(amp), TokenTree::Group(bracket)]),
    );
    args.set_span(span);

    let mut c1 = Punct::new(':', Spacing::Joint);
    c1.set_span(span);
    let mut c2 = Punct::new(':', Spacing::Alone);
    c2.set_span(span);
    let mut c3 = Punct::new(':', Spacing::Joint);
    c3.set_span(span);
    let mut c4 = Punct::new(':', Spacing::Alone);
    c4.set_span(span);

    let lang_ident = Ident::new("lang", span);
    let select_ident = Ident::new("select", span);

    TokenStream::from_iter([
        TokenTree::Punct(c1),
        TokenTree::Punct(c2),
        TokenTree::Ident(lang_ident),
        TokenTree::Punct(c3),
        TokenTree::Punct(c4),
        TokenTree::Ident(select_ident),
        TokenTree::Group(args),
    ])
}

/// Rewrites items (`mod`, `impl`, `fn`) preserving attributes, signatures, `const`/`static`,
/// and only rewriting translatable UI string literals inside function bodies.
fn rewrite_item_stream(stream: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;

    while i < tokens.len() {
        // Preserve attributes `#[...]` and `#![...]`
        if is_punct(&tokens[i], '#') {
            out.push(tokens[i].clone());
            i += 1;
            if i < tokens.len() && is_punct(&tokens[i], '!') {
                out.push(tokens[i].clone());
                i += 1;
            }
            if i < tokens.len() {
                out.push(tokens[i].clone());
                i += 1;
            }
            continue;
        }

        // Preserve `const` / `static` item declarations up to `;`
        if is_ident(&tokens[i], "const") || is_ident(&tokens[i], "static") {
            // Careful: `const fn` should still be handled as `fn`!
            let is_const_fn = is_ident(&tokens[i], "const")
                && i + 1 < tokens.len()
                && (is_ident(&tokens[i + 1], "fn")
                    || (is_ident(&tokens[i + 1], "unsafe")
                        && i + 2 < tokens.len()
                        && is_ident(&tokens[i + 2], "fn")));
            if !is_const_fn {
                while i < tokens.len() {
                    let is_semi = is_punct(&tokens[i], ';');
                    out.push(tokens[i].clone());
                    i += 1;
                    if is_semi {
                        break;
                    }
                }
                continue;
            }
        }

        // Function definition: `fn name(...) ... { body }`
        if is_ident(&tokens[i], "fn") {
            out.push(tokens[i].clone());
            i += 1;
            let mut angle_depth = 0usize;
            let mut prev_hyphen = false;
            while i < tokens.len() {
                let is_arrow = prev_hyphen && is_punct(&tokens[i], '>');
                prev_hyphen = matches!(&tokens[i], TokenTree::Punct(p) if p.as_char() == '-');
                match &tokens[i] {
                    TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => {
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                    TokenTree::Group(g) if g.delimiter() == Delimiter::Brace && angle_depth == 0 => {
                        let rewritten_body = rewrite_expr_stream(g.stream(), false);
                        let mut new_g = Group::new(Delimiter::Brace, rewritten_body);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                        i += 1;
                        break;
                    }
                    TokenTree::Punct(p) if p.as_char() == ';' && angle_depth == 0 => {
                        out.push(tokens[i].clone());
                        i += 1;
                        break;
                    }
                    TokenTree::Punct(p) if p.as_char() == '<' => {
                        angle_depth += 1;
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                    TokenTree::Punct(p) if p.as_char() == '>' && !is_arrow && angle_depth > 0 => {
                        angle_depth -= 1;
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                    _ => {
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            continue;
        }

        // Module or impl block: recurse into `{ ... }` at item level
        if is_ident(&tokens[i], "mod") || is_ident(&tokens[i], "impl") {
            out.push(tokens[i].clone());
            i += 1;
            while i < tokens.len() {
                match &tokens[i] {
                    TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                        let inner = rewrite_item_stream(g.stream());
                        let mut new_g = Group::new(Delimiter::Brace, inner);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                        i += 1;
                        break;
                    }
                    TokenTree::Punct(p) if p.as_char() == ';' => {
                        out.push(tokens[i].clone());
                        i += 1;
                        break;
                    }
                    _ => {
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            continue;
        }

        // Struct / enum / trait / type: leave signatures untouched, except trait default fn bodies
        if is_ident(&tokens[i], "trait") {
            out.push(tokens[i].clone());
            i += 1;
            while i < tokens.len() {
                if let TokenTree::Group(g) = &tokens[i] {
                    if g.delimiter() == Delimiter::Brace {
                        let inner = rewrite_item_stream(g.stream());
                        let mut new_g = Group::new(Delimiter::Brace, inner);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                        i += 1;
                        break;
                    }
                }
                out.push(tokens[i].clone());
                i += 1;
            }
            continue;
        }

        out.push(tokens[i].clone());
        i += 1;
    }

    TokenStream::from_iter(out)
}

fn is_non_rewritable_macro(name: &str) -> bool {
    matches!(
        name,
        "include_str"
            | "include_bytes"
            | "include"
            | "env"
            | "option_env"
            | "concat"
            | "stringify"
            | "asm"
            | "global_asm"
            | "naked_asm"
            | "compile_error"
            | "cfg"
            | "file"
            | "line"
            | "column"
            | "module_path"
    )
}

fn is_format_macro(name: &str) -> bool {
    matches!(
        name,
        "format"
            | "format_args"
            | "write"
            | "writeln"
            | "print"
            | "println"
            | "eprint"
            | "eprintln"
            | "panic"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "assert"
            | "debug_assert"
    )
}

fn rewrite_expr_stream(stream: TokenStream, in_match_body: bool) -> TokenStream {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;

    // Inside a `match` body, tokens from arm start up to `=>` are patterns and must NOT be rewritten.
    let mut expecting_pattern = in_match_body;

    while i < tokens.len() {
        // Attributes inside fn body
        if is_punct(&tokens[i], '#') {
            out.push(tokens[i].clone());
            i += 1;
            if i < tokens.len() && is_punct(&tokens[i], '!') {
                out.push(tokens[i].clone());
                i += 1;
            }
            if i < tokens.len() {
                out.push(tokens[i].clone());
                i += 1;
            }
            continue;
        }

        // Detect `=>` ending a match arm pattern
        if in_match_body && is_punct(&tokens[i], '=') && i + 1 < tokens.len() && is_punct(&tokens[i + 1], '>') {
            expecting_pattern = false;
            out.push(tokens[i].clone());
            out.push(tokens[i + 1].clone());
            i += 2;
            continue;
        }

        // Comma at top level of match arm restarts pattern for next arm
        if in_match_body && !expecting_pattern && is_punct(&tokens[i], ',') {
            expecting_pattern = true;
            out.push(tokens[i].clone());
            i += 1;
            continue;
        }

        // Nested `const` / `static` inside fn
        if is_ident(&tokens[i], "const") || is_ident(&tokens[i], "static") {
            let is_const_fn = is_ident(&tokens[i], "const")
                && i + 1 < tokens.len()
                && is_ident(&tokens[i + 1], "fn");
            if !is_const_fn {
                while i < tokens.len() {
                    let is_semi = is_punct(&tokens[i], ';');
                    out.push(tokens[i].clone());
                    i += 1;
                    if is_semi {
                        break;
                    }
                }
                continue;
            }
        }

        // Nested `fn` inside fn
        if is_ident(&tokens[i], "fn") {
            let mut rest = Vec::new();
            while i < tokens.len() {
                rest.push(tokens[i].clone());
                if let TokenTree::Group(g) = &tokens[i] {
                    if g.delimiter() == Delimiter::Brace {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            out.extend(rewrite_item_stream(TokenStream::from_iter(rest)));
            continue;
        }

        // `extern "C"` ABI literal
        if is_ident(&tokens[i], "extern") {
            out.push(tokens[i].clone());
            i += 1;
            if i < tokens.len() {
                if let TokenTree::Literal(_) = &tokens[i] {
                    out.push(tokens[i].clone());
                    i += 1;
                }
            }
            continue;
        }

        // `match <expr> { <arms> }`
        if is_ident(&tokens[i], "match") {
            out.push(tokens[i].clone());
            i += 1;
            while i < tokens.len() {
                if let TokenTree::Group(g) = &tokens[i] {
                    if g.delimiter() == Delimiter::Brace {
                        let rewritten_arms = rewrite_expr_stream(g.stream(), true);
                        let mut new_g = Group::new(Delimiter::Brace, rewritten_arms);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                        i += 1;
                        break;
                    }
                }
                // Rewrite literals inside match scrutinee expression
                if let TokenTree::Literal(lit) = &tokens[i] {
                    if let Some(s) = parse_str_literal(&lit.to_string()) {
                        if lang_translator::is_translatable_ui_text(&s) {
                            out.extend(emit_select_call(&s, lit.span()));
                            i += 1;
                            continue;
                        }
                    }
                } else if let TokenTree::Group(g) = &tokens[i] {
                    let inner = rewrite_expr_stream(g.stream(), false);
                    let mut new_g = Group::new(g.delimiter(), inner);
                    new_g.set_span(g.span());
                    out.push(TokenTree::Group(new_g));
                    i += 1;
                    continue;
                }
                out.push(tokens[i].clone());
                i += 1;
            }
            continue;
        }

        // `let <pattern> = <expr>` -> skip pattern until `=` or `;`
        if is_ident(&tokens[i], "let") {
            out.push(tokens[i].clone());
            i += 1;
            while i < tokens.len() {
                let eq = is_punct(&tokens[i], '=');
                let semi = is_punct(&tokens[i], ';');
                out.push(tokens[i].clone());
                i += 1;
                if eq || semi {
                    break;
                }
            }
            continue;
        }

        // Macro invocation: `ident ! (...)`
        if let TokenTree::Ident(ident) = &tokens[i] {
            if i + 2 < tokens.len() && is_punct(&tokens[i + 1], '!') {
                if let TokenTree::Group(g) = &tokens[i + 2] {
                    let name = ident.to_string();
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    if is_non_rewritable_macro(&name) {
                        out.push(tokens[i + 2].clone());
                    } else if is_format_macro(&name) {
                        // Keep the format template literal intact; rewrite subsequent args after first `,`
                        let inner = rewrite_format_macro_args(g.stream(), name == "write" || name == "writeln");
                        let mut new_g = Group::new(g.delimiter(), inner);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                    } else {
                        let inner = rewrite_expr_stream(g.stream(), false);
                        let mut new_g = Group::new(g.delimiter(), inner);
                        new_g.set_span(g.span());
                        out.push(TokenTree::Group(new_g));
                    }
                    i += 3;
                    continue;
                }
            }
        }

        // Groups (`(...)`, `{...}`, `[...]`)
        if let TokenTree::Group(g) = &tokens[i] {
            let is_brace_arm_body = in_match_body && !expecting_pattern && g.delimiter() == Delimiter::Brace;
            if expecting_pattern {
                out.push(tokens[i].clone());
            } else {
                let inner = rewrite_expr_stream(g.stream(), false);
                let mut new_g = Group::new(g.delimiter(), inner);
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
            }
            if is_brace_arm_body {
                // In match arms, `{ ... }` ends the arm without needing a comma
                expecting_pattern = true;
            }
            i += 1;
            continue;
        }

        // Plain string literal inside an expression!
        if !expecting_pattern {
            if let TokenTree::Literal(lit) = &tokens[i] {
                if let Some(s) = parse_str_literal(&lit.to_string()) {
                    if lang_translator::is_translatable_ui_text(&s) {
                        out.extend(emit_select_call(&s, lit.span()));
                        i += 1;
                        continue;
                    }
                }
            }
        }

        out.push(tokens[i].clone());
        i += 1;
    }

    TokenStream::from_iter(out)
}

fn rewrite_format_macro_args(stream: TokenStream, has_writer_first_arg: bool) -> TokenStream {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut comma_count = 0usize;
    let template_arg_index = if has_writer_first_arg { 1 } else { 0 };

    let mut i = 0usize;
    while i < tokens.len() {
        if is_punct(&tokens[i], ',') {
            comma_count += 1;
            out.push(tokens[i].clone());
            i += 1;
            continue;
        }

        if comma_count == template_arg_index {
            // Keep the format template literal intact so rustc's format_args! compiles cleanly
            out.push(tokens[i].clone());
        } else if let TokenTree::Literal(lit) = &tokens[i] {
            if let Some(s) = parse_str_literal(&lit.to_string()) {
                if lang_translator::is_translatable_ui_text(&s) {
                    out.extend(emit_select_call(&s, lit.span()));
                    i += 1;
                    continue;
                }
            }
            out.push(tokens[i].clone());
        } else if let TokenTree::Group(g) = &tokens[i] {
            let inner = rewrite_expr_stream(g.stream(), false);
            let mut new_g = Group::new(g.delimiter(), inner);
            new_g.set_span(g.span());
            out.push(TokenTree::Group(new_g));
        } else {
            out.push(tokens[i].clone());
        }
        i += 1;
    }
    TokenStream::from_iter(out)
}

#[inline]
fn is_punct(tt: &TokenTree, ch: char) -> bool {
    matches!(tt, TokenTree::Punct(p) if p.as_char() == ch)
}

#[inline]
fn is_ident(tt: &TokenTree, name: &str) -> bool {
    matches!(tt, TokenTree::Ident(id) if id.to_string() == name)
}

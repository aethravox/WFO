#![no_std]
//! Offline, zero-dependency, `#![no_std]` multi-language translator engine used by `lang`.

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;

mod lexicon;
mod phrases;

pub use lexicon::WORDS;
pub use phrases::PHRASES;

/// Supported built-in languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Language {
    Es = 0,
    En = 1,
    Pt = 2,
    Fr = 3,
    De = 4,
    It = 5,
}

impl Language {
    pub const COUNT: usize = 6;

    pub const ALL: [Language; Self::COUNT] = [
        Language::Es,
        Language::En,
        Language::Pt,
        Language::Fr,
        Language::De,
        Language::It,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Es),
            1 => Some(Self::En),
            2 => Some(Self::Pt),
            3 => Some(Self::Fr),
            4 => Some(Self::De),
            5 => Some(Self::It),
            _ => None,
        }
    }

    /// Short uppercase badge for UI buttons (`"ES"`, `"EN"`, ...).
    #[inline]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Es => "ES",
            Self::En => "EN",
            Self::Pt => "PT",
            Self::Fr => "FR",
            Self::De => "DE",
            Self::It => "IT",
        }
    }

    /// ISO 639-1 lowercase code (`"es"`, `"en"`, ...).
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Pt => "pt",
            Self::Fr => "fr",
            Self::De => "de",
            Self::It => "it",
        }
    }

    /// Native display name (`"Español"`, `"English"`, ...).
    #[inline]
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::Es => "Español",
            Self::En => "English",
            Self::Pt => "Português",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::It => "Italiano",
        }
    }

    /// Parse ISO code (case-insensitive).
    pub fn from_code(code: &str) -> Option<Self> {
        let b = code.as_bytes();
        if b.len() < 2 {
            return None;
        }
        let c0 = b[0].to_ascii_lowercase();
        let c1 = b[1].to_ascii_lowercase();
        match (c0, c1) {
            (b'e', b's') => Some(Self::Es),
            (b'e', b'n') => Some(Self::En),
            (b'p', b't') => Some(Self::Pt),
            (b'f', b'r') => Some(Self::Fr),
            (b'd', b'e') => Some(Self::De),
            (b'i', b't') => Some(Self::It),
            _ => None,
        }
    }

    /// Next language in the built-in cycle.
    #[inline]
    pub const fn next(self) -> Self {
        match self {
            Self::Es => Self::En,
            Self::En => Self::Pt,
            Self::Pt => Self::Fr,
            Self::Fr => Self::De,
            Self::De => Self::It,
            Self::It => Self::Es,
        }
    }
}

/// Deterministic 64-bit FNV-1a hash usable in `const` contexts.
#[inline]
pub const fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Casing {
    Lower,
    Upper,
    Title,
    Sentence,
}

fn detect_casing(s: &str) -> Casing {
    let mut has_alpha = false;
    let mut all_upper = true;
    let mut all_lower = true;
    let mut first_upper = false;
    let mut word_count = 0usize;
    let mut title_matches = 0usize;
    let mut in_word = false;

    for (idx, ch) in s.chars().enumerate() {
        if ch.is_alphabetic() {
            has_alpha = true;
            if ch.is_uppercase() {
                all_lower = false;
                if idx == 0 {
                    first_upper = true;
                }
                if !in_word {
                    title_matches += 1;
                }
            } else {
                all_upper = false;
            }
            if !in_word {
                in_word = true;
                word_count += 1;
            }
        } else {
            in_word = false;
        }
    }

    if !has_alpha || all_lower {
        Casing::Lower
    } else if all_upper && s.chars().filter(|c| c.is_alphabetic()).count() > 1 {
        Casing::Upper
    } else if word_count > 1 && title_matches == word_count {
        Casing::Title
    } else if first_upper {
        Casing::Sentence
    } else {
        Casing::Lower
    }
}

/// Case-insensitive & accent-insensitive byte/char match for offline lookup.
fn eq_folded(a: &str, b: &str, strip_accents: bool) -> bool {
    let mut ia = a.chars();
    let mut ib = b.chars();
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return true,
            (Some(ca), Some(cb)) => {
                if !char_eq_folded(ca, cb, strip_accents) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

#[inline]
fn fold_char(c: char, strip_accents: bool) -> char {
    let lc = match c {
        'A'..='Z' => (c as u8 + 32) as char,
        'Á' => 'á',
        'É' => 'é',
        'Í' => 'í',
        'Ó' => 'ó',
        'Ú' => 'ú',
        'Ü' => 'ü',
        'Ñ' => 'ñ',
        'À' => 'à',
        'Â' => 'â',
        'Ã' => 'ã',
        'Ê' => 'ê',
        'Ô' => 'ô',
        'Õ' => 'õ',
        'Ç' => 'ç',
        'Ä' => 'ä',
        'Ö' => 'ö',
        _ => c,
    };
    if !strip_accents {
        return lc;
    }
    match lc {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        _ => lc,
    }
}

#[inline]
fn char_eq_folded(a: char, b: char, strip_accents: bool) -> bool {
    fold_char(a, strip_accents) == fold_char(b, strip_accents)
}

fn to_upper_char(c: char) -> char {
    match c {
        'a'..='z' => (c as u8 - 32) as char,
        'á' => 'Á',
        'é' => 'É',
        'í' => 'Í',
        'ó' => 'Ó',
        'ú' => 'Ú',
        'ü' => 'Ü',
        'ñ' => 'Ñ',
        'à' => 'À',
        'â' => 'Â',
        'ã' => 'Ã',
        'ê' => 'Ê',
        'ô' => 'Ô',
        'õ' => 'Õ',
        'ç' => 'Ç',
        'ä' => 'Ä',
        'ö' => 'Ö',
        _ => c,
    }
}

/// Fixed-capacity UTF-8 writer over `&mut [u8]` for zero-alloc `#![no_std]` usage.
pub struct BufWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BufWriter<'a> {
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }

    pub fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let avail = self.buf.len().saturating_sub(self.pos);
        let n = if bytes.len() <= avail {
            bytes.len()
        } else {
            // Back off to a UTF-8 char boundary
            let mut end = avail;
            while end > 0 && (bytes[end] & 0b1100_0000) == 0b1000_0000 {
                end -= 1;
            }
            end
        };
        self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
        self.pos += n;
    }

    pub fn push_char(&mut self, c: char) {
        let mut tmp = [0u8; 4];
        let s = c.encode_utf8(&mut tmp);
        self.push_str(s);
    }

    fn push_with_casing(&mut self, s: &str, casing: Casing) {
        let mut first_alpha = true;
        let mut new_word = true;
        for c in s.chars() {
            let out = if c.is_alphabetic() {
                let res = match casing {
                    Casing::Lower => c,
                    Casing::Upper => to_upper_char(c),
                    Casing::Sentence => {
                        if first_alpha {
                            to_upper_char(c)
                        } else {
                            c
                        }
                    }
                    Casing::Title => {
                        if new_word {
                            to_upper_char(c)
                        } else {
                            c
                        }
                    }
                };
                first_alpha = false;
                new_word = false;
                res
            } else {
                new_word = !c.is_alphanumeric();
                c
            };
            self.push_char(out);
        }
    }
}

fn lookup_phrase(text: &str, target: Language) -> Option<&'static str> {
    // Pass 1: exact accent match
    for row in PHRASES {
        for cell in row.iter() {
            if eq_folded(text, cell, false) {
                return Some(row[target.index()]);
            }
        }
    }
    // Pass 2: accent-stripped match
    for row in PHRASES {
        for cell in row.iter() {
            if eq_folded(text, cell, true) {
                return Some(row[target.index()]);
            }
        }
    }
    None
}

fn lookup_word(word: &str, target: Language) -> Option<&'static str> {
    for row in WORDS {
        for cell in row.iter() {
            if eq_folded(word, cell, false) {
                return Some(row[target.index()]);
            }
        }
    }
    if let Some(p) = lookup_phrase(word, target) {
        return Some(p);
    }
    for row in WORDS {
        for cell in row.iter() {
            if eq_folded(word, cell, true) {
                return Some(row[target.index()]);
            }
        }
    }
    None
}

fn is_punct_or_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '.' | ',' | ':' | ';' | '!' | '?' | '¡' | '¿' | '(' | ')' | '[' | ']' | '-' | '/' | '…')
}

/// Detect the most likely source language of a UI string.
pub fn detect_language(text: &str) -> Language {
    let mut scores = [0i32; Language::COUNT];

    for c in text.chars() {
        match c {
            'ñ' | 'Ñ' | '¡' | '¿' => scores[Language::Es.index()] += 6,
            'ã' | 'Ã' | 'õ' | 'Õ' => scores[Language::Pt.index()] += 6,
            'ç' | 'Ç' => {
                scores[Language::Pt.index()] += 3;
                scores[Language::Fr.index()] += 3;
            }
            'ß' | 'ä' | 'Ä' | 'ö' | 'Ö' | 'ü' | 'Ü' => scores[Language::De.index()] += 4,
            'à' | 'è' | 'ù' | 'ê' | 'î' | 'ô' => scores[Language::Fr.index()] += 2,
            _ => {}
        }
    }

    // Score matching phrases
    let trimmed = text.trim();
    for row in PHRASES {
        for (lang_idx, cell) in row.iter().enumerate() {
            if eq_folded(trimmed, cell, false) {
                scores[lang_idx] += 10;
            }
        }
    }

    // Score matching words
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            score_word(&text[s..i], &mut scores);
        }
    }
    if let Some(s) = start {
        score_word(&text[s..], &mut scores);
    }

    let mut best_idx = 0usize;
    let mut best_score = scores[0];
    let mut i = 1;
    while i < Language::COUNT {
        if scores[i] > best_score {
            best_score = scores[i];
            best_idx = i;
        }
        i += 1;
    }
    Language::from_u8(best_idx as u8).unwrap_or(Language::Es)
}

fn score_word(word: &str, scores: &mut [i32; Language::COUNT]) {
    for row in WORDS {
        for (idx, cell) in row.iter().enumerate() {
            if eq_folded(word, cell, false) {
                scores[idx] += 2;
            }
        }
    }
}

/// Heuristic filter for automatic project source scanning:
/// skips technical strings (paths, URLs, hex codes, format placeholders, snake_case identifiers).
pub fn is_translatable_ui_text(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.len() > 240 {
        return false;
    }
    // Must contain at least one letter
    if !trimmed.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    // Skip URLs, paths, MIME types, hex colors
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ws://")
        || trimmed.starts_with("wss://")
        || trimmed.starts_with('/')
        || trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.starts_with('#')
    {
        return false;
    }
    // Skip pure format placeholders like "{}" or "{:?}"
    if trimmed.starts_with('{') && trimmed.ends_with('}') && !trimmed.contains(' ') {
        return false;
    }
    // Skip single-word snake_case or ALL_CAPS_CONST technical identifiers
    if !trimmed.contains(' ') {
        if trimmed.contains('_') || trimmed.contains("::") || trimmed.contains('/') {
            return false;
        }
        if trimmed.ends_with(".rs")
            || trimmed.ends_with(".toml")
            || trimmed.ends_with(".json")
            || trimmed.ends_with(".png")
            || trimmed.ends_with(".svg")
        {
            return false;
        }
    }
    true
}

/// Translate `input` into `target` writing into a caller-supplied `#![no_std]` buffer.
/// Returns the translated `&str` slice backed by `buf`.
pub fn translate_into_buf<'a>(input: &str, target: Language, buf: &'a mut [u8]) -> &'a str {
    let mut w = BufWriter::new(buf);
    translate_into_writer(input, target, &mut w);
    let len = w.len();
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

/// Core `#![no_std]` translation algorithm writing into `BufWriter`.
pub fn translate_into_writer(input: &str, target: Language, w: &mut BufWriter<'_>) {
    if input.is_empty() {
        return;
    }

    // Preserve leading & trailing whitespace
    let trimmed_start = input.trim_start();
    let leading_ws_len = input.len() - trimmed_start.len();
    let trimmed = trimmed_start.trim_end();
    let trailing_ws_len = trimmed_start.len() - trimmed.len();

    if leading_ws_len > 0 {
        w.push_str(&input[..leading_ws_len]);
    }

    // Separate trailing punctuation (e.g. ":", "...", "!", "?", ".")
    let mut core_end = trimmed.len();
    for (idx, ch) in trimmed.char_indices().rev() {
        if matches!(ch, '.' | ':' | ';' | '!' | '?' | ',') {
            core_end = idx;
        } else {
            break;
        }
    }

    // Check if the full trimmed string matches a phrase directly (e.g. "Cargando...")
    if let Some(exact) = lookup_phrase(trimmed, target) {
        w.push_with_casing(exact, detect_casing(trimmed));
        if trailing_ws_len > 0 {
            w.push_str(&trimmed_start[trimmed.len()..]);
        }
        return;
    }

    // Separate leading Spanish inverted punctuation (¡ / ¿)
    let mut core_start = 0usize;
    for (idx, ch) in trimmed[..core_end].char_indices() {
        if matches!(ch, '¡' | '¿') {
            core_start = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    let core = &trimmed[core_start..core_end];
    let trailing_punct = &trimmed[core_end..];
    let leading_punct = &trimmed[..core_start];

    let emit_leading_punct = !matches!(target, Language::En | Language::Pt | Language::Fr | Language::De | Language::It);
    if emit_leading_punct && !leading_punct.is_empty() {
        w.push_str(leading_punct);
    }

    if let Some(phrase) = lookup_phrase(core, target) {
        w.push_with_casing(phrase, detect_casing(core));
    } else {
        // Compositional word-by-word translation keeping formatting placeholders `{...}` and punctuation
        translate_compositional(core, target, w);
    }

    if !trailing_punct.is_empty() {
        w.push_str(trailing_punct);
    }
    if trailing_ws_len > 0 {
        w.push_str(&trimmed_start[trimmed.len()..]);
    }
}

fn translate_compositional(core: &str, target: Language, w: &mut BufWriter<'_>) {
    let bytes = core.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Preserve format placeholders like `{}` or `{name}` verbatim
        if bytes[i] == b'{' {
            if let Some(rel_end) = core[i..].find('}') {
                let end = i + rel_end + 1;
                w.push_str(&core[i..end]);
                i = end;
                continue;
            }
        }
        // Preserve `%s`, `%d`, etc.
        if bytes[i] == b'%' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
            w.push_str(&core[i..i + 2]);
            i += 2;
            continue;
        }

        let ch = core[i..].chars().next().unwrap();
        if is_punct_or_space(ch) {
            w.push_char(ch);
            i += ch.len_utf8();
            continue;
        }

        // Collect word span
        let start = i;
        while i < bytes.len() {
            let c = core[i..].chars().next().unwrap();
            if is_punct_or_space(c) || c == '{' {
                break;
            }
            i += c.len_utf8();
        }
        let word = &core[start..i];
        if let Some(translated) = lookup_word(word, target) {
            w.push_with_casing(translated, detect_casing(word));
        } else {
            w.push_str(word);
        }
    }
}

/// Translate `input` into all 6 built-in languages `[Es, En, Pt, Fr, De, It]` into caller buffers.
#[cfg(feature = "alloc")]
pub fn translate_owned(input: &str, target: Language) -> String {
    let mut buf = [0u8; 1024];
    let s = translate_into_buf(input, target, &mut buf);
    String::from(s)
}

#[cfg(feature = "alloc")]
pub fn translate_all(input: &str) -> [String; Language::COUNT] {
    [
        translate_owned(input, Language::Es),
        translate_owned(input, Language::En),
        translate_owned(input, Language::Pt),
        translate_owned(input, Language::Fr),
        translate_owned(input, Language::De),
        translate_owned(input, Language::It),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_phrase_translation_preserves_casing_and_punct() {
        let mut buf = [0u8; 128];
        assert_eq!(translate_into_buf("Guardar cambios", Language::En, &mut buf), "Save changes");
        assert_eq!(translate_into_buf("GUARDAR", Language::En, &mut buf), "SAVE");
        assert_eq!(translate_into_buf("Cargando...", Language::Fr, &mut buf), "Chargement...");
        assert_eq!(translate_into_buf("Bienvenido al sistema!", Language::En, &mut buf), "Welcome to the system!");
    }

    #[test]
    fn compositional_translation_works_offline() {
        let mut buf = [0u8; 256];
        let out = translate_into_buf("Cambiar color del botón", Language::En, &mut buf);
        assert_eq!(out, "Change color of the button");

        let out2 = translate_into_buf("Estado: {} activo", Language::En, &mut buf);
        assert_eq!(out2, "Status: {} active");
    }

    #[test]
    fn detects_source_language() {
        assert_eq!(detect_language("Configuración del sistema"), Language::Es);
        assert_eq!(detect_language("Save changes"), Language::En);
        assert_eq!(detect_language("Alterações do sistema"), Language::Pt);
    }

    #[test]
    fn filters_technical_strings() {
        assert!(!is_translatable_ui_text("https://example.com"));
        assert!(!is_translatable_ui_text("src/main.rs"));
        assert!(!is_translatable_ui_text("user_id"));
        assert!(!is_translatable_ui_text("{:?}"));
        assert!(is_translatable_ui_text("Hola mundo"));
        assert!(is_translatable_ui_text("Guardar"));
    }
}

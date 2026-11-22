#![no_std]
//! # `lang` — Bare-Metal Language Switcher & Compile-Time Project Translator
//!
//! `lang` is a 100% `#![no_std]` crate with **zero runtime OS, network, or heap dependencies**.
//! It lets any application or bare-metal system:
//! 1. Place a language-switcher button into any ready UI slot via [`ui::LangButton`] and [`ui::LangHost`].
//! 2. Automatically discover and offline pre-translate project strings at compile time using
//!    `#[lang::auto]` (or `lang::scan_project!()` / `lang-build` in `build.rs`), so changing the
//!    language works instantaneously with or without Wi-Fi.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod ui;

pub use lang_translator::{
    detect_language, fnv1a_64, is_translatable_ui_text, translate_into_buf, BufWriter, Language,
};

#[cfg(feature = "alloc")]
pub use lang_translator::{translate_all, translate_owned};

#[cfg(feature = "macros")]
pub use lang_macros::{app, auto, scan_project, t, translate};

#[cfg(feature = "atomics")]
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

#[cfg(feature = "atomics")]
static GLOBAL_LANG: AtomicU8 = AtomicU8::new(Language::Es as u8);

#[cfg(feature = "atomics")]
static GLOBAL_CATALOG: AtomicPtr<LangCatalog> = AtomicPtr::new(core::ptr::null_mut());

#[cfg(feature = "atomics")]
static GLOBAL_OVERRIDES: AtomicPtr<LangCatalog> = AtomicPtr::new(core::ptr::null_mut());

/// Single compile-time entry inside a [`LangCatalog`].
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub hash: u64,
    pub source: &'static str,
    pub translations: [&'static str; Language::COUNT],
}

impl CatalogEntry {
    #[inline]
    pub const fn new(
        hash: u64,
        source: &'static str,
        translations: [&'static str; Language::COUNT],
    ) -> Self {
        Self {
            hash,
            source,
            translations,
        }
    }
}

/// Static, zero-allocation catalog of pre-translated project strings.
///
/// Generated automatically by `lang::scan_project!()` or `lang_build::generate_catalog()`.
#[derive(Debug, Clone, Copy)]
pub struct LangCatalog {
    entries: &'static [CatalogEntry],
}

impl LangCatalog {
    #[inline]
    pub const fn new(entries: &'static [CatalogEntry]) -> Self {
        Self { entries }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a pre-translated string by exact source literal.
    ///
    /// Uses binary search when entries are hash-sorted (default for generated catalogs),
    /// with linear fallback if a custom unsorted slice is provided.
    pub fn lookup(&self, source: &str, target: Language) -> Option<&'static str> {
        let target_hash = fnv1a_64(source.as_bytes());
        let len = self.entries.len();
        if len == 0 {
            return None;
        }

        // Fast binary search on hash
        let mut lo = 0usize;
        let mut hi = len;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let h = self.entries[mid].hash;
            if h < target_hash {
                lo = mid + 1;
            } else if h > target_hash {
                hi = mid;
            } else {
                // Hash matched; verify string equality and scan neighbors on collision
                if self.entries[mid].source == source {
                    return Some(self.entries[mid].translations[target.index()]);
                }
                let mut left = mid;
                while left > 0 && self.entries[left - 1].hash == target_hash {
                    left -= 1;
                    if self.entries[left].source == source {
                        return Some(self.entries[left].translations[target.index()]);
                    }
                }
                let mut right = mid + 1;
                while right < len && self.entries[right].hash == target_hash {
                    if self.entries[right].source == source {
                        return Some(self.entries[right].translations[target.index()]);
                    }
                    right += 1;
                }
                break;
            }
        }

        // Linear fallback for unsorted hand-crafted catalogs
        for entry in self.entries {
            if entry.hash == target_hash && entry.source == source {
                return Some(entry.translations[target.index()]);
            }
        }
        None
    }
}

/// Non-global language engine instance for bare-metal targets without atomics
/// or apps managing multiple isolated language contexts.
#[derive(Debug, Clone, Copy)]
pub struct LangEngine {
    current: Language,
    catalog: Option<&'static LangCatalog>,
    overrides: Option<&'static LangCatalog>,
}

impl LangEngine {
    #[inline]
    pub const fn new(initial: Language) -> Self {
        Self {
            current: initial,
            catalog: None,
            overrides: None,
        }
    }

    #[inline]
    pub const fn with_catalog(initial: Language, catalog: &'static LangCatalog) -> Self {
        Self {
            current: initial,
            catalog: Some(catalog),
            overrides: None,
        }
    }

    #[inline]
    pub const fn with_overrides(
        initial: Language,
        catalog: Option<&'static LangCatalog>,
        overrides: &'static LangCatalog,
    ) -> Self {
        Self {
            current: initial,
            catalog,
            overrides: Some(overrides),
        }
    }

    #[inline]
    pub const fn current(&self) -> Language {
        self.current
    }

    #[inline]
    pub fn set(&mut self, lang: Language) {
        self.current = lang;
    }

    #[inline]
    pub fn cycle(&mut self, allowed: &[Language]) -> Language {
        if allowed.is_empty() {
            self.current = self.current.next();
            return self.current;
        }
        let pos = allowed.iter().position(|&l| l == self.current).unwrap_or(0);
        let next = allowed[(pos + 1) % allowed.len()];
        self.current = next;
        next
    }

    /// Select from compile-time pre-translated variants `[Es, En, Pt, Fr, De, It]`.
    #[inline]
    pub fn select(&self, variants: &'static [&'static str; Language::COUNT]) -> &'static str {
        variants[self.current.index()]
    }

    /// Translate `source` using overrides first, then catalog, falling back to `source`.
    #[inline]
    pub fn tr(&self, source: &'static str) -> &'static str {
        if let Some(ov) = self.overrides {
            if let Some(found) = ov.lookup(source, self.current) {
                return found;
            }
        }
        if let Some(cat) = self.catalog {
            if let Some(found) = cat.lookup(source, self.current) {
                return found;
            }
        }
        source
    }

    /// Translate `source` into `buf` (uses overrides first, then catalog, then `#![no_std]` offline translator).
    pub fn tr_buf<'a>(&self, source: &str, buf: &'a mut [u8]) -> &'a str {
        if let Some(ov) = self.overrides {
            if let Some(found) = ov.lookup(source, self.current) {
                let bytes = found.as_bytes();
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                return core::str::from_utf8(&buf[..n]).unwrap_or("");
            }
        }
        if let Some(cat) = self.catalog {
            if let Some(found) = cat.lookup(source, self.current) {
                let bytes = found.as_bytes();
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                return core::str::from_utf8(&buf[..n]).unwrap_or("");
            }
        }
        translate_into_buf(source, self.current, buf)
    }
}

/// Set the global active language.
#[cfg(feature = "atomics")]
#[inline]
pub fn set_language(lang: Language) {
    GLOBAL_LANG.store(lang as u8, Ordering::Relaxed);
}

/// Read the global active language.
#[cfg(feature = "atomics")]
#[inline]
pub fn current_language() -> Language {
    Language::from_u8(GLOBAL_LANG.load(Ordering::Relaxed)).unwrap_or(Language::Es)
}

/// Advance to the next language in `allowed` (or [`Language::ALL`] if empty) and return it.
#[cfg(feature = "atomics")]
pub fn cycle_language(allowed: &[Language]) -> Language {
    let cur = current_language();
    let list = if allowed.is_empty() {
        &Language::ALL[..]
    } else {
        allowed
    };
    let pos = list.iter().position(|&l| l == cur).unwrap_or(0);
    let next = list[(pos + 1) % list.len()];
    set_language(next);
    next
}

/// Register a static [`LangCatalog`] (generated by `scan_project!()` or `include_catalog!()`)
/// as the global project translation catalog.
#[cfg(feature = "atomics")]
#[inline]
pub fn install_catalog(catalog: &'static LangCatalog) {
    GLOBAL_CATALOG.store(catalog as *const LangCatalog as *mut LangCatalog, Ordering::Release);
}

/// Register a static [`LangCatalog`] as high-priority custom glossary overrides.
#[cfg(feature = "atomics")]
#[inline]
pub fn install_overrides(overrides: &'static LangCatalog) {
    GLOBAL_OVERRIDES.store(overrides as *const LangCatalog as *mut LangCatalog, Ordering::Release);
}

/// Select the string variant matching the current global language.
/// Used transparently by `#[lang::auto]` and `lang::t!`.
#[cfg(feature = "atomics")]
#[inline]
pub fn select(variants: &'static [&'static str; Language::COUNT]) -> &'static str {
    variants[current_language().index()]
}

/// Look up a static string in global overrides or [`LangCatalog`] for the current language.
#[cfg(feature = "atomics")]
#[inline]
pub fn tr(source: &'static str) -> &'static str {
    let ov_ptr = GLOBAL_OVERRIDES.load(Ordering::Acquire);
    if !ov_ptr.is_null() {
        let ov = unsafe { &*ov_ptr };
        if let Some(found) = ov.lookup(source, current_language()) {
            return found;
        }
    }
    let ptr = GLOBAL_CATALOG.load(Ordering::Acquire);
    if !ptr.is_null() {
        // SAFETY: `install_catalog` only stores `&'static LangCatalog`.
        let cat = unsafe { &*ptr };
        if let Some(found) = cat.lookup(source, current_language()) {
            return found;
        }
    }
    source
}

/// Translate `source` into caller-supplied buffer `buf` using global overrides, catalog, or offline translator.
#[cfg(feature = "atomics")]
pub fn tr_buf<'a>(source: &str, buf: &'a mut [u8]) -> &'a str {
    let lang = current_language();
    let ov_ptr = GLOBAL_OVERRIDES.load(Ordering::Acquire);
    if !ov_ptr.is_null() {
        let ov = unsafe { &*ov_ptr };
        if let Some(found) = ov.lookup(source, lang) {
            let bytes = found.as_bytes();
            let n = bytes.len().min(buf.len());
            buf[..n].copy_from_slice(&bytes[..n]);
            return core::str::from_utf8(&buf[..n]).unwrap_or("");
        }
    }
    let ptr = GLOBAL_CATALOG.load(Ordering::Acquire);
    if !ptr.is_null() {
        // SAFETY: `install_catalog` only stores `&'static LangCatalog`.
        let cat = unsafe { &*ptr };
        if let Some(found) = cat.lookup(source, lang) {
            let bytes = found.as_bytes();
            let n = bytes.len().min(buf.len());
            buf[..n].copy_from_slice(&bytes[..n]);
            return core::str::from_utf8(&buf[..n]).unwrap_or("");
        }
    }
    translate_into_buf(source, lang, buf)
}

/// Include the catalog generated by `lang_build::generate_catalog()` in `build.rs`.
#[macro_export]
macro_rules! include_catalog {
    () => {
        include!(concat!(env!("OUT_DIR"), "/lang_catalog.rs"));
    };
}

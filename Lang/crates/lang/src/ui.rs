//! Bare-metal, UI-framework-agnostic button & ready-slot mounting API.
//!
//! Any host application (embedded display, OS kernel, game engine, TUI, WASM canvas)
//! implements [`LangHost`] with two minimal methods and mounts a [`LangButton`].

#[cfg(feature = "atomics")]
use crate::{current_language, set_language, tr_buf};
use crate::{LangCatalog, LangEngine, Language};

/// Axis-aligned rectangle in host coordinate space (pixels, terminal cells, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    #[inline]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub const fn contains(&self, px: i32, py: i32) -> bool {
        let x2 = self.x.saturating_add(self.width as i32);
        let y2 = self.y.saturating_add(self.height as i32);
        px >= self.x && px < x2 && py >= self.y && py < y2
    }
}

/// Interaction behavior for [`LangButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonMode {
    /// Each click/tap immediately cycles to the next configured language.
    Cycle,
    /// Clicking toggles a language option list; tapping an option selects that language.
    Selector,
}

/// Visual interaction state of the mounted button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Idle,
    Hovered,
    Pressed,
    /// Language list is currently open (when [`ButtonMode::Selector`] is active).
    Open,
}

/// How the button's visible label is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Short uppercase code: `"ES"`, `"EN"`, ...
    CodeOnly,
    /// Native language name: `"Español"`, `"English"`, ...
    NativeName,
    /// Combined label: `"ES · Español"` (resolved at compile time as `&'static str`).
    CodeAndName,
}

impl LabelStyle {
    /// Recommended minimum pixel width for a button using this label style.
    #[inline]
    pub const fn suggested_width(self) -> u32 {
        match self {
            Self::CodeOnly => 48,
            Self::NativeName => 104,
            Self::CodeAndName => 136,
        }
    }
}

/// Configuration for mounting a [`LangButton`] into a host's ready slot.
#[derive(Debug, Clone, Copy)]
pub struct ButtonConfig {
    pub bounds: Rect,
    pub languages: &'static [Language],
    pub mode: ButtonMode,
    pub label_style: LabelStyle,
}

impl ButtonConfig {
    /// Returns the suggested minimum button width for the configured [`LabelStyle`].
    #[inline]
    pub const fn suggested_width(&self) -> u32 {
        self.label_style.suggested_width()
    }
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            bounds: Rect::new(0, 0, 64, 28),
            languages: &Language::ALL,
            mode: ButtonMode::Cycle,
            label_style: LabelStyle::CodeOnly,
        }
    }
}

/// Snapshot passed to [`LangHost::draw_lang_button`] whenever the button mounts or updates.
#[derive(Debug, Clone, Copy)]
pub struct ButtonView<'a> {
    pub bounds: Rect,
    pub label: &'static str,
    pub active: Language,
    pub state: ButtonState,
    /// Non-empty when [`ButtonMode::Selector`] is open, listing selectable languages.
    pub options: &'a [Language],
}

impl<'a> ButtonView<'a> {
    /// Recommended minimum width in pixels for the rendered `label` text.
    #[inline]
    pub const fn suggested_width(&self) -> u32 {
        let approx = (self.label.len() as u32).saturating_mul(8).saturating_add(24);
        if approx < 48 { 48 } else { approx }
    }

    /// Helper geometry for dropdown option `index` placed stacked directly under `bounds`.
    #[inline]
    pub const fn option_bounds(&self, index: usize) -> Rect {
        let offset = (index as i32 + 1) * (self.bounds.height as i32);
        Rect::new(
            self.bounds.x,
            self.bounds.y.saturating_add(offset),
            self.bounds.width,
            self.bounds.height,
        )
    }
}

/// Input events routed from the host project into [`LangButton::handle_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// Direct activation on the mounted button (e.g. host already performed hit-testing).
    Click,
    /// Raw pointer/touch tap at host coordinates `(x, y)`.
    Tap { x: i32, y: i32 },
    /// Pointer hover/move at host coordinates `(x, y)`.
    PointerMove { x: i32, y: i32 },
    /// Hardware key / rotary button advancing to the next language.
    KeyNext,
    /// Explicit programmatic language selection.
    Select(Language),
}

/// Bare-metal contract implemented by any application or system that integrates `lang`.
///
/// Defines both how `lang` injects the language-switcher button into a ready UI slot
/// and how it reads/updates the host system's texts when the language changes.
pub trait LangHost {
    /// Returns the bounds of the ready slot where the system expects the button to be injected.
    fn ready_slot(&self) -> Rect {
        Rect::new(0, 0, 64, 28)
    }

    /// Render or update the language button inside the ready UI slot.
    fn draw_lang_button(&mut self, view: &ButtonView<'_>);

    /// Number of retained text nodes managed by the host (`0` for immediate-mode `#[lang::app]` UIs).
    fn text_count(&self) -> usize {
        0
    }

    /// Returns the original source text for retained node `index` so `lang` can translate it.
    fn source_text(&self, _index: usize) -> Option<&str> {
        None
    }

    /// Receives the translated text for retained node `index` after a language change.
    fn set_text(&mut self, _index: usize, _translated: &str) {}

    /// Called immediately after the active language changes (and retained texts are updated)
    /// so the host can repaint its UI.
    fn on_language_changed(&mut self, new_lang: Language);
}

/// Bare-metal mountable language-switcher button widget.
///
/// Holds zero heap memory and interacts with the host strictly via [`LangHost`].
#[derive(Debug, Clone)]
pub struct LangButton {
    config: ButtonConfig,
    state: ButtonState,
    open: bool,
    active_index: usize,
}

impl LangButton {
    /// Create and immediately mount the button into `host`'s declared [`LangHost::ready_slot`].
    #[cfg(feature = "atomics")]
    pub fn mount_in_slot<H: LangHost>(
        languages: &'static [Language],
        mode: ButtonMode,
        label_style: LabelStyle,
        host: &mut H,
    ) -> Self {
        let config = ButtonConfig {
            bounds: host.ready_slot(),
            languages,
            mode,
            label_style,
        };
        Self::mount(config, host)
    }

    /// Create and immediately mount the button into `host`'s ready slot using global atomic state.
    #[cfg(feature = "atomics")]
    pub fn mount<H: LangHost>(config: ButtonConfig, host: &mut H) -> Self {
        let btn = Self::new_unmounted(config, current_language());
        btn.sync_host_button(host);
        btn
    }

    /// Create and mount the button using an explicit [`LangEngine`] (useful on targets without atomics).
    pub fn mount_with_engine<H: LangHost>(
        config: ButtonConfig,
        engine: &LangEngine,
        host: &mut H,
    ) -> Self {
        let btn = Self::new_unmounted(config, engine.current());
        btn.sync_host_button(host);
        btn
    }

    /// Construct a button initialized to `initial_lang` without yet calling [`LangHost::draw_lang_button`].
    pub fn new_unmounted(config: ButtonConfig, initial_lang: Language) -> Self {
        let langs = if config.languages.is_empty() {
            &Language::ALL[..]
        } else {
            config.languages
        };
        let active_index = langs
            .iter()
            .position(|&l| l == initial_lang)
            .unwrap_or(0);
        Self {
            config: ButtonConfig {
                languages: langs,
                ..config
            },
            state: ButtonState::Idle,
            open: false,
            active_index,
        }
    }

    /// Currently selected language.
    #[inline]
    pub fn active(&self) -> Language {
        self.config.languages[self.active_index]
    }

    /// Current button bounds.
    #[inline]
    pub const fn bounds(&self) -> Rect {
        self.config.bounds
    }

    /// Reposition/resize the button in the host slot and re-render it.
    pub fn set_bounds<H: LangHost>(&mut self, bounds: Rect, host: &mut H) {
        self.config.bounds = bounds;
        self.sync_host_button(host);
    }

    /// Build the current [`ButtonView`] snapshot.
    pub fn view(&self) -> ButtonView<'_> {
        let active = self.active();
        let state = if self.open {
            ButtonState::Open
        } else {
            self.state
        };
        let options = if self.open && self.config.mode == ButtonMode::Selector {
            self.config.languages
        } else {
            &[]
        };
        ButtonView {
            bounds: self.config.bounds,
            label: format_label(active, self.config.label_style),
            active,
            state,
            options,
        }
    }

    /// Handle an input event using the global atomic engine. Returns `true` if the active language changed.
    #[cfg(feature = "atomics")]
    pub fn handle_event<H: LangHost>(&mut self, event: UiEvent, host: &mut H) -> bool {
        let mut changed_to = None;
        let handled = self.process_event(event, &mut changed_to);
        if let Some(new_lang) = changed_to {
            set_language(new_lang);
            self.sync_host_button(host);
            Self::sync_host_texts_global(host);
            host.on_language_changed(new_lang);
            return true;
        }
        if handled {
            self.sync_host_button(host);
        }
        false
    }

    /// Handle an input event using an explicit [`LangEngine`]. Returns `true` if the active language changed.
    pub fn handle_event_with_engine<H: LangHost>(
        &mut self,
        event: UiEvent,
        engine: &mut LangEngine,
        host: &mut H,
    ) -> bool {
        let mut changed_to = None;
        let handled = self.process_event(event, &mut changed_to);
        if let Some(new_lang) = changed_to {
            engine.set(new_lang);
            self.sync_host_button(host);
            Self::sync_host_texts_with_engine(engine, host);
            host.on_language_changed(new_lang);
            return true;
        }
        if handled {
            self.sync_host_button(host);
        }
        false
    }

    #[cfg(feature = "atomics")]
    fn sync_host_texts_global<H: LangHost>(host: &mut H) {
        let count = host.text_count();
        let mut i = 0;
        while i < count {
            let mut buf = [0u8; 256];
            let translated_len = if let Some(src) = host.source_text(i) {
                Some(tr_buf(src, &mut buf).len())
            } else {
                None
            };
            if let Some(len) = translated_len {
                if let Ok(translated) = core::str::from_utf8(&buf[..len]) {
                    host.set_text(i, translated);
                }
            }
            i += 1;
        }
    }

    fn sync_host_texts_with_engine<H: LangHost>(engine: &LangEngine, host: &mut H) {
        let count = host.text_count();
        let mut i = 0;
        while i < count {
            let mut buf = [0u8; 256];
            let translated_len = if let Some(src) = host.source_text(i) {
                Some(engine.tr_buf(src, &mut buf).len())
            } else {
                None
            };
            if let Some(len) = translated_len {
                if let Ok(translated) = core::str::from_utf8(&buf[..len]) {
                    host.set_text(i, translated);
                }
            }
            i += 1;
        }
    }

    fn sync_host_button<H: LangHost>(&self, host: &mut H) {
        let v = self.view();
        host.draw_lang_button(&v);
    }

    fn process_event(&mut self, event: UiEvent, changed_to: &mut Option<Language>) -> bool {
        match event {
            UiEvent::PointerMove { x, y } => {
                let hovered = self.config.bounds.contains(x, y);
                let next_state = if hovered {
                    ButtonState::Hovered
                } else {
                    ButtonState::Idle
                };
                if next_state != self.state {
                    self.state = next_state;
                    return true;
                }
                false
            }
            UiEvent::KeyNext => {
                self.advance_cycle(changed_to);
                true
            }
            UiEvent::Select(lang) => {
                if let Some(pos) = self.config.languages.iter().position(|&l| l == lang) {
                    self.open = false;
                    self.state = ButtonState::Idle;
                    if pos != self.active_index {
                        self.active_index = pos;
                        *changed_to = Some(lang);
                    }
                    return true;
                }
                false
            }
            UiEvent::Click => {
                self.activate_main_button(changed_to);
                true
            }
            UiEvent::Tap { x, y } => {
                if self.open && self.config.mode == ButtonMode::Selector {
                    let v = self.view();
                    for (idx, &lang) in self.config.languages.iter().enumerate() {
                        if v.option_bounds(idx).contains(x, y) {
                            self.open = false;
                            self.state = ButtonState::Idle;
                            if idx != self.active_index {
                                self.active_index = idx;
                                *changed_to = Some(lang);
                            }
                            return true;
                        }
                    }
                }
                if self.config.bounds.contains(x, y) {
                    self.activate_main_button(changed_to);
                    return true;
                }
                if self.open {
                    self.open = false;
                    self.state = ButtonState::Idle;
                    return true;
                }
                false
            }
        }
    }

    fn activate_main_button(&mut self, changed_to: &mut Option<Language>) {
        match self.config.mode {
            ButtonMode::Cycle => {
                self.advance_cycle(changed_to);
            }
            ButtonMode::Selector => {
                self.open = !self.open;
                self.state = if self.open {
                    ButtonState::Open
                } else {
                    ButtonState::Idle
                };
            }
        }
    }

    fn advance_cycle(&mut self, changed_to: &mut Option<Language>) {
        if self.config.languages.len() <= 1 {
            return;
        }
        self.active_index = (self.active_index + 1) % self.config.languages.len();
        self.open = false;
        self.state = ButtonState::Idle;
        *changed_to = Some(self.config.languages[self.active_index]);
    }
}

#[inline]
const fn format_label(lang: Language, style: LabelStyle) -> &'static str {
    match style {
        LabelStyle::CodeOnly => lang.badge(),
        LabelStyle::NativeName => lang.native_name(),
        LabelStyle::CodeAndName => match lang {
            Language::Es => "ES · Español",
            Language::En => "EN · English",
            Language::Pt => "PT · Português",
            Language::Fr => "FR · Français",
            Language::De => "DE · Deutsch",
            Language::It => "IT · Italiano",
        },
    }
}

/// Fixed-capacity `#![no_std]` registry for retained UI text nodes (e.g. static LCD/widget labels).
///
/// When the active language changes, calling [`TextSlots::refresh`] translates every bound slot
/// from a [`LangCatalog`] with zero heap allocations.
pub struct TextSlots<const N: usize> {
    sources: [&'static str; N],
    rendered: [&'static str; N],
    len: usize,
}

impl<const N: usize> TextSlots<N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            sources: [""; N],
            rendered: [""; N],
            len: 0,
        }
    }

    /// Bind a new text slot, returning its handle index, or `None` if capacity `N` is full.
    pub fn bind(&mut self, source: &'static str) -> Option<usize> {
        if self.len >= N {
            return None;
        }
        let idx = self.len;
        self.sources[idx] = source;
        self.rendered[idx] = source;
        self.len += 1;
        Some(idx)
    }

    /// Re-translate all bound slots against `catalog` for `lang`.
    pub fn refresh(&mut self, catalog: &LangCatalog, lang: Language) {
        let mut i = 0;
        while i < self.len {
            let src = self.sources[i];
            self.rendered[i] = catalog.lookup(src, lang).unwrap_or(src);
            i += 1;
        }
    }

    /// Read the currently rendered text for slot `id`.
    #[inline]
    pub fn get(&self, id: usize) -> &'static str {
        if id < self.len {
            self.rendered[id]
        } else {
            ""
        }
    }
}

impl<const N: usize> Default for TextSlots<N> {
    fn default() -> Self {
        Self::new()
    }
}

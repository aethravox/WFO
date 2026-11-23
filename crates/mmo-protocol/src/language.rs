#![allow(clippy::all)]
use super::*;
use ::lang::ui::{ButtonMode, ButtonView, LabelStyle, LangButton, LangHost, Rect, UiEvent};
use ::lang::Language;

pub struct MmoLangHost {
    pub slot: Rect,
    pub btn_bounds: Rect,
    pub btn_label: &'static str,
    pub current: Language,
}

impl MmoLangHost {
    pub fn new(slot: Rect) -> Self {
        Self {
            slot,
            btn_bounds: slot,
            btn_label: "ES",
            current: Language::Es,
        }
    }
}

impl LangHost for MmoLangHost {
    fn ready_slot(&self) -> Rect {
        self.slot
    }

    fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
        self.btn_bounds = view.bounds;
        self.btn_label = view.label;
    }

    fn on_language_changed(&mut self, new_lang: Language) {
        self.current = new_lang;
    }
}

pub struct MmoLangController {
    pub host: MmoLangHost,
    pub button: LangButton,
}

impl MmoLangController {
    pub fn new(slot: Rect) -> Self {
        ::lang::install_overrides(&MMO_CATALOG);
        ::lang::set_language(Language::Es);
        let mut host = MmoLangHost::new(slot);
        let button = LangButton::mount_in_slot(
            &Language::ALL,
            ButtonMode::Cycle,
            LabelStyle::CodeOnly,
            &mut host,
        );
        Self { host, button }
    }

    pub fn set_slot(&mut self, slot: Rect) {
        self.host.slot = slot;
        self.host.btn_bounds = slot;
    }

    pub fn handle_tap(&mut self, x: i32, y: i32) -> bool {
        if self.host.btn_bounds.contains(x, y) {
            self.button.handle_event(UiEvent::Click, &mut self.host)
        } else {
            false
        }
    }

    pub fn cycle(&mut self) {
        self.button.handle_event(UiEvent::Click, &mut self.host);
    }

    pub fn tr(&self, key: &'static str) -> &'static str {
        ::lang::tr(key)
    }

    pub fn tr_auto_connect(&self) -> &'static str {
        crate::math::auto_labels::basics()[0]
    }
}

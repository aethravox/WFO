use lang::ui::{
    ButtonConfig, ButtonMode, ButtonState, ButtonView, LabelStyle, LangButton, LangHost, Rect,
    TextSlots, UiEvent,
};
use lang::{CatalogEntry, LangCatalog, LangEngine, Language};

/// Simulated bare-metal framebuffer/UI surface with a ready slot for `LangButton`.
struct MockBareMetalApp {
    button_label: &'static str,
    button_bounds: Rect,
    button_state: ButtonState,
    open_options: Vec<Language>,
    screen_lines: [&'static str; 4],
    repaint_count: usize,
}

impl MockBareMetalApp {
    fn new() -> Self {
        Self {
            button_label: "",
            button_bounds: Rect::new(0, 0, 0, 0),
            button_state: ButtonState::Idle,
            open_options: Vec::new(),
            screen_lines: [""; 4],
            repaint_count: 0,
        }
    }

    fn draw_screen(&mut self) {
        self.screen_lines = app_ui::render_texts();
    }
}

impl LangHost for MockBareMetalApp {
    fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
        self.button_label = view.label;
        self.button_bounds = view.bounds;
        self.button_state = view.state;
        self.open_options = view.options.to_vec();
    }

    fn on_language_changed(&mut self, _new_lang: Language) {
        self.repaint_count += 1;
        self.draw_screen();
    }
}

/// The developer writes normal strings in Spanish inside `#[lang::app]` — zero `tr!()` calls!
#[lang::app]
mod app_ui {
    pub fn render_texts() -> [&'static str; 4] {
        let header = "Bienvenido al sistema";
        let save_btn = "Guardar cambios";
        let status = "Sistema listo";
        let action = match 1u8 {
            1 => "Cambiar color del botón",
            _ => "Cancelar",
        };
        [header, save_btn, status, action]
    }
}

#[test]
fn auto_rewrites_and_button_switches_entire_app_offline() {
    lang::set_language(Language::Es);

    let mut host = MockBareMetalApp::new();
    host.draw_screen();

    assert_eq!(
        host.screen_lines,
        [
            "Bienvenido al sistema",
            "Guardar cambios",
            "Sistema listo",
            "Cambiar color del botón",
        ]
    );

    // Mount the language button into the host's ready slot at (240, 8, 64, 24)
    let mut btn = LangButton::mount(
        ButtonConfig {
            bounds: Rect::new(240, 8, 64, 24),
            languages: &[Language::Es, Language::En, Language::Pt, Language::Fr],
            mode: ButtonMode::Cycle,
            label_style: LabelStyle::CodeOnly,
        },
        &mut host,
    );

    assert_eq!(host.button_label, "ES");
    assert_eq!(host.repaint_count, 0);

    // Tap outside the button: nothing changes
    assert!(!btn.handle_event(UiEvent::Tap { x: 10, y: 10 }, &mut host));
    assert_eq!(host.button_label, "ES");

    // Tap inside the button: switches to English and repaints all app texts automatically!
    assert!(btn.handle_event(UiEvent::Tap { x: 260, y: 20 }, &mut host));
    assert_eq!(host.button_label, "EN");
    assert_eq!(host.repaint_count, 1);
    assert_eq!(
        host.screen_lines,
        [
            "Welcome to the system",
            "Save changes",
            "System ready",
            "Change color of the button",
        ]
    );

    // Tap again -> Portuguese
    assert!(btn.handle_event(UiEvent::Click, &mut host));
    assert_eq!(host.button_label, "PT");
    assert_eq!(
        host.screen_lines,
        [
            "Bem-vindo ao sistema",
            "Salvar alterações",
            "Sistema pronto",
            "Mudar cor do botão",
        ]
    );

    // Tap again -> French
    assert!(btn.handle_event(UiEvent::KeyNext, &mut host));
    assert_eq!(host.button_label, "FR");
    assert_eq!(
        host.screen_lines,
        [
            "Bienvenue dans le système",
            "Enregistrer les modifications",
            "Système prêt",
            "Changer couleur du bouton",
        ]
    );

    // Restore global state
    lang::set_language(Language::Es);
}

#[test]
fn dropdown_selector_mode_and_geometry() {
    let mut engine = LangEngine::new(Language::Es);
    let mut host = MockBareMetalApp::new();

    let mut btn = LangButton::mount_with_engine(
        ButtonConfig {
            bounds: Rect::new(10, 10, 120, 30),
            languages: &[Language::Es, Language::En, Language::De],
            mode: ButtonMode::Selector,
            label_style: LabelStyle::CodeAndName,
        },
        &engine,
        &mut host,
    );

    assert_eq!(host.button_label, "ES · Español");
    assert!(host.open_options.is_empty());

    // First tap opens the language selector without changing language yet
    assert!(!btn.handle_event_with_engine(UiEvent::Tap { x: 20, y: 20 }, &mut engine, &mut host));
    assert_eq!(host.button_state, ButtonState::Open);
    assert_eq!(host.open_options, vec![Language::Es, Language::En, Language::De]);

    // Tap 3rd option ("DE"): option 0 is y=40..70, option 1 is y=70..100, option 2 is y=100..130
    assert!(btn.handle_event_with_engine(UiEvent::Tap { x: 20, y: 110 }, &mut engine, &mut host));
    assert_eq!(engine.current(), Language::De);
    assert_eq!(host.button_label, "DE · Deutsch");
    assert_eq!(host.button_state, ButtonState::Idle);
    assert!(host.open_options.is_empty());
}

#[test]
fn engine_and_text_slots_zero_alloc_flow() {
    static CATALOG: LangCatalog = LangCatalog::new(&[
        CatalogEntry::new(
            lang::fnv1a_64(b"Guardar"),
            "Guardar",
            ["Guardar", "Save", "Salvar", "Enregistrer", "Speichern", "Salva"],
        ),
        CatalogEntry::new(
            lang::fnv1a_64(b"Cancelar"),
            "Cancelar",
            ["Cancelar", "Cancel", "Cancelar", "Annuler", "Abbrechen", "Annulla"],
        ),
    ]);

    let mut engine = LangEngine::with_catalog(Language::Es, &CATALOG);
    let mut slots: TextSlots<4> = TextSlots::new();
    let save_id = slots.bind("Guardar").unwrap();
    let cancel_id = slots.bind("Cancelar").unwrap();

    slots.refresh(&CATALOG, engine.current());
    assert_eq!(slots.get(save_id), "Guardar");
    assert_eq!(slots.get(cancel_id), "Cancelar");

    engine.cycle(&[Language::Es, Language::En]);
    slots.refresh(&CATALOG, engine.current());
    assert_eq!(slots.get(save_id), "Save");
    assert_eq!(slots.get(cancel_id), "Cancel");

    // Zero-alloc buffer fallback for dynamic/unlisted phrases
    let mut buf = [0u8; 64];
    assert_eq!(engine.tr_buf("Error de conexión", &mut buf), "Connection error");
}

#[test]
fn scan_project_discovers_texts_at_compile_time() {
    // Scans `crates/lang/tests` at compile time and embeds the discovered strings!
    static DISCOVERED: LangCatalog = lang::scan_project!("tests");
    assert!(!DISCOVERED.is_empty());
    assert_eq!(
        DISCOVERED.lookup("Guardar cambios", Language::En),
        Some("Save changes")
    );
    assert_eq!(
        DISCOVERED.lookup("Sistema listo", Language::Pt),
        Some("Sistema pronto")
    );
}

#[test]
fn lang_host_manages_ready_slot_and_retained_texts() {
    struct RetainedSystem {
        btn_bounds: Rect,
        btn_label: &'static str,
        sources: [&'static str; 2],
        rendered: [String; 2],
    }

    impl LangHost for RetainedSystem {
        fn ready_slot(&self) -> Rect {
            Rect::new(100, 12, 80, 26)
        }

        fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
            self.btn_bounds = view.bounds;
            self.btn_label = view.label;
        }

        fn text_count(&self) -> usize {
            self.sources.len()
        }

        fn source_text(&self, index: usize) -> Option<&str> {
            self.sources.get(index).copied()
        }

        fn set_text(&mut self, index: usize, translated: &str) {
            if let Some(slot) = self.rendered.get_mut(index) {
                slot.clear();
                slot.push_str(translated);
            }
        }

        fn on_language_changed(&mut self, _new_lang: Language) {}
    }

    lang::set_language(Language::Es);

    let mut sys = RetainedSystem {
        btn_bounds: Rect::new(0, 0, 0, 0),
        btn_label: "",
        sources: ["Iniciar sesión", "Error de conexión"],
        rendered: ["Iniciar sesión".into(), "Error de conexión".into()],
    };

    let mut btn = LangButton::mount_in_slot(
        &[Language::Es, Language::En],
        ButtonMode::Cycle,
        LabelStyle::CodeOnly,
        &mut sys,
    );

    assert_eq!(sys.btn_bounds, Rect::new(100, 12, 80, 26));
    assert_eq!(sys.btn_label, "ES");

    assert!(btn.handle_event(UiEvent::Tap { x: 120, y: 20 }, &mut sys));
    assert_eq!(sys.btn_label, "EN");
    assert_eq!(sys.rendered[0], "Sign in");
    assert_eq!(sys.rendered[1], "Connection error");

    lang::set_language(Language::Es);
}

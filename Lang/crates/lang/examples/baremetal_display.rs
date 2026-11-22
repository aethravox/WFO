use lang::ui::{ButtonConfig, ButtonMode, ButtonView, LabelStyle, LangButton, LangHost, Rect, UiEvent};
use lang::Language;

struct EmbeddedDisplay {
    title: &'static str,
    primary_btn: &'static str,
    lang_btn_text: &'static str,
}

impl EmbeddedDisplay {
    #[lang::auto]
    fn render(&mut self) {
        self.title = "Panel de control";
        self.primary_btn = "Iniciar";
    }
}

impl LangHost for EmbeddedDisplay {
    fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
        self.lang_btn_text = view.label;
    }

    fn on_language_changed(&mut self, _new_lang: Language) {
        self.render();
    }
}

fn main() {
    let mut display = EmbeddedDisplay {
        title: "",
        primary_btn: "",
        lang_btn_text: "",
    };
    display.render();

    let mut lang_button = LangButton::mount(
        ButtonConfig {
            bounds: Rect::new(200, 4, 48, 20),
            languages: &[Language::Es, Language::En, Language::Fr],
            mode: ButtonMode::Cycle,
            label_style: LabelStyle::CodeOnly,
        },
        &mut display,
    );

    println!(
        "[{}] {} | [{}] [{}]",
        lang::current_language().badge(),
        display.title,
        display.primary_btn,
        display.lang_btn_text
    );

    // Simular toque en el botón de idioma
    lang_button.handle_event(UiEvent::Tap { x: 210, y: 12 }, &mut display);

    println!(
        "[{}] {} | [{}] [{}]",
        lang::current_language().badge(),
        display.title,
        display.primary_btn,
        display.lang_btn_text
    );
}

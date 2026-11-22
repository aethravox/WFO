# `lang`

> *La apariencia engaña*: `lang` no es un compilador ni un nuevo lenguaje de programación. Es una librería **`#![no_std]` 100% bare-metal** (cero dependencias externas, cero red/Wi-Fi, cero heap obligatorio) que coloca un botón de cambio de idioma en cualquier sitio listo de tu proyecto y pre-traduce automáticamente los textos de la app al compilar.

## Qué incluye el workspace

- `crates/lang`: Crate principal `#![no_std]` (`LangButton`, `LangHost`, `LangEngine`, `LangCatalog`, `TextSlots<const N>`).
- `crates/lang-macros`: Proc-macros sin dependencias (`#[lang::auto]` / `#[lang::app]`, `lang::t!`, `lang::scan_project!`).
- `crates/lang-translator`: Motor de traducción offline `#![no_std]` (ES / EN / PT / FR / DE / IT) con diccionario de UI + traducción composicional por palabras.
- `crates/lang-build`: Helper opcional para `build.rs` (`lang_build::generate_catalog()` + `lang::include_catalog!()`).

## Uso rápido

### 1. Escribe los textos normales de tu app bajo `#[lang::app]`

No necesitas envolver cada string ni mantener archivos JSON/YAML:

```rust
#[lang::app]
mod mi_ui {
    pub fn textos() -> [&'static str; 2] {
        ["Bienvenido al sistema", "Guardar cambios"]
    }
}
```

En tiempo de compilación, `lang` extrae cada literal de texto UI, lo pre-traduce offline a los 6 idiomas y lo reemplaza por un selector `O(1)` sin heap (`&'static str`).

### 2. Coloca el botón en tu sitio listo implementando `LangHost`

```rust
use lang::ui::{ButtonMode, ButtonView, LabelStyle, LangButton, LangHost, Rect, UiEvent};
use lang::Language;

struct MiPantalla;

impl LangHost for MiPantalla {
    fn ready_slot(&self) -> Rect {
        Rect::new(240, 8, 64, 24)
    }

    fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
        // Dibuja el botón en view.bounds con la etiqueta view.label ("ES", "EN", ...)
    }

    // Opcional si tu UI guarda nodos de texto retenidos (si usas #[lang::app] basta con dejar los defaults):
    // fn text_count(&self) -> usize { ... }
    // fn source_text(&self, index: usize) -> Option<&str> { ... }
    // fn set_text(&mut self, index: usize, translated: &str) { ... }

    fn on_language_changed(&mut self, _new_lang: Language) {
        // Repinta tu UI: los textos bajo #[lang::app] ya salen en el nuevo idioma
    }
}
```

Monta el botón directamente en el `ready_slot` y reenvía eventos de clic/toque/tecla:

```rust
let mut host = MiPantalla;
let mut btn = LangButton::mount_in_slot(
    &[Language::Es, Language::En, Language::Pt, Language::Fr],
    ButtonMode::Cycle,
    LabelStyle::CodeOnly,
    &mut host,
);

btn.handle_event(UiEvent::Tap { x: 250, y: 16 }, &mut host);
```

### 3. Modo sin macros o escaneo completo de proyecto

- **Escaneo completo**: `static CATALOG: lang::LangCatalog = lang::scan_project!();` + `lang::install_catalog(&CATALOG);`
- **Sin atomics globales** (MCU sin CAS): usa `LangEngine::new(Language::Es)` + `LangButton::mount_with_engine(...)`.
- **Sin `std` ni `alloc`**: compila directamente con `--no-default-features` o `--no-default-features --features macros,atomics`.

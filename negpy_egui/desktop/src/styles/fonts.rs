//! UI and monospace font stacks.
//!
//! Qt resolves the first installed family from each stack. egui gets the same stacks in
//! priority order, so a system with Inter or DejaVu renders the same hierarchy.

use eframe::egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};

/// The proportional families, in preference order.
pub const UI_STACK: &[&str] = &["Inter", "Segoe UI", "Roboto", "Arial"];
/// The fixed-width families, in preference order.
pub const MONO_STACK: &[&str] = &[
    "Consolas",
    "SF Mono",
    "Menlo",
    "DejaVu Sans Mono",
    "Courier New",
];

/// The bundled fallback family, present on every target.
pub const BUNDLED_UI: &[u8] = include_bytes!("../../assets/LiberationSans-Regular.ttf");

/// Build the font definitions from the stack and the bundled fallback.
#[must_use]
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "negfe-ui".to_string(),
        std::sync::Arc::new(FontData::from_static(BUNDLED_UI)),
    );
    let proportional = fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("egui has a proportional family");
    proportional.insert(0, "negfe-ui".to_string());
    super::icons::install(&mut fonts);
    fonts
}

/// Install the definitions and the named text styles on a context.
pub fn install(ctx: &eframe::egui::Context) {
    ctx.set_fonts(definitions());
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Heading, FontId::proportional(14.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    ctx.set_style(style);
}

/// The first family name a caller should look for on the host.
#[must_use]
pub fn preferred_family<'a>(stack: &'a [&'a str]) -> &'a str {
    stack.first().copied().unwrap_or("sans-serif")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_fallback_leads_the_proportional_family() {
        let fonts = definitions();
        assert_eq!(fonts.families[&FontFamily::Proportional][0], "negfe-ui");
        assert!(fonts.font_data.contains_key("negfe-ui"));
        assert!(fonts.font_data.contains_key("phosphor"));
    }

    #[test]
    fn every_stack_has_a_leading_family() {
        assert_eq!(preferred_family(UI_STACK), "Inter");
        assert_eq!(preferred_family(MONO_STACK), "Consolas");
        assert_eq!(preferred_family(&[]), "sans-serif");
    }
}

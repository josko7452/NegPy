//! A tab's own strip of sub-tabs over a stack.
//!
//! Named tabs come first. Icon-only tabs follow after a divider. The strip keeps
//! the chosen tab by its key, so a restart opens the same page.

use eframe::egui::{Rect, Sense, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::theme::Theme;
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};

/// The sub-tab strip's height.
pub const SUB_TAB_HEIGHT: f32 = 38.0;

/// One sub-tab: its saved key and its look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubTab {
    pub key: &'static str,
    pub tab: PanelTab,
}

impl SubTab {
    /// A named sub-tab.
    #[must_use]
    pub fn new(key: &'static str, icon: Icon, label: &'static str) -> Self {
        Self {
            key,
            tab: PanelTab::beside(icon, label),
        }
    }

    /// An icon-only sub-tab, keeping its name for the tooltip.
    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.tab = self.tab.icon_only();
        self
    }

    /// Draw a divider before this sub-tab.
    #[must_use]
    pub fn with_divider(mut self) -> Self {
        self.tab = self.tab.with_divider();
        self
    }
}

/// The selected sub-tab's key after a click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubTabState {
    pub current: usize,
    pub keys: Vec<&'static str>,
}

impl SubTabState {
    /// Start on the first sub-tab, or the one the saved key names.
    #[must_use]
    pub fn new(tabs: &[SubTab], saved: Option<&str>) -> Self {
        let current = saved
            .and_then(|key| tabs.iter().position(|tab| tab.key == key))
            .unwrap_or(0);
        Self {
            current,
            keys: tabs.iter().map(|tab| tab.key).collect(),
        }
    }

    /// The key the strip shows.
    #[must_use]
    pub fn current_key(&self) -> &'static str {
        self.keys.get(self.current).copied().unwrap_or("")
    }

    /// Paint the strip and move to a clicked sub-tab.
    pub fn show(&mut self, ui: &mut Ui, tabs: &[SubTab]) -> Option<&'static str> {
        let labels: Vec<PanelTab> = tabs.iter().map(|tab| tab.tab.clone()).collect();
        let picked = panel_tab_strip(ui, &labels, self.current, SUB_TAB_HEIGHT);
        let picked = picked?;
        self.current = picked;
        self.keys.get(picked).copied()
    }
}

/// The page area under a sub-tab strip, with the stack's top inset.
#[must_use]
pub fn sub_tab_page<R>(
    ui: &mut Ui,
    index: usize,
    current: usize,
    body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if index != current {
        return None;
    }
    let theme = Theme::default();
    ui.add_space(f32::from(theme.space_md));
    Some(body(ui))
}

/// A top divider for the strip, on the rail's ground.
#[must_use]
pub fn sub_tab_rule(ui: &mut Ui) -> Rect {
    let theme = Theme::default();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.border_primary);
    rect
}

/// Whether a strip puts a divider before an icon-only tab.
#[must_use]
pub fn starts_icon_tabs(tabs: &[SubTab], index: usize) -> bool {
    tabs.get(index)
        .is_some_and(|tab| tab.tab.icon_only && (index == 0 || !tabs[index - 1].tab.icon_only))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs() -> Vec<SubTab> {
        vec![
            SubTab::new("geometry", Icon::Image, "Geometry"),
            SubTab::new("tone", Icon::Sliders, "Tone"),
            SubTab::new("favourites", Icon::Check, "Favorites").icon_only(),
        ]
    }

    #[test]
    fn a_saved_key_reopens_its_tab() {
        let state = SubTabState::new(&tabs(), Some("tone"));
        assert_eq!(state.current_key(), "tone");
    }

    #[test]
    fn an_unknown_key_opens_the_first_tab() {
        assert_eq!(
            SubTabState::new(&tabs(), Some("missing")).current_key(),
            "geometry"
        );
    }

    #[test]
    fn the_first_icon_only_tab_starts_the_icon_group() {
        assert!(starts_icon_tabs(&tabs(), 2));
        assert!(!starts_icon_tabs(&tabs(), 1));
    }
}

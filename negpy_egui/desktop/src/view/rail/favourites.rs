//! The Frame tab's Favorites card: sliders the user chose to keep in one place.

use eframe::egui::Ui;

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};

/// One slider the Favorites card can show, and the field it reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Favourite {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
}

/// The sliders this shell exposes to the Favorites card.
pub const FAVOURITES: &[Favourite] = &[
    Favourite {
        id: "density",
        label: "Print Density",
        category: "Exposure",
    },
    Favourite {
        id: "grade",
        label: "ISO-R Grade",
        category: "Exposure",
    },
    Favourite {
        id: "shadow_density",
        label: "Shadows Density",
        category: "Exposure",
    },
    Favourite {
        id: "highlight_density",
        label: "Highlights Density",
        category: "Exposure",
    },
    Favourite {
        id: "contrast_mask",
        label: "Contrast Mask",
        category: "Exposure",
    },
    Favourite {
        id: "preflash",
        label: "Preflash",
        category: "Exposure",
    },
    Favourite {
        id: "diffusion",
        label: "Diffusion",
        category: "Exposure",
    },
];

/// The Favorites card's state.
#[derive(Clone, Debug, PartialEq)]
pub struct FavouritesState {
    /// The stored slider ids, in the order the card shows them.
    pub chosen: Vec<String>,
    /// Whether the picker lists every slider.
    pub editing: bool,
    pub open: bool,
    pub status: String,
}

impl Default for FavouritesState {
    fn default() -> Self {
        Self {
            chosen: Vec::new(),
            editing: false,
            open: true,
            status: String::new(),
        }
    }
}

impl FavouritesState {
    /// Replace the stored list, dropping ids this shell no longer exposes.
    pub fn set_chosen(&mut self, ids: Vec<String>) {
        self.chosen = ids
            .into_iter()
            .filter(|id| FAVOURITES.iter().any(|slider| slider.id == id))
            .collect();
    }

    /// Paint the card. Returns whether any shown slider changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        let mut section = CollapsibleSection::new("Favorites")
            .with_guide()
            .with_icon(Icon::Stack);
        section.expanded = self.open;
        let mut changed = false;
        let (_, result) = section.show(ui, |ui| {
            card(ui, |ui| {
                changed = self.show_body(ui, edit);
            })
        });
        self.open = section.expanded;
        let _ = result;
        changed
    }

    fn show_body(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        let mut changed = false;
        if labeled_action(ui, "Edit Favorites…", ActionRank::Normal).clicked() {
            self.editing = !self.editing;
        }
        if self.editing {
            self.show_picker(ui);
        }
        ui.add_space(f32::from(Theme::default().space_sm));
        if self.chosen.is_empty() {
            let _ = hint(
                ui,
                "No favorites yet. Use Edit Favorites to pick the sliders you use most.",
            );
            return changed;
        }
        for id in self.chosen.clone() {
            let Some(favourite) = FAVOURITES.iter().find(|slider| slider.id == id) else {
                continue;
            };
            let mut slider = slider_for(*favourite, edit);
            if slider.show(ui, favourite.label).1 {
                store_slider(*favourite, &slider, edit);
                changed = true;
            }
        }
        changed
    }

    fn show_picker(&mut self, ui: &mut Ui) {
        let _ = section_subheader(ui, "AVAILABLE");
        let mut category = "";
        for favourite in FAVOURITES {
            if favourite.category != category {
                category = favourite.category;
                let _ = section_subheader(ui, &category.to_uppercase());
            }
            let mut shown = self.chosen.iter().any(|id| id == favourite.id);
            if labeled_toggle(ui, favourite.label, &mut shown).clicked() {
                if shown {
                    self.chosen.push(favourite.id.to_string());
                } else {
                    self.chosen.retain(|id| id != favourite.id);
                }
                self.status = "Favorites saved.".to_string();
            }
        }
        if !self.status.is_empty() {
            let _ = hint(ui, &self.status);
        }
    }
}

fn slider_for(favourite: Favourite, edit: &FrameEdit) -> CompactSlider {
    let exposure = &edit.config.exposure;
    let value = match favourite.id {
        "density" => exposure.density,
        "grade" => exposure.grade,
        "shadow_density" => exposure.shadow_density,
        "highlight_density" => exposure.highlight_density,
        "contrast_mask" => exposure.contrast_mask_strength,
        "preflash" => exposure.preflash,
        "diffusion" => exposure.diffusion,
        _ => 0.0,
    };
    let spec = match favourite.id {
        "density" => SliderSpec::new(0.0, 2.0, 0.01, 2).with_default(1.08),
        "grade" => SliderSpec::new(-0.5, 5.0, 0.01, 2)
            .with_default(1.83)
            .with_unit(" R"),
        "contrast_mask" => SliderSpec::new(0.0, 1.0, 0.01, 2),
        "preflash" => SliderSpec::new(0.0, 1.0, 0.01, 2),
        "diffusion" => SliderSpec::new(0.0, 1.0, 0.01, 2),
        _ => SliderSpec::new(-1.0, 1.0, 0.01, 2),
    };
    let mut slider = CompactSlider::new(spec);
    slider.set_value(value);
    slider
}

fn store_slider(favourite: Favourite, slider: &CompactSlider, edit: &mut FrameEdit) {
    let exposure = &mut edit.config.exposure;
    match favourite.id {
        "density" => exposure.density = slider.value,
        "grade" => exposure.grade = slider.value,
        "shadow_density" => exposure.shadow_density = slider.value,
        "highlight_density" => exposure.highlight_density = slider.value,
        "contrast_mask" => exposure.contrast_mask_strength = slider.value,
        "preflash" => exposure.preflash = slider.value,
        "diffusion" => exposure.diffusion = slider.value,
        _ => return,
    }
    edit.mark_dirty();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_list_drops_ids_this_shell_cannot_show() {
        let mut state = FavouritesState::default();
        state.set_chosen(vec!["density".to_string(), "retired".to_string()]);
        assert_eq!(state.chosen, vec!["density"]);
    }

    #[test]
    fn a_mirror_reads_the_edit_value() {
        let mut edit = FrameEdit::default();
        edit.config.exposure.density = 1.42;
        let favourite = FAVOURITES[0];
        let slider = slider_for(favourite, &edit);
        assert!((slider.value - 1.42).abs() < 1e-9);
    }

    #[test]
    fn a_mirror_writes_the_edit_value() {
        let mut edit = FrameEdit::default();
        let favourite = FAVOURITES[0];
        let mut slider = slider_for(favourite, &edit);
        slider.set_value(1.6);
        store_slider(favourite, &slider, &mut edit);
        assert!((edit.config.exposure.density - 1.6).abs() < 1e-9);
        assert!(edit.is_dirty());
    }

    #[test]
    fn the_picker_paints_on_a_context() {
        let ctx = eframe::egui::Context::default();
        let mut state = FavouritesState::default();
        state.editing = true;
        let mut edit = FrameEdit::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, &mut edit);
            });
        });
    }
}

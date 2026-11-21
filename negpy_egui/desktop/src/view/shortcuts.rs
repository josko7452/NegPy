//! One registry for every key binding and the action it dispatches.

use eframe::egui::{self, Key};

use crate::app::{NegFeApp, Tab};

/// One binding: a key and the action it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub action: Action,
    pub key: Key,
    pub shift: bool,
    pub description: &'static str,
    pub category: &'static str,
}

/// A shell action a key can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    PrevFile,
    NextFile,
    RotateCcw,
    RotateCw,
    FlipH,
    FlipV,
    Undo,
    Redo,
    Find,
    ZoomFit,
    Zoom100,
    Print,
    RollAverage,
    CropAll,
    Tools,
}

/// Every binding the shell owns. A key with no entry is not a shortcut.
pub const REGISTRY: &[Binding] = &[
    Binding {
        action: Action::PrevFile,
        key: Key::ArrowLeft,
        shift: false,
        description: "Previous file",
        category: "Navigation",
    },
    Binding {
        action: Action::NextFile,
        key: Key::ArrowRight,
        shift: false,
        description: "Next file",
        category: "Navigation",
    },
    Binding {
        action: Action::RotateCcw,
        key: Key::OpenBracket,
        shift: false,
        description: "Rotate 90 CCW",
        category: "Geometry",
    },
    Binding {
        action: Action::RotateCw,
        key: Key::CloseBracket,
        shift: false,
        description: "Rotate 90 CW",
        category: "Geometry",
    },
    Binding {
        action: Action::FlipH,
        key: Key::H,
        shift: false,
        description: "Flip horizontal",
        category: "Geometry",
    },
    Binding {
        action: Action::FlipV,
        key: Key::V,
        shift: false,
        description: "Flip vertical",
        category: "Geometry",
    },
    Binding {
        action: Action::Undo,
        key: Key::Z,
        shift: false,
        description: "Undo",
        category: "Edit",
    },
    Binding {
        action: Action::Redo,
        key: Key::Z,
        shift: true,
        description: "Redo",
        category: "Edit",
    },
    Binding {
        action: Action::Find,
        key: Key::K,
        shift: false,
        description: "Find a control, card or action",
        category: "Navigation",
    },
    Binding {
        action: Action::ZoomFit,
        key: Key::F,
        shift: false,
        description: "Fit the frame",
        category: "View",
    },
    Binding {
        action: Action::Zoom100,
        key: Key::Num1,
        shift: false,
        description: "Zoom to 100%",
        category: "View",
    },
    Binding {
        action: Action::Print,
        key: Key::P,
        shift: false,
        description: "Open the Export form",
        category: "Export",
    },
    Binding {
        action: Action::RollAverage,
        key: Key::R,
        shift: false,
        description: "Toggle Roll Average",
        category: "Exposure",
    },
    Binding {
        action: Action::CropAll,
        key: Key::A,
        shift: false,
        description: "Crop all frames",
        category: "Geometry",
    },
    Binding {
        action: Action::Tools,
        key: Key::T,
        shift: false,
        description: "Open the Tools menu",
        category: "Tools",
    },
];

/// The binding that matches this key press, if any.
#[must_use]
pub fn binding_for(key: Key, shift: bool) -> Option<&'static Binding> {
    REGISTRY
        .iter()
        .find(|binding| binding.key == key && binding.shift == shift)
}

/// Run a key press, unless a text field has focus. Returns whether it ran.
pub fn dispatch(app: &mut NegFeApp, ctx: &egui::Context) -> bool {
    if text_has_focus(ctx) {
        return false;
    }
    let pressed = ctx.input(keys_pressed);
    let Some(binding) = pressed else {
        return false;
    };
    run(app, binding.action, ctx);
    true
}

fn keys_pressed(input: &egui::InputState) -> Option<Binding> {
    REGISTRY.iter().copied().find(|binding| {
        input.key_pressed(binding.key)
            && input.modifiers.shift == binding.shift
            && !input.modifiers.command
            && !input.modifiers.alt
    })
}

/// Run one action.
pub fn run(app: &mut NegFeApp, action: Action, ctx: &egui::Context) {
    match action {
        Action::PrevFile => {
            app.previous_frame();
        }
        Action::NextFile => {
            app.next_frame();
        }
        Action::RotateCcw => app.transform_action(0, ctx),
        Action::RotateCw => app.transform_action(1, ctx),
        Action::FlipH => app.transform_action(2, ctx),
        Action::FlipV => app.transform_action(3, ctx),
        Action::Undo => app.undo_action(ctx),
        Action::Redo => app.redo_action(ctx),
        Action::Find => app.focus_find(),
        Action::ZoomFit => app.set_zoom(1.0),
        Action::Zoom100 => app.zoom_to_percent(100),
        Action::Print => {
            app.rail.tab = Tab::Export;
            app.rail.export.show_subtab("file");
            app.view.tab = Tab::Export;
            app.photo_mode = false;
        }
        Action::RollAverage => app.toggle_roll_average(ctx),
        Action::CropAll => app.request_crop_all(ctx),
        Action::Tools => app.open_tools(),
    }
    ctx.request_repaint();
}

fn text_has_focus(ctx: &egui::Context) -> bool {
    ctx.memory(|memory| {
        memory
            .focused()
            .and_then(|id| egui::TextEdit::load_state(ctx, id))
            .is_some_and(|state| state.cursor.char_range().is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_has_a_unique_key_and_modifier() {
        for (index, binding) in REGISTRY.iter().enumerate() {
            for other in &REGISTRY[index + 1..] {
                assert!(
                    !(binding.key == other.key && binding.shift == other.shift),
                    "{} and {}",
                    binding.description,
                    other.description
                );
            }
        }
    }

    #[test]
    fn shift_z_is_redo_and_z_is_undo() {
        assert_eq!(binding_for(Key::Z, true).unwrap().action, Action::Redo);
        assert_eq!(binding_for(Key::Z, false).unwrap().action, Action::Undo);
    }

    #[test]
    fn every_action_is_reachable_from_a_binding() {
        for action in [
            Action::PrevFile,
            Action::NextFile,
            Action::RotateCcw,
            Action::RotateCw,
            Action::FlipH,
            Action::FlipV,
            Action::Undo,
            Action::Redo,
            Action::Find,
            Action::ZoomFit,
            Action::Zoom100,
            Action::Print,
            Action::RollAverage,
            Action::CropAll,
            Action::Tools,
        ] {
            assert!(
                REGISTRY.iter().any(|binding| binding.action == action),
                "{action:?}"
            );
        }
    }

    #[test]
    fn a_shifted_key_does_not_run_its_unshifted_action() {
        let mut input = egui::RawInput::default();
        input.modifiers.shift = true;
        input.events.push(egui::Event::Key {
            key: Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: input.modifiers,
        });
        let ctx = egui::Context::default();
        let mut hit = None;
        let _ = ctx.run(input, |ctx| {
            hit = ctx.input(keys_pressed);
        });
        assert!(hit.is_none());
    }

    #[test]
    fn an_arrow_key_reads_as_previous_and_next() {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        let ctx = egui::Context::default();
        let mut hit = None;
        let _ = ctx.run(input, |ctx| {
            hit = ctx.input(keys_pressed);
        });
        assert_eq!(hit.unwrap().action, Action::NextFile);
    }
}

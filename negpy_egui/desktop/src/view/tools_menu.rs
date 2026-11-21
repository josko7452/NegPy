//! The Tools menu: one entry per canvas tool the shell can arm.

use eframe::egui;

use crate::app::Tab;
use crate::view::rail::frame_finish::{FinishState, RetouchTool};
use crate::view::rail::frame_geometry::GeometryState;
use crate::view::rail::frame_tone::ExposureState;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arming_a_retouch_tool_leaves_the_mask_tools() {
        let mut geometry = GeometryState::default();
        let mut exposure = ExposureState::default();
        let mut finish = FinishState::default();
        let mut tab = Tab::Roll;
        let mut subtab = String::new();
        geometry.crop.draw = true;
        exposure.local.draw = true;
        arm(
            ToolAction::Heal,
            &mut geometry,
            &mut exposure,
            &mut finish,
            &mut tab,
            &mut subtab,
        );
        assert!(!geometry.crop.draw);
        assert!(!exposure.local.draw);
        assert_eq!(finish.tool, RetouchTool::Heal);
    }

    #[test]
    fn arming_a_crop_opens_the_geometry_page() {
        let mut geometry = GeometryState::default();
        let mut exposure = ExposureState::default();
        let mut finish = FinishState::default();
        let mut tab = Tab::Roll;
        let mut subtab = String::new();
        arm(
            ToolAction::Crop,
            &mut geometry,
            &mut exposure,
            &mut finish,
            &mut tab,
            &mut subtab,
        );
        assert!(geometry.crop.draw);
        assert_eq!(tab, Tab::Frame);
        assert_eq!(subtab, "geometry");
    }

    #[test]
    fn arming_a_draw_mask_clears_the_crop_and_opens_tone() {
        let mut geometry = GeometryState::default();
        let mut exposure = ExposureState::default();
        let mut finish = FinishState::default();
        let mut tab = Tab::Roll;
        let mut subtab = String::new();
        geometry.crop.draw = true;
        arm(
            ToolAction::DrawMask,
            &mut geometry,
            &mut exposure,
            &mut finish,
            &mut tab,
            &mut subtab,
        );
        assert!(!geometry.crop.draw);
        assert!(exposure.local.draw);
        assert_eq!(tab, Tab::Frame);
        assert_eq!(subtab, "tone");
    }
}

/// The tool a menu entry arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Crop,
    Region,
    DrawMask,
    OvalMask,
    CardMask,
    Heal,
    Scratch,
    Line,
    Clone,
}

/// Open the Tools menu under the pointer and return its choice.
pub fn show_from_pointer(ctx: &egui::Context) -> Option<ToolAction> {
    let at = ctx.input(|input| input.pointer.hover_pos())?;
    show_at(ctx, at)
}

fn show_at(ctx: &egui::Context, at: egui::Pos2) -> Option<ToolAction> {
    let mut picked = None;
    egui::Area::new("tools-menu".into())
        .fixed_pos(at)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                picked = show_contents(ui);
            });
        });
    picked
}

fn show_contents(ui: &mut egui::Ui) -> Option<ToolAction> {
    let mut picked = None;
    ui.set_min_width(190.0);
    for (heading, tools) in [
        ("Crop", &[("Draw Crop", ToolAction::Crop)][..]),
        ("Metering", &[("Draw Region", ToolAction::Region)][..]),
        (
            "Dodge & Burn",
            &[
                ("Draw Mask", ToolAction::DrawMask),
                ("Oval Mask", ToolAction::OvalMask),
                ("Card Edge", ToolAction::CardMask),
            ][..],
        ),
        (
            "Retouch",
            &[
                ("Heal", ToolAction::Heal),
                ("Scratch", ToolAction::Scratch),
                ("Transport Line", ToolAction::Line),
                ("Clone", ToolAction::Clone),
            ][..],
        ),
    ] {
        ui.label(egui::RichText::new(heading).strong());
        for (label, tool) in tools {
            if ui.button(*label).clicked() {
                picked = Some(*tool);
            }
        }
    }
    picked
}

/// Arm one tool and return the tab page that owns it.
pub fn arm(
    tool: ToolAction,
    geometry: &mut GeometryState,
    exposure: &mut ExposureState,
    finish: &mut FinishState,
    tab: &mut Tab,
    subtab: &mut String,
) {
    geometry.crop.draw = false;
    exposure.local.draw = false;
    exposure.local.oval = false;
    exposure.local.card = false;
    finish.tool = RetouchTool::Off;
    match tool {
        ToolAction::Crop => {
            geometry.crop.draw = true;
            *tab = Tab::Frame;
            *subtab = "geometry".to_string();
        }
        ToolAction::Region => {
            *tab = Tab::Roll;
            *subtab = String::new();
        }
        ToolAction::DrawMask => {
            exposure.local.draw = true;
            *tab = Tab::Frame;
            *subtab = "tone".to_string();
        }
        ToolAction::OvalMask => {
            exposure.local.oval = true;
            *tab = Tab::Frame;
            *subtab = "tone".to_string();
        }
        ToolAction::CardMask => {
            exposure.local.card = true;
            *tab = Tab::Frame;
            *subtab = "tone".to_string();
        }
        ToolAction::Heal => finish.tool = RetouchTool::Heal,
        ToolAction::Scratch => finish.tool = RetouchTool::Scratch,
        ToolAction::Line => finish.tool = RetouchTool::Line,
        ToolAction::Clone => finish.tool = RetouchTool::Clone,
    }
}

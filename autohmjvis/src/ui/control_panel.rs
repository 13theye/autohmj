use crate::settings::UiStateConfig;
use nannou::window::Window;
use nannou_egui::Egui;

pub struct ControlPanel {
    pub show: bool,
    pub egui: Egui,
    pub human_color: [f32; 4], // [r, g, b, a]
    pub save_status: Option<String>,
}

impl ControlPanel {
    pub fn new(window: &Window) -> Self {
        let ui_state = UiStateConfig::load();
        let c = ui_state.human_color;
        Self {
            show: false,
            egui: Egui::from_window(window),
            human_color: [c.r, c.g, c.b, c.a],
            save_status: None,
        }
    }
}

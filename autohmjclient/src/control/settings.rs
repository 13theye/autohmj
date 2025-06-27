// src/control/UISettings.rs
//
// Settings for the application as controlled by UI

pub struct Settings {
    pub send_osc_left: bool,
    pub send_osc_right: bool,
    pub send_osc_human: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            send_osc_left: true,
            send_osc_right: true,
            send_osc_human: true,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

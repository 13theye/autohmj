// src/control/controls.rs
//
// controls for the application

pub struct Controls {
    pub send_osc_left: bool,
    pub send_osc_right: bool,
    pub send_osc_human: bool,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            send_osc_left: true,
            send_osc_right: true,
            send_osc_human: true,
        }
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self::new()
    }
}

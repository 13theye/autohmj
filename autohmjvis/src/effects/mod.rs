use nannou::prelude::*;

pub mod background_fx;
pub use background_fx::{BackgroundColorFade, BackgroundFlash};

pub trait BackgroundEffect {
    fn start(&mut self, start_color: Rgb, target_color: Rgb, duration: f32, current_time: f32);
    fn update(&mut self, current_time: f32) -> Option<Rgb>;
    fn is_active(&self) -> bool;
}

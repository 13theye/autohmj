// src/views/background.rs
//
// A simple module to manage background state
// Needs improvement: pattern after backbone_fx

use crate::effects::*;
use nannou::prelude::*;

#[derive(Default)]
pub struct BackgroundManager {
    current_color: Rgb,
    flasher: BackgroundFlash,
    color_fader: BackgroundColorFade,
}

impl BackgroundManager {
    pub fn new(current_color: Rgb) -> Self {
        Self {
            current_color,
            flasher: BackgroundFlash::default(),
            color_fader: BackgroundColorFade::default(),
        }
    }

    pub fn flash(&mut self, flash_color: Rgb, duration: f32, current_time: f32) {
        if !self.flasher.is_active() {
            self.flasher
                .start(flash_color, self.current_color, duration, current_time);
        } else {
            let target_color = self.flasher.target_color;
            self.flasher
                .start(flash_color, target_color, duration, current_time);
        }
    }

    pub fn color_fade(&mut self, target_color: Rgb, duration: f32, current_time: f32) {
        self.color_fader
            .start(self.current_color, target_color, duration, current_time);
    }

    fn update_color(&mut self, current_time: f32) {
        if self.color_fader.is_active() {
            if let Some(new_color) = self.color_fader.update(current_time) {
                self.current_color = new_color;
            }
        }
        if self.flasher.is_active() {
            if let Some(new_color) = self.flasher.update(current_time) {
                self.current_color = new_color;
            }
        }
    }

    pub fn draw(&mut self, draw: &Draw, current_time: f32) {
        self.update_color(current_time);
        draw.background().color(self.current_color);
    }

    pub fn get_current_color(&self) -> Rgb {
        self.current_color
    }
}

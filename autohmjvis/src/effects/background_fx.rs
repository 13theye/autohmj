// src/effects/background_flash.rs

use crate::effects::BackgroundEffect;
use nannou::prelude::*;

// The BackgroundFlash flashes the color White and then
// linearly fades to the target color, which is typically
// the background's color just before the flash begins.

#[derive(Debug, Default)]
pub struct BackgroundFlash {
    start_color: Rgb,
    pub target_color: Rgb,
    start_time: f32,
    duration: f32,
    is_active: bool,
}

impl BackgroundFlash {
    pub fn new() -> Self {
        Self {
            start_color: rgb(0.0, 0.0, 0.0),
            target_color: rgb(0.0, 0.0, 0.0),
            start_time: 0.0,
            duration: 0.0,
            is_active: false,
        }
    }
}

impl BackgroundEffect for BackgroundFlash {
    fn start(&mut self, start_color: Rgb, target_color: Rgb, duration: f32, current_time: f32) {
        self.start_color = start_color;
        self.target_color = target_color;
        self.duration = duration;
        self.start_time = current_time;
        self.is_active = true;
    }

    fn update(&mut self, current_time: f32) -> Option<Rgb> {
        if !self.is_active {
            return None;
        }

        let elapsed = current_time - self.start_time;
        if elapsed > self.duration {
            self.is_active = false;
            return Some(self.target_color);
        }

        // Calculate alpha based on time elapsed
        let progress = elapsed / self.duration;
        let alpha = 1.0 - progress; // Linear fade out

        // Blend with black background
        Some(rgb(
            self.start_color.red * alpha,
            self.start_color.green * alpha,
            self.start_color.blue * alpha,
        ))
    }

    fn is_active(&self) -> bool {
        self.is_active
    }
}

// Linearly fades the background color to a new color.
// Uses HSL color type internally for better aesthetics.
#[derive(Debug, Default)]
pub struct BackgroundColorFade {
    start_color: Rgb,
    target_color: Rgb,
    start_time: f32,
    duration: f32,
    is_active: bool,
}

impl BackgroundColorFade {
    pub fn new() -> Self {
        Self {
            start_color: rgb(0.0, 0.0, 0.0),
            target_color: rgb(0.0, 0.0, 0.0),
            start_time: 0.0,
            duration: 0.0,
            is_active: false,
        }
    }
}

impl BackgroundEffect for BackgroundColorFade {
    fn start(&mut self, start_color: Rgb, target_color: Rgb, duration: f32, current_time: f32) {
        self.start_color = start_color;
        self.target_color = target_color;
        self.duration = duration;
        self.start_time = current_time;
        self.is_active = true;
    }

    fn update(&mut self, current_time: f32) -> Option<Rgb> {
        if !self.is_active {
            return None;
        }

        if self.duration.abs() < 0.001 {
            return Some(self.target_color);
        }

        let elapsed = current_time - self.start_time;
        if elapsed > self.duration {
            self.is_active = false;
            return Some(self.target_color);
        }

        // Calculate interpolation factor (progress between 0.0 and 1.0)
        let progress = elapsed / self.duration;

        // Convert start and target colors to HSL
        let start_hsl = Hsl::from(self.start_color);
        let target_hsl = Hsl::from(self.target_color);

        // Handle hue wraparound to ensure smooth transition
        let h1 = start_hsl.hue.to_degrees().rem_euclid(360.0);
        let h2 = target_hsl.hue.to_degrees().rem_euclid(360.0);
        let h_new = if (h2 - h1).abs() > 180.0 {
            if h1 > h2 {
                lerp(h1, h2 + 360.0, progress) % 360.0
            } else {
                lerp(h1 + 360.0, h2, progress) % 360.0
            }
        } else {
            lerp(h1, h2, progress)
        };

        // Interpolate Saturation and Lightness linearly
        let s_new = lerp(start_hsl.saturation, target_hsl.saturation, progress);
        let l_new = lerp(start_hsl.lightness, target_hsl.lightness, progress);

        let new_hsl = Hsl::new(h_new, s_new, l_new);
        // Convert back to RGB
        Some(Rgb::from(new_hsl))
    }

    fn is_active(&self) -> bool {
        self.is_active
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

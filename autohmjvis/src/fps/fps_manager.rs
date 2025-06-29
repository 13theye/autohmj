// src/fps/fps_manager.rs
//
// The FPS calculating module for apps that use Nannou

use nannou::prelude::*;
use std::time::Instant;

pub struct FpsManager {
    fps: f32,
    fps_update_interval: f32,
    last_update: Instant,
    last_fps_update: Instant,
    frame_count: usize,
    frame_time_accumulator: f32,

    counting: bool,
    should_draw: bool,

    // The place where the FPS will be drawn
    draw_position: Option<Point2>,
}

impl Default for FpsManager {
    fn default() -> Self {
        Self::new_with(false, false)
    }
}

impl FpsManager {
    pub fn new_with(counting: bool, should_draw: bool) -> Self {
        Self {
            fps: 0.0,
            fps_update_interval: 0.3,
            last_update: Instant::now(),
            last_fps_update: Instant::now(),
            frame_count: 0,
            frame_time_accumulator: 0.0,
            counting,
            should_draw,

            draw_position: None,
        }
    }

    pub fn toggle(&mut self) {
        self.counting = !self.counting;
        if self.counting {
            self.initialize();
        }
    }

    // Start counting frames
    fn initialize(&mut self) {
        self.fps = 0.0;
        self.frame_count = 0;
        self.frame_time_accumulator = 0.0;
        let now = Instant::now();
        self.last_update = now;
        self.last_fps_update = now;
    }

    // Update the FPS, without drawing
    pub fn update(&mut self) {
        if !self.counting {
            return;
        }

        self.calculate_fps();
    }

    // Update the FPS and draw if should_draw is true
    pub fn update_and_draw(&mut self, draw: &Draw) {
        // Don't update if not counting
        if !self.counting {
            return;
        }

        self.calculate_fps();
        if self.should_draw {
            self.draw(draw);
        }
    }

    // FPS math
    fn calculate_fps(&mut self) {
        let now = Instant::now();
        let dt_update = now - self.last_update;
        let elapsed_since_last_update = dt_update.as_secs_f32();

        let dt_fps = now - self.last_fps_update;
        let elapsed_since_last_fps = dt_fps.as_secs_f32();

        self.frame_count += 1;
        self.frame_time_accumulator += elapsed_since_last_update;
        self.last_update = now;

        if elapsed_since_last_fps >= self.fps_update_interval {
            if self.frame_count > 0 {
                let avg_frame_time = self.frame_time_accumulator / self.frame_count as f32;
                self.fps = if avg_frame_time > 0.0 {
                    1.0 / avg_frame_time
                } else {
                    0.0
                };
            }

            // Reset accumulators
            self.frame_count = 0;
            self.frame_time_accumulator = 0.0;
            self.last_fps_update = now;
        }
    }

    // get the current FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /***************** Drawing functionality ***************************/

    pub fn set_draw_position(&mut self, position: Point2) {
        self.draw_position = Some(position);
    }

    pub fn draw(&self, draw: &Draw) {
        // Don't draw if no position is set
        let Some(pos) = self.draw_position else {
            return;
        };

        draw.text(&format!("FPS: {:.1}", self.fps))
            .x_y(pos.x, pos.y)
            .color(RED)
            .font_size(10);
    }
}

//! src/views/cell.rs
//!
//! Cell of a grid
//!

use nannou::prelude::*;
use nannou::text::Layout;

use crate::views::{
    animation,
    {grid::TextGridParams, TextGridFonts},
};
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Debug)]
pub struct GridCellParams {
    pub origin: Vec2,
    pub size: Vec2,
}

#[derive(Copy, Clone, Debug)]
pub struct GridCellBackground {
    pub color: Rgba,
}

impl GridCellBackground {
    /// Initialize as a blank background
    pub fn init() -> Self {
        Self {
            color: Rgba::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Set background to blank
    pub fn blank(&mut self) {
        self.color = Rgba::new(0.0, 0.0, 0.0, 0.0);
    }

    /// Set background to white
    pub fn white(&mut self) {
        self.color = Rgba::new(1.0, 1.0, 1.0, 1.0);
    }
}

/// A single character to be displayed in a grid cell and used by the Sequencer.
#[derive(Copy, Clone, Debug)]
pub struct GridCellChar {
    // The complete content character
    pub c: char,
    // The character currently being displayed (i.e. Hangeul partial for typing animation)
    pub display: char,

    pub color: Rgba,
    pub is_visible: bool,
    pub is_hangeul: bool,
    pub style_animation: GridCellCharStyleAnimationState,
}

/// The animation state of a character, used for the simple scaling/color animations
/// triggered by the sequencer. Typing animations are managed by the TypingAnimationController.
#[derive(Copy, Clone, Debug)]
pub struct GridCellCharStyleAnimationState {
    pub start: Instant,
    pub duration: Duration,
}

impl GridCellCharStyleAnimationState {
    pub fn set_start(&mut self, start: Instant) {
        self.start = start;
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    pub fn is_active(&self, now: Instant) -> bool {
        now - self.start < self.duration
    }
}

impl Default for GridCellCharStyleAnimationState {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }
}

/// A single cell of the grid, responsible for drawing the character and background.
#[derive(Copy, Clone, Debug)]
pub struct GridCell {
    pub params: GridCellParams,
    pub content: Option<GridCellChar>,
    pub background: GridCellBackground,
}

impl GridCell {
    /// Initiate and derive params from place in the grid.
    /// `y_x` is a tuple of (row, column), with top-right origin
    pub fn init(row_col: (u32, u32), grid_params: &TextGridParams) -> Self {
        let size = vec2(grid_params.cell_width, grid_params.cell_height);

        let origin = vec2(
            grid_params.rect().top_right().x
                - (row_col.1 as f32 * grid_params.grid_line_stroke)
                - (row_col.1 as f32 * size.x)
                - (size.x / 2.0),
            grid_params.rect().top_right().y
                - (row_col.0 as f32 * grid_params.grid_line_stroke)
                - (row_col.0 as f32 * size.y)
                - (size.y / 2.0),
        );

        Self {
            params: GridCellParams { origin, size },
            content: None,
            background: GridCellBackground::init(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        draw: &Draw,
        grid_text_layout: &Layout,
        _translation_text_layout: &Layout,
        fonts: &TextGridFonts,
        now: Instant,
        show_debug: bool,
    ) {
        self.draw_background(draw);
        self.draw_char(draw, grid_text_layout, fonts, now);

        if show_debug {
            draw.ellipse()
                .xy(self.params.origin)
                .radius(5.0)
                .color(GREEN);
        }
    }

    pub fn draw_background(&self, draw: &Draw) {
        draw.rect()
            .xy(self.params.origin)
            .wh(self.params.size)
            .color(self.background.color);
    }

    pub fn draw_char(&self, draw: &Draw, layout: &Layout, fonts: &TextGridFonts, now: Instant) {
        let Some(character) = self.content else {
            return;
        };

        if character.is_visible {
            let font = fonts.hangeul.clone();

            let (x, y) = (self.params.origin.x, self.params.origin.y);

            // Default values if not animating
            let mut size_factor = 1.0;
            let mut color_brightness = 1.0;

            // Calculate animation effects if character is animating
            if character.style_animation.is_active(now) {
                let elapsed = now - character.style_animation.start;
                let progress = (elapsed.as_secs_f32()
                    / character.style_animation.duration.as_secs_f32())
                .min(1.0);

                // Size animation: quick attack (30%), longer decay (70%)
                // Parameters: progress, attack_ratio, exp_attack, exp_decay, amplitude
                size_factor = animation::animation_curve(
                    progress, 0.5, // Peak at _% of animation duration
                    0.3, // Quicker initial attack (exp < 1.0 = faster start)
                    0.7, // Slower tail decay (exp > 1.0 = longer tail)
                    0.5, // % size increase at peak
                );

                color_brightness = 1.0 + 0.1 * (1.0 - progress).powf(3.0);
            }

            // Apply animation factors to color
            let display_color = rgba(
                (character.color.red * color_brightness).min(1.0),
                (character.color.green * color_brightness).min(1.0),
                (character.color.blue * color_brightness).min(1.0),
                character.color.alpha,
            );

            // Use constant font size with GPU transform scaling to avoid glyph cache thrashing
            draw.x_y(x, y)
                .scale(size_factor)
                .text(&character.display.to_string())
                .font(font)
                .color(display_color)
                .font_size(layout.font_size);
        }
    }
}

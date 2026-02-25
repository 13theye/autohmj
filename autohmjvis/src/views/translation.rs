//! src/views/translation.rs
//!
//! The Translation View
//!

use crate::views::TextGridFonts;

use nannou::prelude::*;
use nannou::text::{self, Font, Layout};

pub struct TranslationView {
    // First language content
    pub content1: String,
    // Second language content
    pub content2: Option<String>,
    pub rect: Rect,
}

impl TranslationView {
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// Measures how tall `content` would be when rendered with `layout` and `font`
    /// at the width of `self.rect`, with no vertical clipping.
    fn measure_content_height(&self, content: &str, layout: &Layout, font: Font) -> f32 {
        let probe_rect = Rect::from_xy_wh(self.rect.xy(), vec2(self.rect.w(), 10_000.0));
        text::text(content)
            .font(font)
            .layout(layout)
            .build(probe_rect)
            .bounding_rect()
            .h()
    }

    /// Returns `true` if `content` would overflow the vertical bounds of `self.rect`.
    fn exceeds_y_bounds(&self, content: &str, layout: &Layout, font: Font) -> bool {
        self.measure_content_height(content, layout, font) > self.rect.h()
    }

    pub fn draw(
        &self,
        draw: &Draw,
        layout: &Layout,
        fonts: &TextGridFonts,
        rotate_sideways: bool,
        color: Rgba,
    ) {
        let content = if let Some(content2) = &self.content2 {
            format!(
                "{}\n–––––\n{}",
                self.content1.to_uppercase(),
                content2.to_uppercase()
            )
        } else {
            self.content1.to_uppercase()
        };

        // Replace spaces with linebreaks for a word-per-line layout
        let word_per_line_content = content.replace(" ", "\n");

        if rotate_sideways {
            // Draw sideways (rotated 90 degrees counterclockwise)
            let rotated_draw = draw.rotate(std::f32::consts::FRAC_PI_2);
            rotated_draw
                .text(&content)
                .xy(vec2(self.rect.xy().y, -self.rect.xy().x))
                .wh(vec2(self.rect.wh().y, self.rect.wh().x))
                .layout(layout)
                .font(fonts.latin.clone())
                .color(color);
        } else {
            // Prefer word-per-line layout; fall back to space-wrapped if it overflows.
            let display_content =
                if self.exceeds_y_bounds(&word_per_line_content, layout, fonts.latin.clone()) {
                    &content
                } else {
                    &word_per_line_content
                };

            draw.text(display_content)
                .xy(self.rect.xy())
                .wh(self.rect.wh())
                .layout(layout)
                .font(fonts.latin.clone())
                .color(color);
        }
    }
}

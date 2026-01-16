//! src/views/translation.rs
//!
//! The Translation View
//!

use crate::views::TextGridFonts;

use nannou::prelude::*;
use nannou::text::Layout;

pub struct TranslationView {
    pub content: String,
    pub rect: Rect,
}

impl TranslationView {
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    pub fn draw(&self, draw: &Draw, layout: &Layout, fonts: &TextGridFonts, rotate_sideways: bool) {
        let content = &self.content.to_uppercase();

        // Replace spaces with linebreaks for a word-per-line layout
        let word_per_line_content = content.replace(" ", "\n");

        if rotate_sideways {
            // Draw sideways (rotated 90 degrees counterclockwise)
            let rotated_draw = draw.rotate(std::f32::consts::FRAC_PI_2);
            rotated_draw
                .text(content)
                .xy(vec2(self.rect.xy().y, -self.rect.xy().x))
                .wh(vec2(self.rect.wh().y, self.rect.wh().x))
                .layout(layout)
                .font(fonts.latin.clone())
                .color(WHITE);
        } else {
            // Draw normally
            draw.text(&word_per_line_content)
                .xy(self.rect.xy())
                .wh(self.rect.wh())
                .layout(layout)
                .font(fonts.latin.clone())
                .color(WHITE);
        }
    }
}

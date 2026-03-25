//! src/views/translation.rs
//!
//! The Translation View
//!

use crate::views::TextGridFonts;

use nannou::prelude::*;
use nannou::text::{self, layout as text_layout, Font, Layout};

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

    fn build_layout_for_font_size(font_size: u32) -> Layout {
        text_layout::Builder::default()
            .font_size(font_size)
            .line_spacing(15.0)
            .wrap_by_word()
            .center_justify()
            .build()
    }

    /// Binary-searches for the largest font size in [min_size, max_size] where
    /// `content` fits within the rotated rect bounds.
    fn find_max_fitting_font_size_rotated(
        &self,
        content: &str,
        font: Font,
        max_size: u32,
        min_size: u32,
    ) -> u32 {
        let mut lo = min_size;
        let mut hi = max_size;
        let mut best = min_size;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let layout = Self::build_layout_for_font_size(mid);
            if !self.exceeds_x_bounds(content, &layout, font.clone()) {
                best = mid;
                lo = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                hi = mid - 1;
            }
        }
        best
    }

    /// Returns `true` if `content` would overflow the bounds of `self.rect` when rotated 90°.
    /// Uses `self.rect.h()` as the layout width (the rotated text's available horizontal span)
    /// and checks the resulting content height against `self.rect.w()`.
    fn exceeds_x_bounds(&self, content: &str, layout: &Layout, font: Font) -> bool {
        let probe_rect = Rect::from_xy_wh(self.rect.xy(), vec2(self.rect.h(), 10_000.0));
        let height = text::text(content)
            .font(font)
            .layout(layout)
            .build(probe_rect)
            .bounding_rect()
            .h();
        height > self.rect.w()
    }

    pub fn draw(
        &self,
        draw: &Draw,
        layout: &Layout,
        layout_small: &Layout,
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

        // Inline separator variants (no newlines around separator)
        let inline_content = if let Some(content2) = &self.content2 {
            format!("{} ///// {}", self.content1.to_uppercase(), content2.to_uppercase())
        } else {
            self.content1.to_uppercase()
        };
        let word_per_line_inline_content = inline_content.replace(" ", "\n");

        if rotate_sideways {
            let font = fonts.latin.clone();

            // Steps 1–5: try preset layouts
            let preset_result: Option<(&str, &Layout)> =
                if !self.exceeds_x_bounds(&word_per_line_content, layout, font.clone()) {
                    Some((&word_per_line_content, layout))
                } else if !self.exceeds_x_bounds(&content, layout, font.clone()) {
                    Some((&content, layout))
                } else if !self.exceeds_x_bounds(&word_per_line_inline_content, layout, font.clone()) {
                    Some((&word_per_line_inline_content, layout))
                } else if !self.exceeds_x_bounds(&inline_content, layout, font.clone()) {
                    Some((&inline_content, layout))
                } else if !self.exceeds_x_bounds(&inline_content, layout_small, font.clone()) {
                    Some((&inline_content, layout_small))
                } else {
                    None
                };

            // Step 6: binary search for max fitting font size
            #[allow(unused_assignments)]
            let mut fallback_layout: Option<Layout> = None;
            let (display_content, active_layout): (&str, &Layout) = match preset_result {
                Some((c, l)) => (c, l),
                None => {
                    let best_size = self.find_max_fitting_font_size_rotated(
                        &inline_content,
                        font.clone(),
                        layout.font_size,
                        8,
                    );
                    fallback_layout = Some(Self::build_layout_for_font_size(best_size));
                    (&inline_content, fallback_layout.as_ref().unwrap())
                }
            };

            let rotated_draw = draw.rotate(std::f32::consts::FRAC_PI_2);
            rotated_draw
                .text(display_content)
                .xy(vec2(self.rect.xy().y, -self.rect.xy().x))
                .wh(vec2(self.rect.wh().y, self.rect.wh().x))
                .layout(active_layout)
                .font(font)
                .color(color);
        } else {
            let font = fonts.latin.clone();
            let (display_content, active_layout) =
                if !self.exceeds_y_bounds(&word_per_line_content, layout, font.clone()) {
                    (&word_per_line_content, layout)
                } else if !self.exceeds_y_bounds(&content, layout, font.clone()) {
                    (&content, layout)
                } else if !self.exceeds_y_bounds(&word_per_line_inline_content, layout, font.clone()) {
                    (&word_per_line_inline_content, layout)
                } else if !self.exceeds_y_bounds(&inline_content, layout, font.clone()) {
                    (&inline_content, layout)
                } else {
                    (&inline_content, layout_small)
                };

            draw.text(display_content)
                .xy(self.rect.xy())
                .wh(self.rect.wh())
                .layout(active_layout)
                .font(font)
                .color(color);
        }
    }
}

// src/views/text.rs
//
// Text styler

use nannou::{prelude::*, text::*};

// Wraps style information for a character
pub struct CharacterEntity {
    character: char,
    position: Point2,
    color: Rgba,
    font_size: u32,
    is_visible: bool,
}

// Handles character positioning
pub struct TextGrid {
    origin_x: f32,
    origin_y: f32,
    cell_width: f32,
    cell_height: f32,
    rows: usize,
    cols: usize,
}

impl TextGrid {
    pub fn new(
        origin_x: f32,
        origin_y: f32,
        cell_width: f32,
        cell_height: f32,
        rows: usize,
        cols: usize,
    ) -> Self {
        Self {
            origin_x,
            origin_y,
            cell_width,
            cell_height,
            rows,
            cols,
        }
    }

    // Calculate the position of a character for a given grid coordinate
    pub fn place_char_at(&self, row: usize, col: usize) -> Point2 {
        pt2(
            self.origin_x + (col as f32 * self.cell_width),
            self.origin_y - (row as f32 * self.cell_height),
        )
    }
}

// Create character entities from a message string
pub fn create_character_entities(
    message: &str,
    grid: &TextGrid,
    base_color: Rgba,
    font_size: u32,
) -> Vec<CharacterEntity> {
    let mut entities = Vec::new();
    let mut row = 0;
    let mut col = 0;

    for ch in message.chars() {
        // Advance to next row on a \n
        if ch == '\n' {
            row += 1;
            col = 0;
            continue;
        }

        // Skip spaces and punctuation
        if ch.is_whitespace() || ch.is_ascii_punctuation() {
            continue;
        }

        let position = grid.place_char_at(row, col);

        entities.push(CharacterEntity {
            character: ch,
            position,
            color: base_color,
            font_size,
            is_visible: true,
        });

        col += 1;

        // Advance to next row if a column is filled
        if col >= grid.cols {
            row += 1;
            col = 0;
        }
    }

    entities
}

// Draw a single character entity
pub fn draw_character(draw: &Draw, entity: &CharacterEntity, font: &Font) {
    if entity.is_visible {
        draw.text(&entity.character.to_string())
            .font(font.clone())
            .x_y(entity.position.x, entity.position.y)
            .color(entity.color)
            .font_size(entity.font_size);
    }
}

// src/views/text.rs
//
// Text styler

use nannou::{prelude::*, text::*};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;

use crate::{
    config::GridConfig,
    events::EventBus,
    models::ConvoItem,
    services::{ConvoEvent, TranslationEvent, TranslationType},
    views::AnimationController,
};

// Wraps display state information for a character
#[derive(Debug)]
pub struct CharacterEntity {
    pub character: char,    // The complete character
    pub display_char: char, // The character currently being displayed
    position: Point2,
    color: Rgba,
    font_size: u32,
    pub is_visible: bool,
    pub is_hangeul: bool,
}

// Handles character positioning
pub struct TextGrid {
    id: String,                                        // author
    is_human: bool,                                    // is this grid for a human?
    latest: Option<(usize, ConvoItem)>,                // the latest (key, message)
    content_chars: Vec<CharacterEntity>,               // the latest message broken up into chars
    connections: Arc<RwLock<HashMap<String, String>>>, // reference to model.connections

    // animation
    animation: AnimationController,

    // event
    convo_rx: broadcast::Receiver<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>,

    // attributes
    origin_x: f32,
    message_y: f32,
    translation_y: f32,
    cell_width: f32,
    cell_height: f32,
    rows: usize,
    cols: usize,
    col_width: f32,

    // text style
    base_color: Rgba,
    translation_color: Rgba,
    font_size: u32,
}

impl TextGrid {
    pub fn new(
        id: &str,
        is_human: bool,
        grid_config: &GridConfig,
        window_dims: (f32, f32),
        column: &Rect,
        events: &EventBus,
        connections: Arc<RwLock<HashMap<String, String>>>,
    ) -> Self {
        // Calculate grid dimensions
        let font_size = grid_config.font_size_text;
        let cell_width = font_size as f32 * 2.0;
        let cell_height = cell_width;
        let rows = grid_config.rows;
        let cols = grid_config.cols;
        let col_width = cell_width * cols as f32 * 0.95;

        // Calculate positioning
        let window_top = window_dims.1 / 2.0;
        let window_bottom = window_dims.1 / -2.0;
        let message_y = window_top - grid_config.top_margin as f32;
        let translation_y = window_bottom + grid_config.bottom_margin as f32;
        let origin_x = column.x() - col_width / 2.0 + cell_width / 2.0;

        let base_color = rgba(0.71, 0.71, 1.0, 1.0);
        let translation_color = rgba(0.7, 0.7, 0.4, 1.0);

        let convo_rx = events.convo.subscribe();
        let translation_rx = events.translation.subscribe();

        // Animation
        let animation = AnimationController::new(1.0, 0.5);

        Self {
            id: id.to_owned(),
            is_human,
            latest: None,
            content_chars: Vec::new(),
            connections,

            animation,

            convo_rx,
            translation_rx,

            origin_x,
            message_y,
            translation_y,
            cell_width,
            cell_height,
            rows,
            cols,
            col_width,
            base_color,
            translation_color,
            font_size,
        }
    }

    pub fn update(
        &mut self,
        time: f32,
        draw: &Draw,
        text_layout: &Layout,
        font: &Font,
        alt_font: &Font,
        translation_type: &TranslationType,
    ) {
        self.process_events(time);
        self.update_animation(time);
        self.draw(draw, text_layout, font, alt_font, translation_type);
    }

    fn update_animation(&mut self, time: f32) {
        if let Some((key, _)) = self.latest {
            self.animation.update(time, key, &mut self.content_chars);
        }
    }

    fn process_events(&mut self, time: f32) {
        while let Ok(event) = self.convo_rx.try_recv() {
            // Update latest message if author is same as this grid's author
            if let ConvoEvent::ItemAdded(new_key, new_convo_item) = event {
                if new_convo_item.author == self.id {
                    // Update latest message
                    if let Some((key, _)) = &self.latest {
                        if new_key != *key {
                            self.content_chars =
                                self.character_entities_from(&new_convo_item.message);

                            if !self.is_human {
                                self.animation.register(
                                    new_key,
                                    &new_convo_item.message,
                                    self.content_chars.len(),
                                    time,
                                );
                            }

                            self.latest = Some((new_key, new_convo_item));
                        }

                        // This is the 1st message by this author
                    } else {
                        self.content_chars = self.character_entities_from(&new_convo_item.message);

                        if !self.is_human {
                            self.animation.register(
                                new_key,
                                &new_convo_item.message,
                                self.content_chars.len(),
                                time,
                            );
                        }

                        self.latest = Some((new_key, new_convo_item));
                    }
                }
            }
        }

        // Update the translation
        while let Ok(event) = self.translation_rx.try_recv() {
            if let TranslationEvent::ItemTranslated(key, translation) = event {
                if let Some((latest_key, latest_entry)) = &self.latest {
                    if key == *latest_key {
                        let mut new_entry = latest_entry.to_owned();
                        new_entry.translation = translation;
                        self.latest = Some((*latest_key, new_entry));
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        draw: &Draw,
        text_layout: &Layout,
        font: &Font,
        alt_font: &Font,
        translation_type: &TranslationType,
    ) {
        if self.draw_grid_then_continue(draw, font) {
            self.draw_translation(
                draw,
                text_layout,
                font,
                alt_font,
                translation_type,
                self.translation_y,
            );
        }
    }

    // Draws grid text and returns true if translation should be drawn
    fn draw_grid_then_continue(&self, draw: &Draw, font: &Font) -> bool {
        // Render convo content if live input is blank
        if !self.is_human
            || (self.connections.read().unwrap().is_empty()
                || self
                    .connections
                    .read()
                    .unwrap()
                    .values()
                    .all(|msg| msg.is_empty()))
        {
            for entity in self.content_chars.iter() {
                if entity.is_visible {
                    self::draw_character(draw, entity, font);
                }
            }

            return true;

            // Render live content; no translation
        } else if let Some(human_msg) = self.connections.read().unwrap().get(&self.id) {
            let human_msg = human_msg.trim();
            let chars = self.character_entities_from(human_msg);
            for mut entity in chars {
                entity.is_visible = true;
                self::draw_character(draw, &entity, font);
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_translation(
        &self,
        draw: &Draw,
        text_layout: &Layout,
        font: &Font,
        alt_font: &Font,
        translation_type: &TranslationType,
        translation_y: f32,
    ) {
        if let Some((_, latest_item)) = &self.latest {
            if let Some(translation) = &latest_item.translation {
                let translation_font = if translation_type == &TranslationType::ToKorean {
                    font
                } else {
                    alt_font
                };

                let translation_text = format!("({})", translation.trim());

                draw.text(&translation_text)
                    .layout(text_layout)
                    .width(self.col_width)
                    .font(translation_font.clone())
                    .x_y(
                        self.origin_x + self.cell_width * self.cols as f32 / 2.0,
                        translation_y,
                    )
                    .color(self.translation_color)
                    .font_size(20);
            }
        }
    }

    // Calculate the position of a character for a given grid coordinate
    fn place_char_at(&self, row: usize, col: usize) -> Point2 {
        pt2(
            self.origin_x + (col as f32 * self.cell_width),
            self.message_y - (row as f32 * self.cell_height),
        )
    }

    // Create character entities from a message string. Hides characters by default
    // so that animations can reveal them.
    fn character_entities_from(&self, message: &str) -> Vec<CharacterEntity> {
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

            let is_hangeul = hangeul::is_hangeul(ch as u32);
            let position = self.place_char_at(row, col);

            entities.push(CharacterEntity {
                character: ch,
                display_char: ch,
                position,
                color: self.base_color,
                font_size: self.font_size,
                is_visible: true,
                is_hangeul,
            });

            col += 1;

            // Advance to next row if a column is filled
            if col >= self.cols {
                row += 1;
                col = 0;
            }
        }

        entities
    }
}

// Draw a single character entity
pub fn draw_character(draw: &Draw, entity: &CharacterEntity, font: &Font) {
    if entity.is_visible {
        draw.text(&entity.display_char.to_string())
            .font(font.clone())
            .x_y(entity.position.x, entity.position.y)
            .color(entity.color)
            .font_size(entity.font_size);
    }
}

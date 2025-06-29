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
    config::{GridConfig, OscSendConfig, SpeedConfig},
    events::EventBus,
    models::ConvoItem,
    services::{ClockService, ConvoEvent, Sequencer, TranslationEvent, TranslationType},
    views::AnimationController,
};

// If punctuation is not on this list, it will be hidden from the grid
const ALLOWED_PUNCTUATION: &[char] = &['?', '!', ':', ';', ',', '"', '\''];

// Wraps display state information for a character
#[derive(Clone, Debug, Default)]
pub struct CharacterEntity {
    pub character: char,    // The complete character
    pub display_char: char, // The character currently being displayed (i.e. partial Hangeul)
    pub position: Point2,
    color: Rgba,
    font_size: u32,
    pub is_visible: bool,
    pub is_hangeul: bool,

    // animation
    pub animation_start: Option<f32>,
    pub animation_duration: f32,
    pub is_animating: bool,
}

// Handles character positioning
pub struct TextGrid {
    pub id: String,                                    // author
    is_human: bool,                                    // is this grid for a human?
    latest: Option<(usize, ConvoItem)>,                // the latest (key, message)
    pub content_chars: Vec<CharacterEntity>,           // the latest message broken up into chars
    connections: Arc<RwLock<HashMap<String, String>>>, // reference to model.connections

    // animation
    animation: AnimationController,

    // event
    convo_rx: broadcast::Receiver<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>,

    // Osc & clock
    pub sequencer: Sequencer,
    clock: ClockService,

    // attributes
    pub origin_x: f32,  // left of the grid
    pub message_y: f32, // top of the grid
    translation_y: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub rows: usize,
    pub cols: usize,
    col_width: f32,

    // text style
    base_color: Rgba,
    translation_color: Rgba,
    font_size: u32,
    translation_font_size: u32,
}

#[allow(clippy::too_many_arguments)]
impl TextGrid {
    pub fn new(
        id: &str,
        is_human: bool,
        grid_config: &GridConfig,
        window_dims: (f32, f32),
        column: &Rect,
        events: &EventBus,
        connections: Arc<RwLock<HashMap<String, String>>>,
        osc_config: &OscSendConfig,
        speed_config: &SpeedConfig,
    ) -> Self {
        // Calculate grid dimensions
        let font_size = grid_config.font_size_text;
        let translation_font_size = grid_config.font_size_translation;
        let cell_width = font_size as f32 * 2.0;
        let cell_height = cell_width;
        let rows = grid_config.rows;
        let cols = grid_config.cols;
        let col_width = cell_width * cols as f32;

        // Calculate positioning
        let window_top = window_dims.1;
        let message_y = window_top - grid_config.top_margin as f32;
        let translation_y =
            message_y - (rows as f32 * cell_height) - grid_config.bottom_margin as f32;
        let origin_x = column.x() - col_width / 2.0 + cell_width / 2.0;

        let base_color = rgba(0.71, 0.71, 1.0, 1.0);
        let translation_color = rgba(0.0, 0.85, 0.0, 1.0);

        let convo_rx = events.convo.subscribe();
        let translation_rx = events.translation.subscribe();

        // Animation
        let animation = AnimationController::new(1.0, 0.5);

        // Initialize OSC Sender
        let sequencer = Sequencer::new(id, osc_config);
        let clock = ClockService::new(speed_config.bpm);

        Self {
            id: id.to_owned(),
            is_human,
            latest: None,
            content_chars: Vec::new(),
            connections,

            animation,
            sequencer,
            clock,

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
            translation_font_size,
        }
    }

    // Called once per frame. Receive live input from human players, update state according to
    // events, then update animations. Send sequencer/OSC if on time. Then draw the grid.
    pub fn update(
        &mut self,
        time: f32,
        draw: &Draw,
        text_layout: &Layout,
        font: &Font,
        alt_font: &Font,
        translation_type: &TranslationType,
    ) {
        self.read_live_input();
        self.process_events(time);
        self.update_animation(time);
        if self.clock.tick() {
            self.sequencer.update(&self.content_chars);
            self.trigger_sequence_animation(time);
        }
        self.draw(draw, text_layout, font, alt_font, translation_type, time);
    }

    pub fn clear(&mut self) {
        self.content_chars = Vec::new();
        self.latest = None;
    }

    // If this word is being sent to the sequencer, trigger the animation
    fn trigger_sequence_animation(&mut self, time: f32) {
        if !self.content_chars.is_empty() {
            let entity = &mut self.content_chars[self.sequencer.current_idx];
            entity.animation_start = Some(time);
            entity.animation_duration = 0.3;
            entity.is_animating = true;
        }
    }

    // Called every update to update the animation state of the current word
    fn update_animation(&mut self, time: f32) {
        if let Some((key, _)) = self.latest {
            self.animation.update(time, key, &mut self.content_chars);
        }
    }

    // Read the live input from the human user. Compare the input string to the previous
    // state of the input string, and set up animation states accordingly.
    fn read_live_input(&mut self) {
        if let Some(human_msg) = self.connections.read().unwrap().get(&self.id) {
            // Don't do anything if there's no input and there's a previous  message displayed.
            if human_msg.is_empty() && self.latest.is_some() {
                return;
            }

            if !human_msg.is_empty() && self.latest.is_some() {
                self.latest = None;
            }

            //let human_msg = human_msg.trim(); // We're keeping whitespaces now
            let new_chars = self.character_entities_from(human_msg);

            // Don't replace if unchanged
            if new_chars.len() == self.content_chars.len()
                && new_chars
                    .iter()
                    .zip(&self.content_chars)
                    .all(|(a, b)| a.character == b.character)
            {
                return;
            }

            // APPROACH: Preserve animation states using longest common subsequence logic

            // Create new vector for updated characters
            let mut updated_chars = Vec::with_capacity(new_chars.len());

            // CASE 1: Last character changed but length stayed the same
            // This covers Korean composition (ㄱ + ㅏ → 가)
            if new_chars.len() == self.content_chars.len() {
                // Check if only the last character differs
                let prefix_matches = new_chars
                    .iter()
                    .zip(&self.content_chars)
                    .take(new_chars.len() - 1)
                    .all(|(a, b)| a.character == b.character);

                if prefix_matches {
                    // Copy all characters including their animation states
                    for (i, new_entity) in new_chars.into_iter().enumerate() {
                        let mut entity = new_entity;
                        // Copy animation state from corresponding position
                        entity.animation_start = self.content_chars[i].animation_start;
                        entity.animation_duration = self.content_chars[i].animation_duration;
                        entity.is_animating = self.content_chars[i].is_animating;
                        updated_chars.push(entity);
                    }

                    self.content_chars = updated_chars;
                    return;
                }
            }

            // CASE 2: Appending characters to the end
            if new_chars.len() > self.content_chars.len()
                && new_chars
                    .iter()
                    .take(self.content_chars.len())
                    .zip(&self.content_chars)
                    .all(|(a, b)| a.character == b.character)
            {
                // Keep all existing character states
                for (i, new_entity) in new_chars.into_iter().enumerate() {
                    if i < self.content_chars.len() {
                        // Copy existing entity with its animation state
                        let mut entity = new_entity;
                        entity.animation_start = self.content_chars[i].animation_start;
                        entity.animation_duration = self.content_chars[i].animation_duration;
                        entity.is_animating = self.content_chars[i].is_animating;
                        updated_chars.push(entity);
                    } else {
                        // Append new characters
                        updated_chars.push(new_entity);
                    }
                }
            }
            // CASE 3: Removing characters from the end
            else if new_chars.len() < self.content_chars.len()
                && new_chars
                    .iter()
                    .zip(&self.content_chars)
                    .all(|(a, b)| a.character == b.character)
            {
                // Keep animation states for remaining characters
                for (i, new_entity) in new_chars.into_iter().enumerate() {
                    let mut entity = new_entity;
                    entity.animation_start = self.content_chars[i].animation_start;
                    entity.animation_duration = self.content_chars[i].animation_duration;
                    entity.is_animating = self.content_chars[i].is_animating;
                    updated_chars.push(entity);
                }
            }
            // CASE 4: Text changed in the middle
            else {
                // Create a mapping of positions with same characters
                let mut position_map = Vec::new();

                // Find matching positions
                for (new_idx, new_char) in new_chars.iter().enumerate() {
                    for (old_idx, old_char) in self.content_chars.iter().enumerate() {
                        if new_char.character == old_char.character && old_char.is_animating {
                            position_map.push((new_idx, old_idx));
                            break; // Only map each old character once
                        }
                    }
                }

                // Apply animation states based on mapping
                for (i, mut entity) in new_chars.into_iter().enumerate() {
                    // Find if this position has a mapping
                    if let Some((_, old_idx)) =
                        position_map.iter().find(|(new_idx, _)| *new_idx == i)
                    {
                        entity.animation_start = self.content_chars[*old_idx].animation_start;
                        entity.animation_duration = self.content_chars[*old_idx].animation_duration;
                        entity.is_animating = self.content_chars[*old_idx].is_animating;
                    }
                    updated_chars.push(entity);
                }
            }

            // Update content chars with new vector
            self.content_chars = updated_chars;
        }
    }

    // This function lsitens for ItemAdded messages from the ConversationService. If the
    // author of the message is this grid's author, it reads that message and generates
    // CharacterEntities from the the message.
    //
    // If the author is an AI, we know that there is only one way to handle the content:
    // run the "typing" animation and feed the currently visible characters to the Sequencer.
    //
    // If the author is human, there are two possibilities:
    // 1. As above
    // 2. Message is currently "in progress" -- so need need to handle the input via
    // ...the Connections HashMap in Main.

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
                                    &self.content_chars,
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
                                &self.content_chars,
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
        &mut self,
        draw: &Draw,
        text_layout: &Layout,
        font: &Font,
        alt_font: &Font,
        translation_type: &TranslationType,
        time: f32,
    ) {
        if self.draw_grid_then_continue(draw, font, time) {
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
    fn draw_grid_then_continue(&mut self, draw: &Draw, font: &Font, time: f32) -> bool {
        // Render previously entered convo content if:
        // 1. This is an AI's grid
        // 2. This is a human's grid and the live input is blank
        // 3. All the humans live inputs are blank (fallback for incomplete multi-user feature)
        if !self.is_human
            || (self.connections.read().unwrap().is_empty()
                || self
                    .connections
                    .read()
                    .unwrap()
                    .values()
                    .all(|msg| msg.is_empty()))
        {
            let mut drawn_entities = 0;
            for entity in self.content_chars.iter() {
                if entity.is_visible {
                    self::draw_character(draw, entity, font, time);
                    drawn_entities += 1;
                }
            }

            // If all the characters have been drawn, return true to trigger translation drawing
            return drawn_entities >= self.content_chars.len();

        // Render live content; no translation
        } else {
            for entity in self.content_chars.iter_mut() {
                entity.is_visible = true;
                self::draw_character(draw, entity, font, time);
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

                let translation_text = format!("- {}", translation.trim());

                draw.text(&translation_text)
                    .layout(text_layout)
                    .width(self.col_width)
                    .font(translation_font.clone())
                    .x_y(
                        self.origin_x + self.cell_width * self.cols as f32 / 2.0,
                        translation_y,
                    )
                    .color(self.translation_color)
                    .font_size(self.translation_font_size);
            }
        }
    }

    // Calculate the position of a character for a given grid coordinate
    pub fn place_char_at(&self, row: usize, col: usize) -> Point2 {
        pt2(
            self.origin_x + (col as f32 * self.cell_width),
            self.message_y - (row as f32 * self.cell_height),
        )
    }

    // Create character entities from a message string. Hides characters by default
    // so that animations can reveal them.
    fn character_entities_from(&self, message: &str) -> Vec<CharacterEntity> {
        let len = message.chars().count();
        let mut entities = Vec::new();
        let mut row = 0;
        let mut col = 0;

        for (idx, ch) in message.chars().enumerate() {
            // Stop processing chars for display if rows are filled
            if row >= self.rows {
                break;
            }

            // Stop processing if we are at the end of the message and the last character is a period
            // This prevents the only character in a row from being a period
            if ch == '.' && col == 0 && idx == len - 1 {
                break;
            }

            // Advance to next row on a \n
            if ch == '\n' {
                row += 1;
                col = 0;
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
                ..Default::default()
            });

            col += 1;

            if col >= self.cols {
                row += 1;
                col = 0;
            }
        }

        entities
    }
}

// Draw a single character entity
pub fn draw_character(draw: &Draw, entity: &CharacterEntity, font: &Font, time: f32) {
    if entity.is_visible {
        // Default values if not animating
        let mut size_factor = 1.0;
        let mut color_brightness = 1.0;

        // Calculate animation effects if character is animating
        if entity.is_animating {
            if let Some(start_time) = entity.animation_start {
                let elapsed = time - start_time;
                let progress = (elapsed / entity.animation_duration).min(1.0);

                // Size animation: quick attack (30%), longer decay (70%)
                // Parameters: progress, attack_ratio, exp_attack, exp_decay, amplitude
                size_factor = animation_curve(
                    progress, 0.5, // Peak at _% of animation duration
                    0.5, // Quicker initial attack (exp < 1.0 = faster start)
                    0.5, // Slower tail decay (exp > 1.0 = longer tail)
                    0.5, // % size increase at peak
                );

                // Brightness animation: exponential fade out
                // Could use animation_curve here too, but a simple exponential decay works well
                color_brightness = 1.0 + 0.1 * (1.0 - progress).powf(3.0);
            }
        }

        // Apply animation factors to size and color
        let display_size = (entity.font_size as f32 * size_factor) as u32;

        // Brighten the color for the animation
        let base_color = entity.color;
        let display_color = rgba(
            (base_color.red * color_brightness).min(1.0),
            (base_color.green * color_brightness).min(1.0),
            (base_color.blue * color_brightness).min(1.0),
            base_color.alpha,
        );

        draw.text(&entity.display_char.to_string())
            .font(font.clone())
            .x_y(entity.position.x, entity.position.y)
            .color(display_color)
            .font_size(display_size);
    }
}

// Animation curve function for flexible pulse effects
fn animation_curve(
    progress: f32,
    attack_ratio: f32,
    exp_attack: f32,
    exp_decay: f32,
    amplitude: f32,
) -> f32 {
    let peak_point = attack_ratio;

    let pulse_value = if progress < peak_point {
        // Attack phase - rise to peak
        let attack_progress = progress / peak_point;
        attack_progress.powf(exp_attack) // Lower values = faster initial attack
    } else {
        // Decay phase - fall from peak
        let decay_progress = (progress - peak_point) / (1.0 - peak_point);
        (1.0 - decay_progress).powf(exp_decay) // Higher values = longer tail
    };

    // Scale and offset (base = 1.0, add amplitude * pulse_value)
    1.0 + amplitude * pulse_value
}

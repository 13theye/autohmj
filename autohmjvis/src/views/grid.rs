//! src/views/grid.rs
//!
//! The Grid view of the Content

use nannou::prelude::*;
use nannou::text::{Font, Layout};

use prat::clockservice::{BeatEvent, BeatSubdivision, ClockService};
use tokio::sync::broadcast;

use crate::{
    content::{ContentEvent, KeyedConvoItem},
    events::HMJEventBus,
    services::Sequencer,
    settings::{GridConfig, OscSendConfig},
    views::{
        grid_cell::GridCellCharStyleAnimationState, GridCell, GridCellChar,
        TypingAnimationController,
    },
};

use std::time::{Duration, Instant};

// If punctuation is not on this list, it will be hidden from the grid
#[allow(dead_code)]
const ALLOWED_PUNCTUATION: &[char] = &['?', '!', ':', ';', ',', '"', '\''];

const CHARACTER_ANIMATION_DURATION: Duration = Duration::from_millis(300);

pub struct TextGridParams {
    pub origin: Vec2,
    pub rows: usize,
    pub cols: usize,
    pub top_margin: f32,
    pub bottom_margin: f32,
    pub left_margin: f32,
    pub right_margin: f32,
    pub grid_spacing: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub grid_line_stroke: f32,
    pub grid_text_layout: Layout,
    pub translation_text_layout: Layout,
}

impl TextGridParams {
    pub fn from_grid_config(grid_config: &GridConfig, origin: Vec2) -> Self {
        let text_layout_builder = nannou::text::layout::Builder::default();
        let grid_text_layout = text_layout_builder
            .line_spacing(15.0)
            .font_size(grid_config.font_size_text)
            .wrap_by_word()
            .center_justify()
            .build();

        let text_layout_builder = nannou::text::layout::Builder::default();
        let translation_text_layout = text_layout_builder
            .line_spacing(15.0)
            .font_size(grid_config.font_size_translation)
            .wrap_by_word()
            .center_justify()
            .build();

        Self {
            origin,
            rows: grid_config.grid_rows as usize,
            cols: grid_config.grid_cols as usize,
            top_margin: grid_config.top_margin as f32,
            bottom_margin: grid_config.bottom_margin as f32,
            left_margin: grid_config.left_margin as f32,
            right_margin: grid_config.right_margin as f32,
            grid_spacing: grid_config.grid_spacing as f32,
            cell_width: grid_config.cell_width as f32,
            cell_height: grid_config.cell_height as f32,
            grid_line_stroke: grid_config.grid_line_stroke as f32,
            grid_text_layout,
            translation_text_layout,
        }
    }

    pub fn width(&self) -> f32 {
        self.cell_width * self.cols as f32 + self.grid_line_stroke * (self.cols as f32 - 1.0)
    }

    pub fn height(&self) -> f32 {
        self.cell_height * self.rows as f32 + self.grid_line_stroke * (self.rows as f32)
    }

    pub fn rect(&self) -> Rect {
        Rect::from_xy_wh(self.origin, vec2(self.width(), self.height()))
    }
}

pub struct TextGridStyle {
    pub base_color: Rgba,
    pub translation_color: Rgba,
    pub font_size: u32,
    pub translation_font_size: u32,
    pub cell_bgcolor: Rgba,
}

impl TextGridStyle {
    pub fn init() -> Self {
        Self {
            base_color: Rgba::new(0.0, 0.0, 0.0, 1.0),
            translation_color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            font_size: 60,
            translation_font_size: 40,
            cell_bgcolor: Rgba::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

pub enum TextGridPosition {
    Left,
    Center,
    Right,
}

impl TextGridPosition {
    /// Returns the origin of the grid
    pub fn origin(&self, grid_config: &GridConfig) -> Vec2 {
        let margin = grid_config.grid_spacing as f32 + 2.0 * grid_config.grid_line_stroke as f32;
        let cells_width = grid_config.cell_width as f32 * grid_config.grid_cols as f32
            + grid_config.grid_line_stroke as f32 * (grid_config.grid_cols as f32 - 1.0);

        match self {
            TextGridPosition::Center => vec2(0.0, 0.0),
            TextGridPosition::Left => vec2(0.0 - cells_width - margin, 0.0),
            TextGridPosition::Right => vec2(0.0 + cells_width + margin, 0.0),
        }
    }
}

// Handles character positioning
pub struct TextGrid {
    pub id: String, // author
    pub is_human: bool,
    pub origin: Vec2,
    pub position: TextGridPosition,

    // Params
    pub params: TextGridParams,    // grid parameters
    pub text_style: TextGridStyle, // Text styling

    // Current content of the Grid
    pub latest_convo_item: Option<KeyedConvoItem>,
    pub content_chars: Vec<GridCellChar>,
    pub cells: Vec<GridCell>,

    // animation
    typing_animation: TypingAnimationController,

    // Sequencer
    pub sequencer: Sequencer,

    // events
    content_event_rx: broadcast::Receiver<ContentEvent>,
    beat_rx: broadcast::Receiver<BeatEvent>,
}

impl TextGrid {
    pub fn new(
        id: &str,
        is_human: bool,
        position: TextGridPosition,
        grid_config: &GridConfig,
        osc_config: &OscSendConfig,
        clock: &ClockService,
        event_bus: &HMJEventBus,
    ) -> Self {
        // Init TextGridParams
        let origin = position.origin(grid_config);

        let params = TextGridParams::from_grid_config(grid_config, origin);
        let text_style = TextGridStyle::init();

        // Init GridCells
        let cells = (0..grid_config.grid_cols)
            .flat_map(|c| (0..grid_config.grid_rows).map(move |r| (r, c)))
            .map(|(r, c)| GridCell::init((r, c), &params))
            .collect::<Vec<GridCell>>();

        // Animation
        let animation = TypingAnimationController::new(1.0, 0.5);

        // Subscribe to events
        let content_event_rx = event_bus.content.subscribe();
        let beat_rx = clock
            .subscribe_to_beats()
            .expect("TextGrid::init() -Failed to subscribe to beats");

        // Sequencer
        let sequencer = Sequencer::new(id, osc_config);

        Self {
            id: id.to_owned(),
            is_human,
            origin,
            position,
            params,
            text_style,
            latest_convo_item: None,
            content_chars: Vec::new(),
            cells,
            typing_animation: animation,
            sequencer,
            content_event_rx,
            beat_rx,
        }
    }

    /// Run once every app cycle.
    /// - Receive events from the ContentManager update the grid state.
    /// - Receive BeatEvents and advance the Sequencer
    /// - Update typing animation states
    /// - Prep GridCells for drawing.
    pub fn update(&mut self, now: Instant) {
        self.process_events(now);
        self.update_typing_animation(now);
        self.fill_cells();
    }

    fn process_events(&mut self, now: Instant) {
        while let Ok(event) = self.content_event_rx.try_recv() {
            match event {
                ContentEvent::UpdatedLatest(id, latest) if id == self.id => {
                    self.latest_convo_item = latest;

                    self.update_content_chars();

                    // Register typing animation if AI grid
                    if !self.is_human {
                        if let Some(latest) = &self.latest_convo_item {
                            self.register_animation(latest.0, now);
                        }
                    }
                }
                ContentEvent::UpdatedLiveInput(id, message) if id == self.id => {
                    self.update_with_live_message(&message, now);
                }
                ContentEvent::UpdatedTranslation(id, latest) if id == self.id => {
                    self.latest_convo_item = latest;
                    self.update_content_chars();
                }
                _ => {} // Ignore events for other grids
            }
        }

        while let Ok(beat_event) = self.beat_rx.try_recv() {
            if beat_event.subdivisions.contains(&BeatSubdivision::Quarter) {
                self.sequencer.update(&self.content_chars);
                self.trigger_sequencer_animation(now);
            }
        }
    }

    /// Update the content of the grid
    fn update_content_chars(&mut self) {
        if self.latest_convo_item.is_none() {
            self.content_chars = Vec::with_capacity(0);
            return;
        }

        let Some(latest) = &self.latest_convo_item else {
            return;
        };

        self.content_chars = grid_cell_chars_from(&latest.1.message, self.text_style.base_color);
    }

    /// Register a new animation
    fn register_animation(&mut self, key: usize, now: Instant) {
        self.typing_animation
            .register(key, &self.content_chars, now);
    }

    // If this word is being sent to the sequencer, trigger the animation
    fn trigger_sequencer_animation(&mut self, now: Instant) {
        if self.content_chars.is_empty() {
            return;
        }

        // Get indices of visible characters (matching sequencer's filtering logic)
        let visible_indices: Vec<usize> = self
            .content_chars
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_visible)
            .map(|(i, _)| i)
            .collect();

        if visible_indices.is_empty() {
            return;
        }

        // Ensure current_idx is within bounds and get the actual index in content_chars
        let idx = self.sequencer.current_idx % visible_indices.len();
        let actual_idx = visible_indices[idx];

        // Animate the correct character
        let character = &mut self.content_chars[actual_idx];
        character.style_animation.set_start(now);
        character
            .style_animation
            .set_duration(CHARACTER_ANIMATION_DURATION);
    }

    /// Update the animation for the latest message
    fn update_typing_animation(&mut self, now: Instant) {
        let Some((key, _)) = self.latest_convo_item else {
            return;
        };

        self.typing_animation
            .update(key, &mut self.content_chars, now);
    }

    /// Update the grid with a live input message
    fn update_with_live_message(&mut self, message: &str, now: Instant) {
        // Don't do anything if there's no input and a message is already in the grid
        if message.is_empty() && !self.content_chars.is_empty() {
            return;
        }

        // Uncomment this if eliminating whitespace
        // let message = message.trim();
        let new_chars = grid_cell_chars_from(message, self.text_style.base_color);

        // Don't do anything if the message is unchanged
        if self.message_length_same(&new_chars) && self.message_contents_same(&new_chars) {
            return;
        }

        // APPROACH: Preserve animation states using longest common subsequence logic

        // Create new vector for updated characters
        let mut updated_chars = Vec::with_capacity(new_chars.len());

        // CASE 1: Last character changed but length stayed the same
        // This covers Korean composition (ㄱ + ㅏ → 가)
        if self.message_only_last_char_changed(&new_chars) {
            // Copy all characters including animation states
            new_chars.into_iter().enumerate().for_each(|(i, new_char)| {
                // Copy all GridCellChars including their animation states
                let mut c = new_char;
                c.style_animation = self.content_chars[i].style_animation;
                updated_chars.push(c);
            });

        // CASE 2: Appending characters to the end
        } else if self.message_added_chars(&new_chars) {
            // Copy all characters including animation states
            new_chars.into_iter().enumerate().for_each(|(i, new_char)| {
                if i < self.content_chars.len() {
                    // Copy all existing GridCellChars including their animation states
                    let mut c = new_char;
                    c.style_animation = self.content_chars[i].style_animation;
                    updated_chars.push(c);
                } else {
                    updated_chars.push(new_char);
                }
            });
        // CASE 3: Removing characters from the end
        } else if self.message_removed_chars(&new_chars) {
            // Keep animation states for the remaining GridCellChars
            new_chars.into_iter().enumerate().for_each(|(i, new_char)| {
                let mut c = new_char;
                c.style_animation = self.content_chars[i].style_animation;
                updated_chars.push(c);
            });
        // CASE 4: Text changed in the middle
        } else {
            // Create a mapping of positions with the same characters
            let mut position_map = Vec::new();

            // Find matching positions
            new_chars
                .iter()
                .enumerate()
                .for_each(|(new_idx, new_char)| {
                    self.content_chars
                        .iter()
                        .enumerate()
                        .for_each(|(old_idx, old_char)| {
                            if new_char.c == old_char.c && old_char.style_animation.is_active(now) {
                                position_map.push((new_idx, old_idx));
                            }
                        })
                });

            // Apply animation states based on mapping
            new_chars.into_iter().enumerate().for_each(|(i, mut c)| {
                // Find if this position has a mapping
                if let Some((_, old_idx)) = position_map.iter().find(|(new_idx, _)| *new_idx == i) {
                    c.style_animation = self.content_chars[*old_idx].style_animation;
                }
                updated_chars.push(c);
            })
        }

        // Update content chars
        self.content_chars = updated_chars;
    }

    fn message_length_same(&self, new_chars: &[GridCellChar]) -> bool {
        new_chars.len() == self.filled_cells_count()
    }

    fn message_contents_same(&self, new_chars: &[GridCellChar]) -> bool {
        new_chars
            .iter()
            .zip(&self.content_chars)
            .all(|(a, b)| a.c == b.c)
    }

    /// Returns `true` if the new chars are the different from the old chars
    pub fn message_changed(&self, new_chars: &[GridCellChar]) -> bool {
        !self.message_length_same(new_chars) || !self.message_contents_same(new_chars)
    }

    /// Length stayed the same, but last character changed
    /// This covers Korean composition (ㄱ + ㅏ → 가)
    fn message_only_last_char_changed(&self, new_chars: &[GridCellChar]) -> bool {
        self.message_length_same(new_chars)
            && !self.message_contents_same(new_chars)
            && new_chars
                .iter()
                .zip(&self.content_chars)
                .take(new_chars.len() - 1)
                .all(|(a, b)| a.c == b.c)
    }

    /// New message appended characters to the end
    fn message_added_chars(&self, new_chars: &[GridCellChar]) -> bool {
        new_chars.len() > self.content_chars.len()
            && new_chars
                .iter()
                .take(self.content_chars.len())
                .zip(&self.content_chars)
                .all(|(a, b)| a.c == b.c)
    }

    /// New message only removed characters from the end
    fn message_removed_chars(&self, new_chars: &[GridCellChar]) -> bool {
        let old = &self.content_chars;

        new_chars.len() < old.len()
            && old[..new_chars.len()]
                .iter()
                .zip(new_chars)
                .all(|(a, b)| a.c == b.c)
    }

    fn filled_cells_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.content.is_some())
            .count()
    }

    fn filled_cols_count(&self) -> usize {
        self.filled_cells_count().div_ceil(self.params.rows)
    }

    /// Returns a rect defining the unfilled columns of the grid, if any.
    fn unfilled_cols_rect(&self) -> Option<Rect> {
        let unfilled_cols = self.params.cols.saturating_sub(self.filled_cols_count());

        if unfilled_cols == 0 {
            return None;
        }

        let left_edge_x = self.params.origin.x - self.params.width() / 2.0;

        // Width of unfilled columns (with grid lines between them, but not after the last)
        let unfilled_width = self.params.cell_width * unfilled_cols as f32
            + self.params.grid_line_stroke * (unfilled_cols as f32 - 1.0);

        // Unfilled columns start at the left edge (since filling goes right-to-left)
        let unfilled_start_x = left_edge_x;

        // Center of unfilled rect
        let center_x = unfilled_start_x + unfilled_width / 2.0;
        let center_y = self.params.origin.y;

        Some(Rect::from_xy_wh(
            vec2(center_x, center_y),
            vec2(unfilled_width, self.params.height()),
        ))
    }

    /// Clears the content_chars and the latest_convo_item
    pub fn clear(&mut self) {
        self.content_chars = Vec::with_capacity(0);
        self.latest_convo_item = None;
    }

    /// Propagate the content_chars to the cells
    pub fn fill_cells(&mut self) {
        self.cells
            .iter_mut()
            .enumerate()
            .for_each(|(i, cell)| match self.content_chars.get(i) {
                Some(character) => {
                    if character.is_visible {
                        cell.background.white()
                    } else {
                        cell.background.blank()
                    }
                    cell.content = Some(*character);
                }

                None => {
                    cell.background.blank();
                    cell.content = None;
                }
            });
    }

    pub fn draw(&self, draw: &Draw, font: &Font, alt_font: &Font, now: Instant, show_debug: bool) {
        self.cells.iter().for_each(|cell| {
            cell.draw(
                draw,
                &self.params.grid_text_layout,
                &self.params.translation_text_layout,
                font.clone(),
                alt_font.clone(),
                now,
                show_debug,
            )
        });

        if show_debug {
            draw.ellipse().xy(self.params.origin).radius(5.0).color(RED);
            draw.rect()
                .xy(self.params.origin)
                .wh(self.params.rect().wh())
                .no_fill()
                .stroke(RED)
                .stroke_weight(5.0);

            let unfilled_rect = self.unfilled_cols_rect();
            if let Some(rect) = unfilled_rect {
                draw.rect()
                    .xy(rect.xy())
                    .wh(rect.wh())
                    .no_fill()
                    .stroke(BLUE)
                    .stroke_weight(5.0);
            }
        }
    }
}

// Create character entities from a message string. Hides characters by default
// so that animations can reveal them.
fn grid_cell_chars_from(message: &str, color: Rgba) -> Vec<GridCellChar> {
    let mut grid_cell_chars = Vec::new();

    for ch in message.chars() {
        let is_hangeul = hangeul::is_hangeul(ch as u32);

        grid_cell_chars.push(GridCellChar {
            c: ch,
            display: ch,
            color,
            is_visible: true,
            is_hangeul,
            style_animation: GridCellCharStyleAnimationState::default(),
        });
    }

    grid_cell_chars
}

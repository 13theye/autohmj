//! src/models/model.rs
//!
//! The main Nannou model for the AutoHMJVis project.

use crate::{
    content::ContentManager,
    events::AIEvent,
    fps::FpsManager,
    intro::IntroImage,
    models::LiveInputRegistry,
    server::HMJServer,
    services::{AIService, ConversationService, TranslationService},
    ui::control_panel::ControlPanel,
    views::{BackgroundManager, TextGrid, TextGridFonts},
};
use nnpipe::*;
use prat::ClockService;

use nannou::{prelude::*, wgpu::TextureReshaper};
use std::{cell::RefCell, collections::HashMap, time::Instant};
use tokio::sync::broadcast;

pub struct Model {
    pub background: BackgroundManager, // handles background color and potential for transitions
    pub intro_image: IntroImage,       // the intro image

    // Services
    pub convo: ConversationService,    // handles the conversation
    pub translate: TranslationService, // handles translation for both language slots
    pub ai: AIService,                 // handles the AI
    pub clock: ClockService,           // handles the clock

    // Tempo
    pub original_tempo: f32,

    // Conversation channel (for moderator)
    pub gemma_rx: broadcast::Receiver<AIEvent>, // the channel for AI events
    pub humans_turn: HumansTurn, // true if the moderator decides human should speak next

    // Content and views
    pub content: Vec<ContentManager>,
    pub grids: Vec<TextGrid>, // the onscreen text grids

    // WebSockets for client
    pub server: HMJServer,                        // the WebSockets server
    pub live_input_registry: LiveInputRegistry, // manages per-author watch channels for live input
    pub cursor_positions: HashMap<String, usize>, // the cursor position in the grid

    // Fonts
    pub text_grid_fonts: TextGridFonts, // fonts used by TextGrids

    // Nannou API and rendering pipeline
    pub rendering: RefCell<Nnpipe>, // the rendering pipeline

    // Draws
    pub draw: nannou::Draw,           // the main draw
    pub audience_draw: nannou::Draw,  // the draw for the audience window
    pub performer_draw: nannou::Draw, // the draw for the performer window

    // Window's texture reshaper
    pub audience_window_id: WindowId,
    pub performer_window_id: WindowId,
    pub audience_reshaper: TextureReshaper,
    pub performer_reshaper: TextureReshaper,

    // Timing
    pub update_time: Instant,

    // FPS
    pub fps: FpsManager, // handles FPS calculations and display

    // When true, displays more verbose messages in terminal
    pub show_debug: bool,

    // When true, displays FPS
    pub show_fps: bool,

    // Control panel (toggled with M key)
    pub control_panel: ControlPanel,

    // Wrapped in Option so Drop can move it to a plain OS thread before dropping.
    // Nannou runs its event loop inside block_on(), so the main thread is always
    // in an async context — dropping a Runtime there panics.
    pub runtime: Option<tokio::runtime::Runtime>,
}

// ************************ Graceful Shutdown  *************************************

impl Drop for Model {
    fn drop(&mut self) {
        println!("\nShutting down AutoHMJVis...");
        // Move the runtime onto a plain OS thread before dropping it.
        // Nannou runs inside block_on(), so we're in an async context here;
        // dropping a Runtime directly from an async context panics.
        if let Some(rt) = self.runtime.take() {
            std::thread::spawn(move || drop(rt));
        }
    }
}

// This enum tracks whether it's the human's turn.
// It's set to Error if the API returns a blank response.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum HumansTurn {
    True,
    False,
    Error,
}

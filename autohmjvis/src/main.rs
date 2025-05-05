// src/main.rs
//
// auto-hunminjeongak server and visualizer
//

use nannou::{prelude::*, text::*};
use nnpipe::*;
use std::{collections::HashMap, fs, time::Instant};

use autohmjvis::{
    config::{AuthConfig, Config, GemmaConfig},
    models::{HMJMessage, HistoryManager},
    server::HMJServer,
    services::{GemmaManager, TranslationType},
    views::{text, BackgroundManager, TextGrid},
};

const HUMAN_ID: &str = "Human";

struct Model {
    background: BackgroundManager,
    text_layout: Layout,

    // History and Translation
    history: HistoryManager,

    // AI
    ai: GemmaManager,

    korean_font: Font,
    latin_font: Font,

    // WebSockets for client
    server: HMJServer,
    connections: HashMap<String, String>,

    // Nannou API
    draw: nannou::Draw,
    draw_renderer: nannou::draw::Renderer,

    texture_main: wgpu::Texture,
    texture_reshaper_main: wgpu::TextureReshaper,
    post_processing: Nnpipe,

    // FPS
    fps: Fps,

    // When on, displays more verbose messages in terminal
    verbose: bool,
}

fn model(app: &App) -> Model {
    // Load config
    let config = Config::load().expect("\nAuto훈민정음: FAILED TO LOAD CONFIG.TOML\n");
    let auth_config =
        AuthConfig::load(&config.paths.auth).expect("\nAuto훈민정음: FAILED TO LOAD KEY.TOML\n");
    let gemma_config = GemmaConfig::load(&config.paths.gemma)
        .expect("\nAuto훈민정음: FAILED TO LOAD GEMMA.TOML\n");

    // Initialize HMJServer
    let mut server = HMJServer::new(config.server.port);
    server.start().expect("Failed to start HMJServer");

    // Set up history manager
    let history = HistoryManager::default();

    // Set up Gemma
    let ai = GemmaManager::new(&gemma_config, &auth_config.google.api_key);

    // --- Load Font for Nannou Draw (Hangul) ---
    // Assumes "assets/gulim.ttf" exists relative to the executable
    // or relative to the project root if running with `cargo run`
    let assets = app.assets_path().expect("Could not find assets directory");
    let font_path = assets.join("gulim.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let korean_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    // --- Load Font for Nannou Draw (Latin) ---
    // Assumes "assets/avernir4.ttf" exists relative to the executable
    // or relative to the project root if running with `cargo run`

    let font_path = assets.join("avenir4.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let latin_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    // Create main output window
    let main_window_id = app
        .new_window()
        .title("Auto-훈민정음 0.1.0")
        .size(config.main_window.width, config.main_window.height)
        .msaa_samples(1)
        .view(view)
        .key_pressed(key_pressed)
        .build()
        .unwrap();

    let main_window = app.window(main_window_id).unwrap();

    // Set up render texture
    let device = main_window.device();
    let draw = nannou::Draw::new();
    let texture_main = wgpu::TextureBuilder::new()
        .size([
            config.rendering_main.texture_width,
            config.rendering_main.texture_height,
        ])
        // Our texture will be used as the RENDER_ATTACHMENT for our `Draw` render pass.
        // It will also be SAMPLED by the `TextureCapturer` and `TextureResizer`.
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        // Use nannou's default multisampling sample count.
        .sample_count(config.rendering_main.texture_samples)
        // Use a spacious 16-bit linear sRGBA format suitable for high quality drawing: Rgba16Float
        // Use 8-bit for standard quality and better perforamnce: Rgba8Unorm Rgb10a2Unorm
        .format(wgpu::TextureFormat::Rgba16Float)
        // Build
        .build(device);

    // Set up rendering pipeline
    let draw_renderer = nannou::draw::RendererBuilder::new()
        .build_from_texture_descriptor(device, texture_main.descriptor());
    let sample_count = main_window.msaa_samples();
    let post_processing = Nnpipe::new(
        device,
        config.rendering_main.texture_width,
        config.rendering_main.texture_height,
        config.rendering_main.texture_samples,
    );

    // Create the texture reshaper for on-screen display
    let texture_view_main = texture_main.view().build();
    let texture_main_sample_count = texture_main.sample_count();
    let texture_main_sample_type = texture_main.sample_type();
    let dst_format = Frame::TEXTURE_FORMAT;
    let texture_reshaper_main = wgpu::TextureReshaper::new(
        device,
        &texture_view_main,
        texture_main_sample_count,
        texture_main_sample_type,
        sample_count,
        dst_format,
    );

    // Set up Text display style
    let text_layout_builder = nannou::text::layout::Builder::default();
    let text_layout = text_layout_builder
        .line_spacing(15.0)
        .wrap_by_word()
        .left_justify()
        .build();

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        text_layout,

        history,
        ai,

        korean_font,
        latin_font,

        draw,
        draw_renderer,
        texture_main,
        texture_reshaper_main,

        server,
        connections: HashMap::new(),

        post_processing,

        fps: Fps::default(),

        verbose: false,
    }
}

fn main() {
    nannou::app(model).update(update).run();
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let now = Instant::now();
    let duration = now - model.fps.last_update;
    let dt = duration.as_secs_f32();
    model.fps.last_update = now;

    // FPS update
    if model.verbose {
        model.fps.update(app.time, dt);
        draw_debug(app, model);
    }

    // Handle the background
    model.background.draw(&model.draw, app.time);

    // Check for new client registrations and trigger history broadcast
    update_new_clients(model);

    // Receive incoming datagrams and update connections
    receive_human(model);
    receive_gemmas(model);

    // Process any new history items
    model.history.update();

    // Broadcast history to clients
    if model.history.needs_broadcast {
        model.history.needs_broadcast = false;
        model.server.broadcast(model.history.serialize());
    }

    // Update & draw graphics
    draw_conversation(app, model);

    // Send to rendering engine and post processing
    render_and_post(app, model);
}

fn view(_app: &App, model: &Model, frame: Frame) {
    //resize texture to screen
    let mut encoder = frame.command_encoder();

    model
        .texture_reshaper_main
        .encode_render_pass(frame.texture_view(), &mut encoder);
}

// ****************************** View functions ***********************************

fn draw_conversation(app: &App, model: &Model) {
    let rect = app.main_window().rect();
    let width = rect.w();
    let height = rect.h();

    // Define three columns
    let column_width = width / 4.0;
    let left_col = Rect::from_x_y_w_h(
        rect.left() + column_width / 2.0 + 100.0,
        0.0,
        column_width,
        height,
    );
    let center_col = Rect::from_x_y_w_h(0.0, 0.0, column_width, height);
    let right_col = Rect::from_x_y_w_h(
        rect.right() - column_width / 2.0 - 100.0,
        0.0,
        column_width,
        height,
    );

    // Find the most recent message from each speaker
    let latest_gemma1 = model.history.get_latest_by_author("Uri");
    let latest_human = model.history.get_latest_by_author("Human");
    let latest_gemma2 = model.history.get_latest_by_author("Ani");

    // Position for message display (upper third of each column)
    let message_y = rect.top() - 150.0;
    let translation_y = rect.bottom() + 150.0;

    // Draw the most recent message from Gemma 1
    if let Some(item) = latest_gemma1 {
        let message_text = item.message.trim();
        let translation_text = match &item.translation {
            Some(translation) => format!("({})", translation.trim()),
            None => String::new(),
        };

        draw_message(
            &model.draw,
            &model.text_layout,
            &model.korean_font,
            &model.latin_font,
            message_text.to_owned(),
            translation_text,
            &model.history.translation_type,
            left_col.x(),
            message_y,
            translation_y,
            column_width * 0.95, // width constraint
        );
    }

    // Draw the most recent message from Gemma 2
    if let Some(item) = latest_gemma2 {
        let message_text = item.message.trim();
        let translation_text = match &item.translation {
            Some(translation) => format!("({})", translation.trim()),
            None => String::new(),
        };

        draw_message(
            &model.draw,
            &model.text_layout,
            &model.korean_font,
            &model.latin_font,
            message_text.to_owned(),
            translation_text,
            &model.history.translation_type,
            right_col.x(),
            message_y,
            translation_y,
            column_width * 0.95, // width constraint
        );
    }

    // Draw human's current input OR history item if no input is in progress
    if model.connections.is_empty() || model.connections.values().all(|msg| msg.is_empty()) {
        if let Some(item) = latest_human {
            let message_text = item.message.trim();
            let translation_text = match &item.translation {
                Some(translation) => format!("({})", translation.trim()),
                None => String::new(),
            };

            draw_message(
                &model.draw,
                &model.text_layout,
                &model.korean_font,
                &model.latin_font,
                message_text.to_owned(),
                translation_text,
                &model.history.translation_type,
                center_col.x(),
                message_y,
                translation_y,
                column_width * 0.95, // width constraint
            );
        }
    } else if let Some(human_msg) = model.connections.get(HUMAN_ID) {
        let message_text = human_msg.trim();
        let translation_text = String::new();
        draw_message(
            &model.draw,
            &model.text_layout,
            &model.korean_font,
            &model.latin_font,
            message_text.to_owned(),
            translation_text,
            &model.history.translation_type,
            center_col.x(),
            message_y,
            translation_y,
            column_width * 0.95, // width constraint
        );
    }
}

#[allow(clippy::too_many_arguments)]
// Helper function to draw a message with text and translation
fn draw_message(
    draw: &Draw,
    text_layout: &Layout,
    font: &Font,
    alt_font: &Font,
    message: String,
    translation: String,
    translation_type: &TranslationType,
    x: f32,
    y: f32,
    translation_y: f32,
    width: f32,
) {
    let cell_side = 50.0;
    let rows = 8;
    let cols = 8;
    let grid_width = cell_side * cols as f32;

    // Create grid for message
    let message_grid = TextGrid::new(
        x - grid_width / 2.0 + cell_side / 2.0, // adjust x to align with left edge
        y,
        cell_side, // cell width
        cell_side, // cell height
        rows,
        cols,
    );

    // Create character entities for the message
    let message_color = rgba(0.71, 0.71, 1.0, 1.0);
    let message_entities =
        text::create_character_entities(&message, &message_grid, message_color, 25);

    // Draw each character in the message
    for entity in &message_entities {
        text::draw_character(draw, entity, font);
    }

    // Draw translation if available
    if !translation.is_empty() {
        let translation_font = if translation_type == &TranslationType::ToKorean {
            font
        } else {
            alt_font
        };

        draw.text(&translation)
            .layout(text_layout)
            .width(width)
            .font(translation_font.clone())
            .x_y(x + cell_side / 2.0, translation_y)
            .color(rgba(0.7, 0.7, 0.4, 1.0))
            .font_size(20);
    }
}

#[allow(clippy::too_many_arguments)]
// Helper function to draw a message with text and translation
fn draw_message_old(
    draw: &Draw,
    text_layout: &Layout,
    font: &Font,
    alt_font: &Font,
    message: String,
    translation: String,
    translation_type: &TranslationType,
    x: f32,
    y: f32,
    translation_y: f32,
    width: f32,
) {
    // Draw main message
    draw.text(&message)
        .layout(text_layout)
        .width(width)
        .font(font.clone())
        .x_y(x, y)
        .color(rgba(0.71, 0.71, 1.0, 1.0))
        .font_size(50);

    // Draw translation if available
    if !translation.is_empty() {
        let translation_font = if translation_type == &TranslationType::ToKorean {
            font
        } else {
            alt_font
        };

        draw.text(&translation)
            .layout(text_layout)
            .width(width)
            .font(translation_font.clone())
            .x_y(x, translation_y)
            .color(rgba(0.7, 0.7, 0.4, 1.0))
            .font_size(40);
    }
}

// ****************************** Controller functions ******************************

fn update_new_clients(model: &mut Model) {
    while let Some(client_id) = model.server.get_new_registrations() {
        // Send history to this specific client
        model
            .server
            .send_to_client(&client_id, model.history.serialize());
    }
}

fn receive_human(model: &mut Model) {
    // Receive WebSocket messages
    while let Some(HMJMessage(id, text)) = model.server.try_recv() {
        // Update connections map for live display or finalize messages
        if text.ends_with('\n') {
            // Committed message
            let entry = HistoryManager::new_item(&id, &text);

            // Send to next AI speaker
            let _ = model.ai.send(&entry, &model.history.entries);
            model.history.add(entry);
            // Clear the buffer
            model.connections.remove(&id);
        } else {
            // In-progress message
            model.connections.insert(id, text);
        }
    }
}

fn receive_gemmas(model: &mut Model) {
    // Trigger AI Manager to collect responses
    model.ai.receive_all();

    // Turn the queued responses into HistoryItem entries
    while model.ai.has_queued_responses() {
        let response = model.ai.responses_pop_front().unwrap();
        let entry = HistoryManager::new_item(&response.author, &response.message);
        model.history.add(entry);
    }
}

// *************************** Rendering and Capture *****************************

fn render_and_post(app: &App, model: &mut Model) {
    // Get the window device and queue
    let window = app.main_window();
    let device = window.device();
    let queue = window.queue();

    // Process the scene with post-processing
    let texture_view = model.texture_main.view().build();
    model.post_processing.process(
        device,
        queue,
        &texture_view,
        &mut model.draw_renderer,
        &model.draw,
    );
}

// ************************ FPS and debug display  *************************************
struct Fps {
    pub last_update: Instant,
    pub fps: f32,
    pub fps_update_interval: f32,
    pub frame_count: usize,
    pub last_fps_display_update: f32,
    pub frame_time_accumulator: f32,
}

impl Default for Fps {
    fn default() -> Self {
        Self {
            last_update: Instant::now(),
            fps: 0.0,
            fps_update_interval: 0.3,
            frame_count: 0,
            last_fps_display_update: 0.0,
            frame_time_accumulator: 0.0,
        }
    }
}

impl Fps {
    pub fn start(&mut self, time: f32) {
        self.fps = 0.0;
        self.frame_count = 0;
        self.frame_time_accumulator = 0.0;
        self.last_fps_display_update = time;
    }

    pub fn update(&mut self, time: f32, dt: f32) {
        self.frame_count += 1;
        self.frame_time_accumulator += dt;
        let elapsed_since_last_fps_update = time - self.last_fps_display_update;
        if elapsed_since_last_fps_update >= self.fps_update_interval {
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
            self.last_fps_display_update = time;
        }
    }
}

fn draw_debug(app: &App, model: &Model) {
    let draw = &model.draw;
    let rect = app.main_window().inner_size_points();

    // Draw (+,+) axes
    draw.line()
        .points(pt2(0.0, 0.0), pt2(50.0, 0.0))
        .color(RED)
        .stroke_weight(1.0);
    draw.line()
        .points(pt2(0.0, 0.0), pt2(0.0, 50.0))
        .color(BLUE)
        .stroke_weight(1.0);

    // Draw rect bounds
    model
        .draw
        .rect()
        .xy(pt2(0.0, 0.0))
        .wh(pt2(rect.0, rect.1))
        .stroke(rgba(0.5, 1.0, 0.5, 0.5)) // Green outline
        .stroke_weight(2.0)
        .no_fill();

    // Visualize FPS (Optional)
    draw.text(&format!(
        "FPS: {:.1}\nTranslation: {:?}",
        model.fps.fps, model.history.translation_type
    ))
    .x_y(rect.0 - 150.0, rect.1 - 30.0)
    .color(RED)
    .font_size(20);
}

// ************************ Main window input  *************************************

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::Key1 => {
            model.history.translation_type = TranslationType::ToKorean;
        }
        Key::Key2 => {
            model.history.translation_type = TranslationType::ToEnglish;
        }
        Key::Key3 => {
            model.history.translation_type = TranslationType::ToFrench;
        }
        Key::P => {
            model.verbose = !model.verbose;
            model.fps.start(app.time);
        }
        Key::Escape => {
            //shutdown(model);
            app.quit();
        }
        _ => {}
    }
}

// ************************ Graceful Shutdown  *************************************

impl Drop for Model {
    fn drop(&mut self) {
        // Modules shut themselves down gracefully.

        println!("\nShutting down AutoHMJVis...");
    }
}

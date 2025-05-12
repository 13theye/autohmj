// src/main.rs
//
// auto-hunminjeongak server and visualizer
//

use nannou::{prelude::*, text::*};
use nnpipe::*;
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
    time::Instant,
};

use autohmjvis::{
    config::{AuthConfig, Config, GemmaConfig, GridConfig, OscSendConfig},
    events::EventBus,
    models::HMJMessage,
    server::HMJServer,
    services::{ConversationService, GemmaService, TranslationService},
    views::{BackgroundManager, TextGrid},
};

//const HUMAN_ID: &str = "Human";

struct Model {
    background: BackgroundManager,
    text_layout: Layout,

    // Services
    convo: ConversationService,
    translate: TranslationService,
    ai: GemmaService,

    // View components
    grids: Vec<TextGrid>,

    // WebSockets for client
    server: HMJServer,
    connections: Arc<RwLock<HashMap<String, String>>>, // a shared reference to live input

    // Fonts
    korean_font: Font,
    latin_font: Font,

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
    // Load configs
    let config = Config::load().expect("\nAuto훈민정음: FAILED TO LOAD CONFIG.TOML\n");
    let auth_config =
        AuthConfig::load(&config.paths.auth).expect("\nAuto훈민정음: FAILED TO LOAD KEY.TOML\n");
    let gemma_config = GemmaConfig::load(&config.paths.gemma)
        .expect("\nAuto훈민정음: FAILED TO LOAD GEMMA.TOML\n");

    // Initialize event bus
    let events = EventBus::default();

    // Initialize HMJServer
    let mut server = HMJServer::new(config.server.port, &events);
    server.start().expect("Failed to start HMJServer");

    // Initialize connections
    let connections = Arc::new(RwLock::new(HashMap::new()));

    // Initialize services
    let convo = ConversationService::new(&events);
    let translate = TranslationService::new(&events);
    let ai = GemmaService::new(&gemma_config, &auth_config.google.api_key, &events);

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

    // Initialize three text grids
    let grids = init_three_grids(
        app,
        &config.grid,
        &gemma_config,
        &events,
        connections.clone(),
        &config.osc_send,
    );

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        text_layout,

        convo,
        translate,
        ai,

        grids,

        korean_font,
        latin_font,

        draw,
        draw_renderer,
        texture_main,
        texture_reshaper_main,

        server,
        connections,

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

    // Update services
    model.translate.update();
    model.ai.update();
    model.server.update();

    // Receive incoming datagrams and update connections
    receive_human_input(model);

    // Process any new conversation items
    model.convo.update();

    // Update & draw graphics
    update_grids(app, model);

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
fn update_grids(app: &App, model: &mut Model) {
    for grid in model.grids.iter_mut() {
        grid.update(
            app.time,
            &model.draw,
            &model.text_layout,
            &model.korean_font,
            &model.latin_font,
            &model.translate.translation_type,
        );
    }
}

fn init_three_grids(
    app: &App,
    grid_config: &GridConfig,
    gemma_config: &GemmaConfig,
    events: &EventBus,
    connections: Arc<RwLock<HashMap<String, String>>>,
    osc_config: &OscSendConfig,
) -> Vec<TextGrid> {
    // Get window size
    let rect = app.main_window().rect();
    let width = rect.w();
    let height = rect.h();

    // Define three columns
    let column_width = width / 4.0;
    let left_right_margin = grid_config.left_right_margin as f32;
    let left_col = Rect::from_x_y_w_h(
        rect.left() + column_width / 2.0 + left_right_margin,
        0.0,
        column_width,
        height,
    );
    let center_col = Rect::from_x_y_w_h(0.0, 0.0, column_width, height);
    let right_col = Rect::from_x_y_w_h(
        rect.right() - column_width / 2.0 - left_right_margin,
        0.0,
        column_width,
        height,
    );

    let human_grid = TextGrid::new(
        "Human",
        true,
        grid_config,
        (width, height),
        &center_col,
        events,
        connections.clone(),
        osc_config,
    );

    let gemma1_grid = TextGrid::new(
        &gemma_config.persona_1.id,
        false,
        grid_config,
        (width, height),
        &left_col,
        events,
        connections.clone(),
        osc_config,
    );

    let gemma2_grid = TextGrid::new(
        &gemma_config.persona_2.id,
        false,
        grid_config,
        (width, height),
        &right_col,
        events,
        connections.clone(),
        osc_config,
    );

    vec![human_grid, gemma1_grid, gemma2_grid]
}

// ****************************** Controller functions ******************************

fn receive_human_input(model: &mut Model) {
    // Receive WebSocket messages
    while let Some(HMJMessage(id, text)) = model.server.try_recv() {
        // Update connections map for live display or finalize messages
        if text.ends_with('\n') {
            // Create convo item
            let entry = ConversationService::new_item(&id, &text);

            // Send to next AI speaker
            let _ = model.ai.send(&entry, model.convo.entries());

            // Add message to conversation
            model.convo.add(entry);

            // Clear the buffer
            model.connections.write().unwrap().remove(&id);
        } else {
            // Message is in progress
            model.connections.write().unwrap().insert(id, text);
        }
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
        model.fps.fps, model.translate.translation_type
    ))
    .x_y(rect.0 / 2.0 - 100.0, rect.1 / 2.0 - 30.0)
    .color(RED)
    .font_size(10);
}

// ************************ Main window input  *************************************

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::Key1 => {
            model.translate.set_to_korean();
        }
        Key::Key2 => {
            model.translate.set_to_english();
        }
        Key::Key3 => {
            model.translate.set_to_french();
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

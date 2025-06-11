// src/main.rs
//
// auto-hunminjeongak server and visualizer
//

use nannou::{prelude::*, text::*, wgpu::TextureReshaper};
use nnpipe::*;
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
    time::Instant,
};

use autohmjvis::{
    config::{AuthConfig, Config, GemmaConfig, GridConfig, OscSendConfig, SpeedConfig},
    events::EventBus,
    fps::FpsManager,
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

    // Text Grids
    grids: Vec<TextGrid>,

    // WebSockets for client
    server: HMJServer,
    connections: Arc<RwLock<HashMap<String, String>>>, // a shared reference to live input

    // Fonts
    korean_font: Font,
    latin_font: Font,

    // Nannou API and rendering pipeline
    rendering: Nnpipe,

    // Draws
    draw: nannou::Draw,
    audience_draw: nannou::Draw,
    performer_draw: nannou::Draw,

    // Window's texture reshaper
    audience_window_id: WindowId,
    performer_window_id: WindowId,
    audience_reshaper: TextureReshaper,
    performer_reshaper: TextureReshaper,

    // FPS
    fps: FpsManager,

    // When true, displays more verbose messages in terminal
    show_debug: bool,

    // When true, displays FPS
    show_fps: bool,
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

    //let font_path = assets.join("avenir4.ttf");
    let font_path = assets.join("gulim.ttf");

    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let latin_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    // Create main output window
    let audience_window_id = app
        .new_window()
        .title("Auto-훈민정음 0.1.0")
        .size(config.audience_window.width, config.audience_window.height)
        .msaa_samples(1)
        .view(audience_view)
        .build()
        .unwrap();

    let performer_window_id = app
        .new_window()
        .title("Auto-훈민정음 0.1.0 Performer")
        .size(
            config.performer_window.width,
            config.performer_window.height,
        )
        .msaa_samples(1)
        .view(performer_view)
        .key_pressed(key_pressed)
        .build()
        .unwrap();

    let Some(audience_window) = app.window(audience_window_id) else {
        eprintln!("Audience window not found. Exiting app.");
        std::process::exit(1);
    };

    let Some(performer_window) = app.window(performer_window_id) else {
        eprintln!("Performer window not found. Exiting app.");
        std::process::exit(1);
    };
    println!(
        "Audience window scale factor: {}",
        audience_window.scale_factor()
    );

    println!(
        "Performer window scale factor: {}",
        performer_window.scale_factor()
    );
    let audience_draw = nannou::Draw::new();
    let performer_draw = nannou::Draw::new();

    // Set up render texture
    let draw = nannou::Draw::new();
    let device = audience_window.device();
    let rendering = Nnpipe::new(
        device,
        config.rendering_main.texture_width,
        config.rendering_main.texture_height,
        config.rendering_main.texture_samples,
    );
    let audience_reshaper = rendering.create_reshaper_for_post_processed(device, &audience_window);
    let performer_reshaper =
        rendering.create_reshaper_for_post_processed(device, &performer_window);

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
        &config.speed,
    );

    // Set up FPS manager
    let mut fps = FpsManager::new_with(false, false);
    let performer_rect = performer_window.rect();
    fps.set_draw_position(pt2(
        performer_rect.left() + 40.0,
        performer_rect.top() - 10.0,
    ));

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        text_layout,

        convo,
        translate,
        ai,

        grids,

        korean_font,
        latin_font,

        audience_window_id,
        performer_window_id,

        audience_reshaper,
        performer_reshaper,

        draw,
        audience_draw,
        performer_draw,

        server,
        connections,

        rendering,

        fps,

        show_fps: false,
        show_debug: false,
    }
}

fn main() {
    nannou::app(model).update(update).run();
}

fn update(app: &App, model: &mut Model, _update: Update) {
    // FPS update
    if model.show_fps {
        model.fps.update();
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

fn performer_view(app: &App, model: &Model, frame: Frame) {
    // Get the post-processed texture view
    let _post_processed_view = model.rendering.get_post_processed_view();

    // Update reshaper if needed (could be cached in Model)
    model
        .rendering
        .draw_to_frame(&model.performer_reshaper, &frame);

    // Handle FPS and origin display
    if model.show_fps {
        model.fps.draw(&model.performer_draw);
    }

    // Then draw UI over it
    let _ = model.performer_draw.to_frame(app, &frame);
}

fn audience_view(app: &App, model: &Model, frame: Frame) {
    // Get the post-processed texture view
    let _post_processed_view = model.rendering.get_post_processed_view();

    // Update reshaper if needed (could be cached in Model)
    model
        .rendering
        .draw_to_frame(&model.audience_reshaper, &frame);

    // Handle FPS and origin display
    if model.show_debug {
        draw_debug(app, model);
    }

    // Then draw UI over it
    let _ = model.audience_draw.to_frame(app, &frame);
}

// ****************************** View functions ***********************************
fn update_grids(app: &App, model: &mut Model) {
    for grid in model.grids.iter_mut() {
        grid.update(
            app.time,
            &model.draw,
            &model.text_layout,
            &model.korean_font,
            //&model.latin_font,
            &model.korean_font,
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
    speed_config: &SpeedConfig,
) -> Vec<TextGrid> {
    // Get window size
    let rect = app.main_window().rect();
    let width = rect.w();
    let height = rect.h();

    let font_size = grid_config.font_size_text;
    let cols = grid_config.cols;

    let margin = grid_config.left_right_margin as f32;
    let column_width = (font_size * 2 * cols as u32) as f32;

    // Define three columns
    let center_col = Rect::from_x_y_w_h(0.0, 0.0, column_width, height);
    let left_col = Rect::from_x_y_w_h(-(column_width + margin), 0.0, column_width, height);
    let right_col = Rect::from_x_y_w_h(column_width + margin, 0.0, column_width, height);

    let human_grid = TextGrid::new(
        "Human",
        true,
        grid_config,
        (width, height),
        &center_col,
        events,
        connections.clone(),
        osc_config,
        speed_config,
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
        speed_config,
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
        speed_config,
    );

    vec![gemma1_grid, human_grid, gemma2_grid]
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

    // Render the game to texture and post-process
    model.rendering.render_scene(device, queue, &model.draw);
    model.rendering.post_process(device, queue);
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

fn draw_debug(app: &App, model: &Model) {
    let draw = &model.audience_draw;
    let rect = app.window(model.audience_window_id).unwrap().rect();

    // Draw (+,+) axes
    draw.line()
        .points(pt2(0.0, 0.0), pt2(25.0, 0.0))
        .color(RED)
        .stroke_weight(1.0);
    draw.line()
        .points(pt2(0.0, 0.0), pt2(0.0, 25.0))
        .color(BLUE)
        .stroke_weight(1.0);

    // Draw rect bounds
    draw.rect()
        .xy(pt2(0.0, 0.0))
        .wh(pt2(rect.w(), rect.h()))
        .stroke(rgba(0.5, 1.0, 0.5, 0.5)) // Green outline
        .stroke_weight(2.0)
        .no_fill();
}

// ************************ Main window input  *************************************

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::Key0 => {
            model.grids[0].sequencer.send = !model.grids[0].sequencer.send;
            println!("Grid 0 send OSC: {}", model.grids[0].sequencer.send);
        }
        Key::Key1 => {
            model.grids[1].sequencer.send = !model.grids[1].sequencer.send;
            println!("Grid 1 send OSC: {}", model.grids[1].sequencer.send);
        }
        Key::Key2 => {
            model.grids[2].sequencer.send = !model.grids[2].sequencer.send;
            println!("Grid 2 send OSC: {}", model.grids[2].sequencer.send);
        }
        Key::Key5 => {
            model.translate.set_to_korean();
            println!("Translation set to Korean");
        }
        Key::Key6 => {
            model.translate.set_to_english();
            println!("Translation set to English");
        }
        Key::Key7 => {
            model.translate.set_to_french();
            println!("Translation set to French");
        }
        Key::P => {
            model.show_fps = !model.show_fps;
            model.fps.toggle();
        }
        Key::D => {
            model.show_debug = !model.show_debug;
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

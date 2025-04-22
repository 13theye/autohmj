// src/main.rs
//
// auto-hunminjeongak server and visualizer
//

use nannou::{prelude::*, text::*};
use nnpipe::*;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    net::TcpListener,
    sync::mpsc::{channel, Receiver, Sender},
    time::Instant,
};
use tungstenite::{accept, Message};

use autohmjcommon::{History, HistoryItem};
use autohmjvis::{config::Config, services::Translate, views::BackgroundManager};

// Number of entries in input_history
const MAX_HISTORY: usize = 10;

struct Model {
    background: BackgroundManager,
    text_layout: Layout,

    // Input history tracking
    input_history: BTreeMap<usize, HistoryItem>,
    next_history_idx: usize,

    // Translation
    translate: Translate,
    pending_translations: Vec<usize>, // Keys of items awaiting translation
    translation_runtime: tokio::runtime::Runtime,
    translation_tx: Sender<(usize, Option<String>)>,
    translation_rx: Receiver<(usize, Option<String>)>,

    main_font: Font,

    // networking
    ws_rx: Receiver<String>,
    ws_writer_rx: Receiver<Sender<String>>, // new writers from accept thread
    ws_writers: Vec<Sender<String>>,        // one per connection
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

    // Set up WebSocket
    let (ws_tx, ws_rx) = channel::<String>();
    let (writer_tx, ws_writer_rx) = channel::<Sender<String>>();
    let listen_addr = format!("0.0.0.0:{}", config.server.port);
    std::thread::spawn(move || {
        let listener = TcpListener::bind(&listen_addr).expect("Failed to bind WebSocket listener");

        // for each new TCP connection:
        for stream in listener.incoming().flatten() {
            let mut ws = accept(stream).expect("WebSocket handshake failed");

            // create a channel for this client's outbound messages
            let (out_tx, out_rx) = channel::<String>();

            // tell main thread that there's a new client that can be written to
            writer_tx.send(out_tx.clone()).unwrap();

            let in_tx = ws_tx.clone();

            // set nonblocking so read_message() returns WouldBlock
            ws.get_mut().set_nonblocking(true).unwrap();

            // Now run a simple read/write loop
            std::thread::spawn(move || {
                loop {
                    // 1. Read commits
                    match ws.read() {
                        Ok(Message::Text(utf8)) => {
                            let line = utf8.to_string();
                            let _ = in_tx.send(line);
                        }
                        Err(tungstenite::Error::Io(ref e))
                            if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => break, // connection closed or error
                        _ => {}
                    }

                    // 2. Drain outbound history messages
                    while let Ok(msg) = out_rx.try_recv() {
                        let _ = ws.send(Message::Text(msg.into()));
                    }

                    // avoid busy-spin
                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
            });
        }
    });

    // Set up translation send/receive
    let (translation_tx, translation_rx) = channel::<(usize, Option<String>)>();

    // --- Load Font for Nannou Draw ---
    // Assumes "assets/gulim.ttf" exists relative to the executable
    // or relative to the project root if running with `cargo run`
    let assets = app.assets_path().expect("Could not find assets directory");
    let font_path = assets.join("gulim.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let main_font = Font::from_bytes(font_bytes)
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

    // Create the texture reshaper.
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

    // Text display style
    let text_layout_builder = nannou::text::layout::Builder::default();
    let text_layout = text_layout_builder
        .line_spacing(25.0)
        .wrap_by_word()
        .left_justify()
        .build();

    // Set up translation runtime
    let translation_runtime =
        tokio::runtime::Runtime::new().expect("Failed to create Tokio translation runtime");

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        text_layout,

        input_history: BTreeMap::new(),
        next_history_idx: 0,

        translate: Translate::default(),
        pending_translations: Vec::new(),
        translation_runtime,
        translation_tx,
        translation_rx,

        main_font,

        draw,
        draw_renderer,
        texture_main,
        texture_reshaper_main,

        ws_rx,
        ws_writer_rx,
        ws_writers: Vec::new(),
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

    // FPS calculations
    if model.verbose {
        model.fps.update(app.time, dt);
    }

    // Handle the background
    model.background.draw(&model.draw, app.time);

    // Receive incoming datagrams and update connections
    receive(model);
    history_cleanup(model);

    // Initiate translations for any untranslated message in history
    process_translations(model);

    // Update & draw
    draw_output(app, model);

    render_and_post(app, model);
}

fn view(_app: &App, model: &Model, frame: Frame) {
    //resize texture to screen
    let mut encoder = frame.command_encoder();

    model
        .texture_reshaper_main
        .encode_render_pass(frame.texture_view(), &mut encoder);
}

fn draw_output(app: &App, model: &Model) {
    let num = model.connections.len() as f32;
    let rect = app.main_window().rect();
    for (i, (_id, text)) in model.connections.iter().enumerate() {
        let x = rect.left() + rect.w() / (num + 1.0) * (i as f32 + 1.0);
        model
            .draw
            .text(text)
            .layout(&model.text_layout)
            .width(1000.0)
            .font(model.main_font.clone())
            .x_y(x, 0.0)
            .color(rgba(0.71, 0.71, 1.0, 1.0))
            .font_size(50);
    }
    // Handle FPS and origin display
    if model.verbose {
        draw_fps(model);
    }
}

// ******************************* Rendering and Capture *****************************

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

// ************************ Networking *************************************************
fn receive(model: &mut Model) {
    // Pick up any brand-new client writers
    while let Ok(writer) = model.ws_writer_rx.try_recv() {
        println!("Registered new client. Broadcasting history.");
        broadcast_history(model, Some(&writer));
        model.ws_writers.push(writer);
    }

    let mut history_updated = false;

    // Drain everything that arrived since last frame
    while let Ok(raw) = model.ws_rx.try_recv() {
        if let Some((command, payload)) = raw.split_once('|') {
            if command == "register" {
                model.connections.insert(payload.to_string(), String::new());
                continue;
            }
        }

        if let Some((id, text)) = raw.split_once(':') {
            // Update the connections map for live display
            if !text.is_empty() {
                model.connections.insert(id.to_string(), text.to_string());
            } else {
                // Remove empty text entries
                model.connections.remove(id);
            }

            // If message ends with newline, it's a committed message
            if text.ends_with('\n') {
                let entry = HistoryItem::new(id, text);
                add_history_item(model, entry);

                history_updated = true;
            }
        }
    }

    // 2) broadcast the updated history if it was updated
    if history_updated {
        println!(
            "History updated for key {}, broadcasting history.",
            model.next_history_idx - 1
        );
        broadcast_history(model, None);
    }
}

fn history_cleanup(model: &mut Model) {
    while model.input_history.len() > MAX_HISTORY {
        if let Some(smallest_key) = model.input_history.keys().next().copied() {
            model.input_history.remove(&smallest_key);
        }
    }
}

fn add_history_item(model: &mut Model, item: HistoryItem) {
    let key = model.next_history_idx;
    model.input_history.insert(key, item);
    model.next_history_idx += 1;
    // trigger async translation of the history item's message here.
    model.pending_translations.push(key);
}

fn process_translations(model: &mut Model) {
    while !model.pending_translations.is_empty() {
        if let Some(key) = model.pending_translations.pop() {
            if let Some(item) = model.input_history.get(&key).cloned() {
                if item.translation.is_none() && !item.msg.trim().is_empty() {
                    let translate = model.translate.clone();
                    let msg = item.msg.clone();
                    let tx = model.translation_tx.clone();

                    // Spawn async task to handle translation
                    model.translation_runtime.spawn(async move {
                        let translation = translate.to_korean(&msg).await;
                        let _ = tx.send((key, translation));
                    });

                    // only start one new translation per frame
                    break;
                }
            }
        }
    }

    // check for completed translations
    while let Ok((key, translation)) = model.translation_rx.try_recv() {
        if let Some(item) = model.input_history.get_mut(&key) {
            item.translation = translation;
            println!(
                "Translation received for key {}, broadcasting history.",
                key
            );
            broadcast_history(model, None);
        }
    }
}

fn broadcast_history(model: &Model, writer: Option<&Sender<String>>) {
    let dump = serde_json::to_string(&History(model.input_history.clone())).unwrap();
    if let Some(w) = writer {
        // send to specific
        let _ = w.send(dump.clone());
    } else {
        // send to all
        for w in &model.ws_writers {
            let _ = w.send(dump.clone());
        }
    }
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

fn draw_fps(model: &Model) {
    let draw = &model.draw;
    // Draw (+,+) axes
    draw.line()
        .points(pt2(0.0, 0.0), pt2(50.0, 0.0))
        .color(RED)
        .stroke_weight(1.0);
    draw.line()
        .points(pt2(0.0, 0.0), pt2(0.0, 50.0))
        .color(BLUE)
        .stroke_weight(1.0);

    // Visualize FPS (Optional)
    draw.text(&format!("FPS: {:.1}", model.fps.fps))
        .x_y(900.0, 520.0)
        .color(RED)
        .font_size(20);
}

// ************************ Main window input  *************************************

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::P => {
            model.verbose = !model.verbose;
            model.fps.start(app.time);
        }
        Key::A => {
            // cheap way to make clippy quiet
        }
        _ => {}
    }
}

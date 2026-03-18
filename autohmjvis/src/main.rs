// src/main.rs
//
// auto-hunminjeongak server and visualizer
//

use nannou::{prelude::*, text::*};
use nnpipe::*;
use prat::ClockService;
use std::{cell::RefCell, collections::HashMap, fs};

use autohmjvis::{
    content::ContentManager,
    events::{GemmaEvent, HMJEventBus},
    fps::FpsManager,
    intro::IntroImage,
    models::{CommandMessage, HMJMessageWrapper, HumansTurn, LiveInputRegistry, Model},
    server::HMJServer,
    services::{ConversationService, GemmaPersona, GemmaService, TranslationService},
    settings::{
        AuthConfig, GemmaConfig, GridConfig, HumanColorConfig, OscSendConfig, Settings,
        UiStateConfig, UiStateFileConfig,
    },
    ui::control_panel::ControlPanel,
    views::{BackgroundManager, TextGrid, TextGridFonts, TextGridPosition},
};
use nannou_egui::egui;

//const HUMAN_ID: &str = "Human";

fn model(app: &App) -> Model {
    // Load configs
    // general config from the CONFIG.TOML file
    let config = Settings::load().expect("\nAuto훈민정음: FAILED TO LOAD CONFIG.TOML\n");
    // Gemma API key from the /auth/key.toml file
    let auth_config = AuthConfig::load(&config.paths.auth)
        .unwrap_or_else(|e| panic!("\nAuto훈민정음: FAILED TO LOAD KEY.TOML\nError: {:?}\n", e));

    // Gemma config from the /gemma/gemma.toml file
    let gemma_config = GemmaConfig::load(&config.paths.gemma)
        .expect("\nAuto훈민정음: FAILED TO LOAD GEMMA.TOML\n");

    // Init and start ClockService
    let mut clock = ClockService::with().tempo(config.tempo.bpm as f64).build();
    // Start the clock thread or quit if it fails
    clock
        .start_thread()
        .expect("AutoHMJVis: fatal error: Failed to start clock thread");
    clock
        .start_clock()
        .expect("AutoHMJVis: fatal error: Failed to start clock");

    // Initialize event bus
    let event_bus = HMJEventBus::default();

    // Subscribe to Gemma events
    let gemma_rx = event_bus.gemma.subscribe();

    // Create shared Tokio runtime and distribute handles to services
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create shared Tokio runtime");
    let rthandle = runtime.handle().clone();

    // Initialize & start HMJServer
    let mut server = HMJServer::new(config.server.port, &event_bus, rthandle.clone());
    server.start().expect("Failed to start HMJServer");

    // Initialize live input registry
    let mut live_input_registry = LiveInputRegistry::new();

    // Initialize services
    let convo = ConversationService::new(&event_bus);

    let translate = TranslationService::new(
        &event_bus,
        config.translation.target_language,
        config.translation.enabled,
        config.translation.second_target_language,
        config.translation.enable_second_language,
        rthandle.clone(),
    );
    let ai = GemmaService::new(
        &gemma_config,
        &auth_config.google.api_key,
        &event_bus,
        rthandle,
    );

    // --- Load Font for Nannou Draw (Hangeul) ---
    // Assumes "assets/gulim.ttf" exists relative to the executable
    // or relative to the project root if running with `cargo run`
    let assets = app.assets_path().expect("Could not find assets directory");
    let font_path = assets.join("AppleMyungjo.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let korean_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    // --- Load Font for Nannou Draw (Latin) ---
    let font_path = assets.join("SometypeMono-v.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let latin_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    // --- Load Font for Nannou Draw (Symbols) ---
    let font_path = assets.join("AppleSymbols.ttf");
    let font_bytes = fs::read(&font_path)
        .unwrap_or_else(|_| panic!("Failed to read font file at {:?}", font_path));
    let symbol_font = Font::from_bytes(font_bytes)
        .unwrap_or_else(|_| panic!("Failed to load font at {:?}", font_path));

    let text_grid_fonts = TextGridFonts {
        hangeul: korean_font,
        latin: latin_font,
        symbols: symbol_font,
    };

    // Create main output window
    let audience_window_id = app
        .new_window()
        .title("Auto-훈민정음 0.1.0")
        .size(config.audience_window.width, config.audience_window.height)
        .msaa_samples(1)
        .view(audience_view)
        .build()
        .unwrap();

    // Create the performer window
    let performer_window_id = app
        .new_window()
        .title("Auto-훈민정음 0.1.0 Performer")
        .size(
            config.performer_window.width,
            config.performer_window.height,
        )
        .msaa_samples(1)
        .view(performer_view)
        .raw_event(raw_window_event)
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

    // Print the scale factor for the windows
    println!(
        "Audience window scale factor: {}",
        audience_window.scale_factor()
    );

    println!(
        "Performer window scale factor: {}",
        performer_window.scale_factor()
    );

    // Set up the draws for the windows
    let audience_draw = nannou::Draw::new();
    let performer_draw = nannou::Draw::new();

    // Set up render texture
    let draw = nannou::Draw::new();
    let device = audience_window.device();
    let mut rendering = Nnpipe::new(
        device,
        config.rendering_main.texture_width,
        config.rendering_main.texture_height,
        config.rendering_main.texture_samples,
    );

    // Set up the reshapers for the windows
    let audience_reshaper = rendering.create_reshaper_for_post_processed(device, &audience_window);
    let performer_reshaper =
        rendering.create_reshaper_for_post_processed(device, &performer_window);

    // Set up effects pipeline
    let lo_config = TextureConfig {
        width: config.rendering_main.texture_width / 2,
        height: config.rendering_main.texture_height / 2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
    };

    let med_config = TextureConfig {
        width: config.rendering_main.texture_width,
        height: config.rendering_main.texture_height,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
    };

    let hi_config = TextureConfig {
        width: config.rendering_main.texture_width,
        height: config.rendering_main.texture_height,
        format: wgpu::TextureFormat::Rgba16Float,
    };

    let effects = PipelineBuilder::new()
        .name("Particle Effects Pipeline")
        .brightness_extract(med_config, 0.5)
        .downsample(lo_config)
        .gaussian_blur_passes(lo_config, 2, 2.0, 5.0)
        .bloom_composite_with_curve(hi_config, 2.0, 3.0)
        .build(device);

    if let Ok(effect) = effects {
        rendering.add_multi_pipeline("effects", effect);
    }

    // Initialize three ContentManagers specific to this performance
    let content = init_three_content_managers(&event_bus, &mut live_input_registry, &gemma_config);

    // Initialize three text grids specific to this performance
    let grids = init_three_grids(
        &config.grid,
        &gemma_config,
        &config.osc_send,
        &text_grid_fonts,
        &clock,
        &event_bus,
    );

    // Set up FPS manager
    let mut fps = FpsManager::new_with(false, false);
    let performer_rect = performer_window.rect();
    fps.set_draw_position(pt2(
        performer_rect.left() + 40.0,
        performer_rect.top() - 10.0,
    ));

    // Load the intro image
    let mut intro_image = IntroImage::new(&config.paths.intro_image);
    intro_image.load(app);

    // Create the control panel for the performer window
    let control_panel = ControlPanel::new(&performer_window);

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        intro_image,

        convo,
        translate,
        ai,
        clock,

        gemma_rx,
        humans_turn: HumansTurn::True,

        content,
        grids,

        text_grid_fonts,

        audience_window_id,
        performer_window_id,

        audience_reshaper,
        performer_reshaper,

        draw,
        audience_draw,
        performer_draw,

        server,
        live_input_registry,
        cursor_positions: HashMap::new(),

        rendering: RefCell::new(rendering),

        update_time: std::time::Instant::now(),

        fps,

        show_fps: false,
        show_debug: false,
        control_panel,
        runtime,
    }
}

fn main() {
    nannou::app(model).update(update).run();
}

fn raw_window_event(_app: &App, model: &mut Model, event: &nannou::winit::event::WindowEvent) {
    model.control_panel.egui.handle_raw_event(event);
}

fn update_control_panel(model: &mut Model, update: Update) {
    model
        .control_panel
        .egui
        .set_elapsed_time(update.since_start);
    let ctx = model.control_panel.egui.begin_frame();

    let style = (*ctx.style()).clone();
    ctx.set_style(adjust_style_from(style));

    if model.control_panel.show {
        egui::TopBottomPanel::bottom("control_panel")
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                stroke: egui::Stroke::new(0.0, egui::Color32::from_rgb(10, 10, 10)),
                inner_margin: egui::style::Margin::symmetric(10.0, 10.0),
                outer_margin: egui::style::Margin::same(0.0),
                rounding: egui::Rounding::same(1.0),
                shadow: egui::epaint::Shadow::NONE,
            })
            .show(&ctx, |ui| {
                let panel_height = ui.available_height();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_height(panel_height);
                        ui.heading("Control Panel");
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label("Grid Color");
                        ui.add_space(4.0);
                        ui.label("Human");
                        let r_changed = ui
                            .add(
                                egui::Slider::new(
                                    &mut model.control_panel.human_color[0],
                                    0.0..=1.0,
                                )
                                .text("R"),
                            )
                            .changed();
                        let g_changed = ui
                            .add(
                                egui::Slider::new(
                                    &mut model.control_panel.human_color[1],
                                    0.0..=1.0,
                                )
                                .text("G"),
                            )
                            .changed();
                        let b_changed = ui
                            .add(
                                egui::Slider::new(
                                    &mut model.control_panel.human_color[2],
                                    0.0..=1.0,
                                )
                                .text("B"),
                            )
                            .changed();
                        let a_changed = ui
                            .add(
                                egui::Slider::new(
                                    &mut model.control_panel.human_color[3],
                                    0.0..=1.0,
                                )
                                .text("A"),
                            )
                            .changed();
                        if r_changed || g_changed || b_changed || a_changed {
                            model.control_panel.save_status =
                                Some("Unsaved changes...".to_string());
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                let [r, g, b, a] = model.control_panel.human_color;
                                let config = UiStateFileConfig {
                                    human_color: HumanColorConfig { r, g, b, a },
                                };
                                match UiStateConfig::save(&config) {
                                    Ok(()) => {
                                        model.control_panel.save_status = Some("Saved".to_string())
                                    }
                                    Err(e) => {
                                        model.control_panel.save_status =
                                            Some(format!("Error: {}", e))
                                    }
                                }
                            }
                            ui.add_space(15.0);
                            if ui.button("Revert").clicked() {
                                let loaded = UiStateConfig::load();
                                let c = loaded.human_color;
                                model.control_panel.human_color = [c.r, c.g, c.b, c.a];
                                model.control_panel.save_status =
                                    Some("Reverted changes.".to_string());
                            }
                        });
                        ui.label(model.control_panel.save_status.as_deref().unwrap_or(""));
                    });
                });
            });
    }

    // Always apply color to human grid (even when panel is hidden)
    let [r, g, b, a] = model.control_panel.human_color;
    if let Some(grid) = model.grids.iter_mut().find(|g| g.id == "Human") {
        grid.text_style.cell_bgcolor = rgba(r, g, b, a);
    }
}

fn adjust_style_from(style: egui::Style) -> egui::Style {
    let mut style = style;
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            egui::FontId::new(15.0, egui::FontFamily::Monospace),
        ),
        (
            egui::TextStyle::Body,
            egui::FontId::new(13.0, egui::FontFamily::Monospace),
        ),
        (
            egui::TextStyle::Button,
            egui::FontId::new(13.0, egui::FontFamily::Monospace),
        ),
        (
            egui::TextStyle::Small,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
        ),
    ]
    .into_iter()
    .collect();

    style.spacing.item_spacing = egui::vec2(10.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 2.0);
    style.spacing.slider_width = 150.0;

    let mut visuals = style.visuals.clone();
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(120, 120, 120);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 70, 70);
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0);
    visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(10, 10, 10));

    style.visuals = visuals;

    style
}

// The main update loop, runs at 60Hz per Nannou
fn update(_app: &App, model: &mut Model, update: Update) {
    model.update_time = std::time::Instant::now();

    // FPS update
    if model.show_fps {
        model.fps.update();
    }

    // Update services
    model.translate.update();
    model.ai.update();
    model.server.update();

    // Handle Gemma Moderator events
    handle_moderator_events(model);

    // Receive incoming datagrams and update connections
    receive_hmjmessage(model);

    // Process any new conversation items
    model.convo.update();

    // Update ContentManagers
    model.content.iter_mut().for_each(|content| {
        content.update();
    });

    // Update control panel (must be before grid updates so cell_bgcolor is fresh)
    update_control_panel(model, update);

    // Update Grid View modules
    model.grids.iter_mut().for_each(|grid| {
        grid.update(model.update_time);
    })
}

// The view functions get a copy of the resized main texture, then draw UI/HUD elements over it.
fn performer_view(app: &App, model: &Model, frame: Frame) {
    // Clear the performer draw before starting
    model.performer_draw.reset();

    // Get the post-processed texture view
    let _post_processed_view = model.rendering.borrow().get_post_processed_view();

    // Update reshaper if needed (could be cached in Model)
    model
        .rendering
        .borrow()
        .draw_to_frame(&model.performer_reshaper, &frame);

    // If audience is viewing intro image, show this message
    if model.intro_image.is_visible() {
        model
            .performer_draw
            .text("Intro Image Onscreen.\nSelect this window and press I to hide/show.")
            .x_y(0.0, 0.0)
            .wh(pt2(500.0, 500.0))
            .font_size(50)
            .color(RED);
    }

    // Handle FPS and origin display
    if model.show_fps {
        model.fps.draw(&model.performer_draw);
    }

    draw_humans_turn_indicator(&model.performer_draw, model.humans_turn, frame.rect());

    // Then draw UI over it
    let _ = model.performer_draw.to_frame(app, &frame);
    model.control_panel.egui.draw_to_frame(&frame).unwrap();
}

// The audience view is the main window that displays the performance, that the audience sees.
// Its main job is to display the post-processed texture view, resized to the venue resolution.\
// A debug view shows the bounds of the window and the axes at the origin.
fn audience_view(app: &App, model: &Model, frame: Frame) {
    // Begin rendering context
    {
        let mut rendering = model.rendering.borrow_mut();

        let window = app.main_window();
        let device = window.device();
        let mut encoder = rendering.create_command_encoder(device);
        let queue = window.queue();

        // Clear textures
        rendering.encode_clear_all_textures(&mut encoder, wgpu::Color::BLACK);

        // Draw content
        model.grids.iter().for_each(|grid| {
            grid.draw(&rendering.draw, model.update_time, model.show_debug);
        });

        rendering.encode_draw_commands(device, &mut encoder);

        /*
        if let Err(e) = rendering.execute_named_pipeline("effects", device, &mut encoder) {
            eprintln!("Error executing effects pipeline: {}", e);
        }
         */

        // Skip effects pipeline and use passthrough
        rendering.encode_passthrough_to_view(&mut encoder, rendering.output_view());

        rendering.submit_command_encoder(device, queue, encoder);

        rendering.draw_to_frame(&model.audience_reshaper, &frame);
    }

    // Clear the audience draw before starting
    model.audience_draw.reset();

    // Draw intro image if visible
    if model.intro_image.is_visible() {
        let rect = app.window(model.audience_window_id).unwrap().rect();

        // Draw intro image, sized to audience window
        model.intro_image.draw(&model.audience_draw, rect);
    }

    // Handle FPS and origin display
    if model.show_debug {
        draw_debug(app, model);
    }

    // Then draw UI over it
    let _ = model.audience_draw.to_frame(app, &frame);
}

// ****************************** View functions ***********************************

/// Initialize three ContentManagers specific to this performance
fn init_three_content_managers(
    event_bus: &HMJEventBus,
    live_input_registry: &mut LiveInputRegistry,
    gemma_config: &GemmaConfig,
) -> Vec<ContentManager> {
    let human_content = ContentManager::new("Human", event_bus, live_input_registry);

    let gemma1_content =
        ContentManager::new(&gemma_config.persona_1.id, event_bus, live_input_registry);

    let gemma2_content =
        ContentManager::new(&gemma_config.persona_2.id, event_bus, live_input_registry);

    vec![human_content, gemma1_content, gemma2_content]
}

// Initialize three text grids specific to this performance
fn init_three_grids(
    grid_config: &GridConfig,
    gemma_config: &GemmaConfig,
    osc_config: &OscSendConfig,
    text_grid_fonts: &TextGridFonts,
    clock: &ClockService,
    event_bus: &HMJEventBus,
) -> Vec<TextGrid> {
    let human_grid = TextGrid::new(
        "Human",
        true,
        TextGridPosition::Center,
        grid_config,
        osc_config,
        text_grid_fonts,
        clock,
        event_bus,
    );

    let gemma1_grid = TextGrid::new(
        &gemma_config.persona_1.id,
        false,
        TextGridPosition::Left,
        grid_config,
        osc_config,
        text_grid_fonts,
        clock,
        event_bus,
    );

    let gemma2_grid = TextGrid::new(
        &gemma_config.persona_2.id,
        false,
        TextGridPosition::Right,
        grid_config,
        osc_config,
        text_grid_fonts,
        clock,
        event_bus,
    );

    vec![gemma1_grid, human_grid, gemma2_grid]
}

// ****************************** Controller functions ******************************

// This function is a controller that coordinates the different services.
// Receive WebSocket messages from the client via the HMJServer.
// Trigger functionality among the various services.
fn receive_hmjmessage(model: &mut Model) {
    // Receive WebSocket messages
    while let Some(HMJMessageWrapper {
        author,
        message,
        command,
        cursor_position,
    }) = model.server.try_recv_message()
    {
        // Handle convo messages
        if let Some(message) = message {
            // Update live input registry for live display or finalize messages
            if message.ends_with('\n') {
                // Create convo item
                let entry = ConversationService::new_item(&author, &message);

                // Send to next AI speaker (disabled: now requires specific command)
                //let _ = model.ai.send(&entry, model.convo.entries());

                // Add message to conversation
                model.convo.add(entry);

                // Clear the live input
                model.live_input_registry.clear(&author);
                model.humans_turn = HumansTurn::False;
            } else {
                // Message is in progress - update the live input
                model.live_input_registry.update(&author, message);
                if let Some(cursor_position) = cursor_position {
                    model.cursor_positions.insert(author, cursor_position);
                }
            }
        }

        // Handle command messages
        if let Some(command) = command {
            println!("Receive command: {:#?}", command);
            match command {
                CommandMessage::OscLeftSetting(setting) => {
                    let Some(grid) = model.grids.iter_mut().find(|grid| grid.id == "Left") else {
                        println!("No grid named Left");
                        return;
                    };
                    println!("Setting Left OSC: {}", setting);
                    grid.sequencer.is_sending = setting;
                }
                CommandMessage::OscHumanSetting(setting) => {
                    let Some(grid) = model.grids.iter_mut().find(|grid| grid.id == "Human") else {
                        println!("No grid named Human");
                        return;
                    };
                    println!("Setting Human OSC: {}", setting);
                    grid.sequencer.is_sending = setting;
                }
                CommandMessage::OscRightSetting(setting) => {
                    let Some(grid) = model.grids.iter_mut().find(|grid| grid.id == "Right") else {
                        println!("No grid named Right");
                        return;
                    };
                    println!("Setting Right OSC: {}", setting);
                    grid.sequencer.is_sending = setting;
                }
                CommandMessage::AISend(ai_id) => match ai_id.as_str() {
                    "Left" => {
                        let _ = model
                            .ai
                            .send(GemmaPersona::Gemma1, None, model.convo.entries());
                    }
                    "Right" => {
                        let _ = model
                            .ai
                            .send(GemmaPersona::Gemma2, None, model.convo.entries());
                    }
                    "Both" => {
                        let _ = model
                            .ai
                            .send(GemmaPersona::Gemma1, None, model.convo.entries());
                        let _ = model
                            .ai
                            .send(GemmaPersona::Gemma2, None, model.convo.entries());
                    }
                    "Moderator" => {
                        let _ = model
                            .ai
                            .send(GemmaPersona::Moderator, None, model.convo.entries());
                    }
                    _ => {
                        println!("Unknown AI ID: {}", ai_id);
                    }
                },
                CommandMessage::ClearGrid(grid_id) => {
                    let Some(grid) = model.grids.iter_mut().find(|grid| grid.id == grid_id) else {
                        println!("No grid named {}", grid_id);
                        return;
                    };
                    grid.clear();
                }
                CommandMessage::ResetConversation => {
                    model.convo.reset();
                    for grid in model.grids.iter_mut() {
                        grid.clear();
                    }
                }
            }
        }
    }
}

// Handle moderator events from the GemmaService
fn handle_moderator_events(model: &mut Model) {
    while let Ok(event) = model.gemma_rx.try_recv() {
        if let GemmaEvent::ModeratorChooses(id) = event {
            println!("Moderator chooses: {:?}", id);
            if id == "Human" {
                model.humans_turn = HumansTurn::True;
            } else {
                let persona: GemmaPersona;
                if id == "Left" {
                    model.humans_turn = HumansTurn::False;
                    persona = GemmaPersona::Gemma1;
                    let _ = model.ai.send(persona, None, model.convo.entries());
                } else if id == "Right" {
                    model.humans_turn = HumansTurn::False;
                    persona = GemmaPersona::Gemma2;
                    let _ = model.ai.send(persona, None, model.convo.entries());
                } else {
                    model.humans_turn = HumansTurn::Error;
                }
            }
        }
    }
}

// ************************ Performer HUD  *************************************

// Draws the audience view debug mode -- a rectangle showing the bounds of the window,
// and the axes at the center origin.
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

// Draws the performer view HUD.
// A grid showing the squares the human grid to help with spacing, and an indicator for the human's turn.
// Also the current cursor position.

fn draw_humans_turn_indicator(draw: &Draw, humans_turn: HumansTurn, screen_rect: Rect) {
    let next_speaker_indicator_color = if humans_turn == HumansTurn::True {
        rgba(0.0, 0.83, 0.0, 1.0)
    } else if humans_turn == HumansTurn::False {
        rgba(0.72, 0.0, 0.0, 1.0)
    } else {
        rgba(0.8, 0.8, 0.0, 1.0)
    };

    let indicator_y = screen_rect.top() - 30.0;

    // Draw next speaker indicator
    draw.ellipse()
        .x_y(0.0, indicator_y)
        .w_h(20.0, 20.0)
        .color(next_speaker_indicator_color);
}

// ************************ Main window input  *************************************

// Shortcut keys are activated when the performer window is focused.
fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::T => {
            model.translate.toggle_first_enabled();
        }

        Key::Y => {
            model.translate.toggle_second_enabled();
        }

        Key::P => {
            model.show_fps = !model.show_fps;
            model.fps.toggle();
        }
        Key::D => {
            model.show_debug = !model.show_debug;
        }
        Key::I => {
            model.intro_image.toggle_visible();
        }
        Key::M => {
            model.control_panel.show = !model.control_panel.show;
        }
        Key::Escape => {
            app.quit();
        }
        _ => {}
    }
}

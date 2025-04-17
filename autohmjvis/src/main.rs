// src/main.rs

use egui::{FontData, FontDefinitions, FontFamily};
use nannou::{
    prelude::*,
    text::*,
    winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
};
use nannou_egui::{egui, Egui};
use nnpipe::*;
use std::{fs, time::Instant};

use autohmjeum::{config::Config, views::BackgroundManager};

struct Model {
    background: BackgroundManager,
    text_layout: Layout,

    // input
    input_string: String,
    input_history: Vec<String>,

    // for hangeul
    input_committed: String,
    input_composing: Vec<char>,

    main_font: Font,
    input_focus_next_frame: bool,

    // Random
    rng: nannou::rand::rngs::ThreadRng,

    // Nannou API
    draw: nannou::Draw,
    draw_renderer: nannou::draw::Renderer,

    texture_main: wgpu::Texture,
    texture_reshaper_main: wgpu::TextureReshaper,
    post_processing: Nnpipe,

    // Egui API
    egui: Egui,

    // FPS
    last_update: Instant,
    fps: f32,
    fps_update_interval: f32,
    frame_count: usize,
    last_fps_display_update: f32,
    frame_time_accumulator: f32,

    // When on, displays more verbose messages in terminal
    verbose: bool,
}

fn model(app: &App) -> Model {
    // Load config
    let config = Config::load().expect("\nAuto훈민정음: FAILED TO LOAD CONFIG.TOML\n");

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
        .view(view_main)
        .key_pressed(key_pressed)
        .build()
        .unwrap();

    // Create input window (egui)
    let input_window_id = app
        .new_window()
        .title("Auto-훈민정음 input 0.1.0")
        .size(config.input_window.width, config.input_window.height)
        .view(view_input)
        .raw_event(raw_window_event)
        .build()
        .unwrap();

    let main_window = app.window(main_window_id).unwrap();
    let input_window = app.window(input_window_id).unwrap();

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

    // --- Initialize Egui ---
    let egui = Egui::from_window(&input_window);
    let mut fonts = FontDefinitions::default();
    let font_data = FontData::from_static(include_bytes!("../assets/gulim.ttf")); // Adjust path relative to src/main.rs
    let font_name = "Gulim".to_owned();
    fonts.font_data.insert(font_name.clone(), font_data);
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, font_name.clone());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, font_name.clone());
    egui.ctx().set_fonts(fonts);

    let text_layout_builder = nannou::text::layout::Builder::default();
    let text_layout = text_layout_builder
        .line_spacing(25.0)
        .wrap_by_word()
        .left_justify()
        .build();

    Model {
        background: BackgroundManager::new(rgb(0.05, 0.03, 0.0)),
        text_layout,

        input_string: String::new(),
        input_history: Vec::new(),

        input_committed: String::new(),
        input_composing: Vec::new(),

        input_focus_next_frame: true,

        rng: nannou::rand::thread_rng(),

        draw,
        draw_renderer,
        texture_main,
        texture_reshaper_main,

        main_font,
        egui,

        post_processing,

        last_update: Instant::now(),
        fps: 0.0,
        fps_update_interval: 0.3,
        last_fps_display_update: 0.0,
        frame_count: 0,
        frame_time_accumulator: 0.0,

        verbose: false,
    }
}

fn main() {
    nannou::app(model).update(update).run();
}

fn update(app: &App, model: &mut Model, update: Update) {
    let now = Instant::now();
    let duration = now - model.last_update;
    let dt = duration.as_secs_f32();
    model.last_update = now;

    // FPS calculations
    if model.verbose {
        calculate_fps(app, model, dt);
    }

    // Grab the input from keyboard
    update_input(app, model, update);

    // Handle the background
    model.background.draw(&model.draw, app.time);

    // Update & draw
    draw_output(model);

    render_and_post(app, model);
}

fn view_main(_app: &App, model: &Model, frame: Frame) {
    //resize texture to screen
    let mut encoder = frame.command_encoder();

    model
        .texture_reshaper_main
        .encode_render_pass(frame.texture_view(), &mut encoder);
}

fn draw_output(model: &Model) {
    let (clusters, _) = cluster_jamo_with_spans(&model.input_composing);

    // 2) if the *whole* thing makes one syllable, show that; else show clusters
    let tail: Vec<char> = if let Some(one) = collapse_to_syllable(&clusters) {
        vec![one]
    } else {
        clusters
    };

    // 3) stitch together
    let mut display = model.input_committed.clone();
    display.extend(tail.iter());

    model
        .draw
        .text(&display)
        .layout(&model.text_layout)
        .width(1000.0)
        .font(model.main_font.clone())
        .x_y(0.0, 0.0)
        .color(rgba(0.71, 0.71, 1.0, 1.0))
        .font_size(50);
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

// ************************ FPS and debug display  *************************************

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
    draw.text(&format!("FPS: {:.1}", model.fps))
        .x_y(900.0, 520.0)
        .color(RED)
        .font_size(20);
}

fn init_fps(app: &App, model: &mut Model) {
    model.fps = 0.0;
    model.frame_count = 0;
    model.frame_time_accumulator = 0.0;
    model.last_fps_display_update = app.time;
}

fn calculate_fps(app: &App, model: &mut Model, dt: f32) {
    model.frame_count += 1;
    model.frame_time_accumulator += dt;
    let elapsed_since_last_fps_update = app.time - model.last_fps_display_update;
    if elapsed_since_last_fps_update >= model.fps_update_interval {
        if model.frame_count > 0 {
            let avg_frame_time = model.frame_time_accumulator / model.frame_count as f32;
            model.fps = if avg_frame_time > 0.0 {
                1.0 / avg_frame_time
            } else {
                0.0
            };
        }

        // Reset accumulators
        model.frame_count = 0;
        model.frame_time_accumulator = 0.0;
        model.last_fps_display_update = app.time;
    }
}

// ************************ Main window input  *************************************

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::P => {
            model.verbose = !model.verbose;
            init_fps(app, model);
        }
        Key::A => {
            // cheap way to make clippy quiet
        }
        _ => {}
    }
}

// ************************ Input window  *************************************

fn update_input(_app: &App, model: &mut Model, update: Update) {
    let egui = &mut model.egui;
    egui.set_elapsed_time(update.since_start);
    let ctx = egui.begin_frame();

    let text_edit_id = egui::Id::new("input_field");
    let output_panel_id = egui::Id::new("output_panel");

    let bottom_frame = egui::Frame {
        // inner_margin: egui::style::Margin::same(10.0), // Padding inside the frame
        // outer_margin: egui::style::Margin::same(0.0), // No margin outside the frame
        // rounding: egui::Rounding::none(), // Sharp corners
        // stroke: egui::Stroke::NONE, // No border stroke
        // shadow: egui::epaint::Shadow::NONE,        // No shadow
        fill: egui::Color32::from_rgb(0, 0, 0), // Dark background color
        ..Default::default()                    // Use defaults for unspecified fields
    };

    // Input Text field
    egui::TopBottomPanel::bottom("input_panel")
        .frame(bottom_frame) // Apply the custom frame style
        .resizable(false)
        .show(&ctx, |ui| {
            ui.vertical(|ui| {
                // Padding above input line (clickable)
                let top_padding_h = 10.0;
                let (padding_id, padding_rect) =
                    ui.allocate_space(egui::vec2(ui.available_width(), top_padding_h));

                // Make the padding part of the focusable area for the field
                let padding_response = ui.interact(padding_rect, padding_id, egui::Sense::click());
                if padding_response.clicked() {
                    //ui.memory_mut(|m| m.request_focus(text_edit_id));
                    model.input_focus_next_frame = true;
                }

                egui::Frame::none()
                    .inner_margin(egui::Margin {
                        left: 10.0,
                        right: 10.0,
                        top: 0.0,
                        bottom: 10.0,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Style the prompt label
                            ui.label(
                                egui::RichText::new("> ")
                                    .color(egui::Color32::WHITE) // Prompt color
                                    //.monospace() // Use monospace font
                                    .size(14.0), // Font size
                            );

                            // build the display field
                            let (clusters, _) = cluster_jamo_with_spans(&model.input_composing);

                            // 2) if the *whole* thing makes one syllable, show that; else show clusters
                            let tail: Vec<char> = if let Some(one) = collapse_to_syllable(&clusters)
                            {
                                vec![one]
                            } else {
                                clusters
                            };

                            // 3) stitch together
                            let mut display = model.input_committed.clone();
                            display.extend(tail.iter());

                            // Style the text edit field
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut display) // Use model.input_string or input_text
                                    .id_source(text_edit_id)
                                    .desired_width(f32::INFINITY) // Fill available width
                                    //.font(egui::TextStyle::Monospace) // Use monospace font
                                    .frame(false) // Remove the default TextEdit frame for flatter look
                                    .text_color(egui::Color32::WHITE), // Text color
                            );

                            // Auto-focus logic
                            if model.input_focus_next_frame {
                                response.request_focus();
                                model.input_focus_next_frame = false;
                            }

                            // Handle Enter key press
                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                println!("Input submitted: {}", display); // Use model.input_string or input_text
                                                                          // ** Trigger your actions (OSC, etc.) here **
                                model.input_history.push(display.clone());
                                model.input_committed.clear();
                                model.input_composing.clear();

                                response.request_focus();
                            }
                        }); // end horizontal
                    }); // end inner frame for input line
            }); // end vertical
        }); // end centered_and_justified

    let history_frame = egui::Frame {
        fill: egui::Color32::from_rgb(0, 0, 0),
        inner_margin: (egui::Margin {
            left: 10.0,
            right: 10.0,
            top: 10.0,
            bottom: 10.0,
        }),
        ..Default::default()
    };

    // History panel
    egui::CentralPanel::default()
        .frame(history_frame)
        .show(&ctx, |ui| {
            // Make the history area scrollable
            egui::ScrollArea::vertical()
                .auto_shrink([false, false]) // Fill available space
                .stick_to_bottom(true) // Keep scrolled to the bottom on new input
                .show(ui, |ui| {
                    // clickable for focus
                    let history_bg_rect = ui.available_rect_before_wrap();
                    let history_response = ui.interact(
                        history_bg_rect,
                        ui.id().with("history_frame_bg"),
                        egui::Sense::click(),
                    );

                    if history_response.clicked() {
                        // Set the flag to request focus for input field.
                        model.input_focus_next_frame = true;
                    }
                    // Iterate over the history and display each entry
                    for line in &model.input_history {
                        ui.label(
                            egui::RichText::new(line)
                                .color(egui::Color32::WHITE) // History text color
                                //.monospace()
                                .size(14.0),
                        );
                    }
                });
        });
}

fn raw_window_event(_app: &App, model: &mut Model, event: &nannou::winit::event::WindowEvent) {
    // Give egui a chance to handle non-jamo input
    model.egui.handle_raw_event(event);

    if let WindowEvent::KeyboardInput {
        input:
            KeyboardInput {
                virtual_keycode: Some(key),
                state: ElementState::Pressed,
                ..
            },
        ..
    } = event
    {
        match key {
            VirtualKeyCode::Return => {
                // Enter key (Return on macOS/Windows)
                handle_enter_commit(model);
            }
            VirtualKeyCode::Back => {
                println!(
                    "before: {}, {:?}",
                    model.input_committed, model.input_composing
                );
                // Backspace key (also Delete on macOS input for back deletion)
                handle_backspace(model);
                println!(
                    "after: {}, {:?}",
                    model.input_committed, model.input_composing
                );
            }
            _ => {}
        }
    }

    // Look for pure character input
    if let nannou::winit::event::WindowEvent::ReceivedCharacter(ch) = event {
        println!("Character: {}", ch);
        if is_punctuation(ch) {
            handle_punctuation_commit(model, *ch);
            return;
        }

        let code = *ch as u32;
        let is_jamo = hangeul::is_jamo(code) || hangeul::is_compat_jamo(code);
        let is_vowel = hangeul::is_moeum(code);

        if is_vowel {
            if model.input_composing.is_empty()
                && hangeul::ends_with_jongseong(&model.input_committed).unwrap_or(false)
            {
                let last = model.input_committed.pop().unwrap();
                let Ok((l, v, Some(tail))) = hangeul::decompose_char(&last) else {
                    todo!()
                };
                let (first_tail, second_tail) = split_final_jamo(tail);

                let base = first_tail
                    .map(|t1| hangeul::compose_char(&l, &v, Some(&t1)).unwrap())
                    .unwrap_or_else(|| hangeul::compose_char(&l, &v, None).unwrap());

                model.input_committed.push(base);
                model.input_composing.clear();
                model.input_composing.push(second_tail);
                model.input_committed.push(*ch);
                return;
            }

            // B) Case: it’s fully in your composing buffer as one syllable
            let (clusters, _spans) = cluster_jamo_with_spans(&model.input_composing);
            //if clusters.len() == 1 {
            if let Some(syl) = collapse_to_syllable(&clusters) {
                if hangeul::ends_with_jongseong(&syl.to_string()).unwrap_or(false) {
                    let Ok((l, v, Some(tail))) = hangeul::decompose_char(&syl) else {
                        todo!()
                    };
                    let (first_tail, second_tail) = split_final_jamo(tail);

                    let base = first_tail
                        .map(|t1| hangeul::compose_char(&l, &v, Some(&t1)).unwrap())
                        .unwrap_or_else(|| hangeul::compose_char(&l, &v, None).unwrap());

                    model.input_committed.push(base);
                    model.input_composing.clear();
                    model.input_composing.push(second_tail);
                    model.input_composing.push(*ch);
                    return;
                }
                //}
            }
        }

        // If not Hangeul jamo, commit immediately
        let code = *ch as u32;
        if !hangeul::is_jamo(code) && !hangeul::is_compat_jamo(code) {
            for c in &model.input_composing {
                model.input_committed.push(*c);
            }
            model.input_composing.clear();
            model.input_committed.push(*ch);
        } else {
            // jamo -> push into buffer
            model.input_composing.push(*ch);
            if model.input_composing.len() > 5 {
                let dropped = model.input_composing.remove(0);
                model.input_committed.push(dropped);
            }

            // commit any prefix *clusters* that can’t extend
            loop {
                let (clusters, spans) = cluster_jamo_with_spans(&model.input_composing);
                // if the *whole* clustered buffer is a valid syllable, stop
                if collapse_to_syllable(&clusters).is_some() {
                    break;
                }
                // otherwise find the longest prefix that *is* a syllable
                let mut did = false;
                for i in (1..clusters.len()).rev() {
                    if let Some(syll) = collapse_to_syllable(&clusters[..i]) {
                        // commit that syllable
                        model.input_committed.push(syll);

                        // remove exactly sum(spans[0..i]) raw chars
                        let raw_to_remove: usize = spans[..i].iter().sum();
                        model.input_composing.drain(0..raw_to_remove);
                        did = true;
                        break;
                    }
                }
                if !did {
                    break;
                }
            }
        }
    }

    if let nannou::winit::event::WindowEvent::Focused(focused) = event {
        if *focused {
            model.input_focus_next_frame = true;
        }
    }
}

fn view_input(_app: &App, model: &Model, frame: Frame) {
    model.egui.draw_to_frame(&frame).unwrap();
}

/// Collapse at most three elements (choseong, jungseong, jongseong) into one syllable.
fn collapse_to_syllable(clustered: &[char]) -> Option<char> {
    match clustered.len() {
        0 => None,
        1 => Some(clustered[0]),
        2 => hangeul::compose_char(&clustered[0], &clustered[1], None).ok(),
        3 => hangeul::compose_char(&clustered[0], &clustered[1], Some(&clustered[2])).ok(),
        _ => None,
    }
}

/// Try combining two simple jungseong into one compound vowel.
fn try_combine_vowel(a: char, b: char) -> Option<char> {
    match (a, b) {
        ('ㅗ', 'ㅏ') => Some('ㅘ'),
        ('ㅗ', 'ㅐ') => Some('ㅙ'),
        ('ㅗ', 'ㅣ') => Some('ㅚ'),
        ('ㅜ', 'ㅓ') => Some('ㅝ'),
        ('ㅜ', 'ㅔ') => Some('ㅞ'),
        ('ㅜ', 'ㅣ') => Some('ㅟ'),
        ('ㅡ', 'ㅣ') => Some('ㅢ'),
        _ => None,
    }
}

/// Try combining two simple jongseong into one compound final.
fn try_combine_final(a: char, b: char) -> Option<char> {
    match (a, b) {
        ('ㄱ', 'ㅅ') => Some('ㄳ'),
        ('ㄴ', 'ㅈ') => Some('ㄵ'),
        ('ㄴ', 'ㅎ') => Some('ㄶ'),
        ('ㄹ', 'ㄱ') => Some('ㄺ'),
        ('ㄹ', 'ㅁ') => Some('ㄻ'),
        ('ㄹ', 'ㅂ') => Some('ㄼ'),
        ('ㄹ', 'ㅅ') => Some('ㄽ'),
        ('ㄹ', 'ㅌ') => Some('ㄾ'),
        ('ㄹ', 'ㅍ') => Some('ㄿ'),
        ('ㄹ', 'ㅎ') => Some('ㅀ'),
        ('ㅂ', 'ㅅ') => Some('ㅄ'),
        _ => None,
    }
}

/// Returns (clusters, spans) where:
///  - `clusters[k]` is the k-th merged jamo (e.g. ㄹ+ㄱ → ㄺ), and
///  - `spans[k]` is how many raw characters that cluster consumed (1 or 2).
fn cluster_jamo_with_spans(raw: &[char]) -> (Vec<char>, Vec<usize>) {
    let mut clusters = Vec::new();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        // try double‑vowel first
        if i + 1 < raw.len() {
            if let Some(v) = try_combine_vowel(raw[i], raw[i + 1]) {
                clusters.push(v);
                spans.push(2);
                i += 2;
                continue;
            }
        }
        // try double‑final
        if i + 1 < raw.len() {
            if let Some(f) = try_combine_final(raw[i], raw[i + 1]) {
                clusters.push(f);
                spans.push(2);
                i += 2;
                continue;
            }
        }
        // else single jamo
        clusters.push(raw[i]);
        spans.push(1);
        i += 1;
    }
    (clusters, spans)
}

/// Given a final jamo (possibly compound), return (first, second).
/// If it isn’t compound, we interpret that as (no‑final, final).
fn split_final_jamo(j: char) -> (Option<char>, char) {
    match j {
        'ㄳ' => (Some('ㄱ'), 'ㅅ'),
        'ㄵ' => (Some('ㄴ'), 'ㅈ'),
        'ㄶ' => (Some('ㄴ'), 'ㅎ'),
        'ㄺ' => (Some('ㄹ'), 'ㄱ'),
        'ㄻ' => (Some('ㄹ'), 'ㅁ'),
        'ㄼ' => (Some('ㄹ'), 'ㅂ'),
        'ㄽ' => (Some('ㄹ'), 'ㅅ'),
        'ㄾ' => (Some('ㄹ'), 'ㅌ'),
        'ㄿ' => (Some('ㄹ'), 'ㅍ'),
        'ㅀ' => (Some('ㄹ'), 'ㅎ'),
        'ㅄ' => (Some('ㅂ'), 'ㅅ'),
        // not a compound final
        other if hangeul::is_jaeum(other as u32) => (None, other),
        _ => (None, j),
    }
}

fn finalize_composing_buffer(model: &mut Model) {
    let (clusters, _) = cluster_jamo_with_spans(&model.input_composing);

    if let Some(syllable) = collapse_to_syllable(&clusters) {
        model.input_committed.push(syllable);
    } else {
        for c in clusters {
            model.input_committed.push(c);
        }
    }
    model.input_composing.clear();
}

/// Called when Enter is pressed.
fn handle_enter_commit(model: &mut Model) {
    finalize_composing_buffer(model);
    let final_line = model.input_committed.clone();
    model.input_history.push(final_line);
    model.input_committed.clear();
}

/// Called when Space or punctuation is typed.
fn handle_punctuation_commit(model: &mut Model, ch: char) {
    finalize_composing_buffer(model);
    model.input_committed.push(ch);
}

fn handle_backspace(model: &mut Model) {
    // Step 1: pop from input_composing if it has anything
    if model.input_composing.pop().is_some() {
        println!("Composing pop");
        return;
    }

    // Case 2: composing is empty → decompose last syllable from committed
    if let Some(last) = model.input_committed.pop() {
        println!("Popped from committed: {}", last);
        if let Ok((lead, vowel, final_opt)) = hangeul::decompose_char(&last) {
            let mut jamo = vec![lead, vowel];
            if let Some(j) = final_opt {
                jamo.push(j);
            }

            model.input_composing = jamo;
            model.input_composing.pop();
            // The next backspace will remove the last jamo from this buffer
        } else {
            // If it's not decomposable, just treat it as a single pop
            // (e.g., English letters, symbols)
            // Already popped above
        }
    }
    if let Some(last) = model.input_committed.pop() {
        // Try to decompose the last character
        if let Ok((lead, vowel, final_opt)) = hangeul::decompose_char(&last) {
            model.input_composing.clear();
            model.input_composing.push(lead);
            model.input_composing.push(vowel);
            if let Some(final_jamo) = final_opt {
                model.input_composing.push(final_jamo);
            }
            // Remove the last jamo
            model.input_composing.pop();
        } else {
            // If not a decomposable Hangul syllable, do nothing or treat as raw pop
        }
    }
}

/// Determines if a character is considered punctuation or a space.
fn is_punctuation(c: &char) -> bool {
    matches!(
        c,
        '!' | '"'
            | '#'
            | '$'
            | '%'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | '-'
            | '.'
            | '/'
            | ':'
            | ';'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '['
            | '\\'
            | ']'
            | '^'
            | '_'
            | '`'
            | '{'
            | '|'
            | '}'
            | '~'
            | ' '
    )
}

// src/main.rs
//
// auto-hunminjeongak client
//
// handling user input

use autohmjclient::config::Config;
use eframe::{egui, CreationContext};
use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::{
    collections::BTreeMap,
    sync::mpsc::{channel, Receiver, Sender},
};
use tungstenite::{connect, Message};

use autohmjcommon::{History, HistoryItem};

// The application state for the input-only window
struct Model {
    // input from the user
    input_text: String,
    // History of submitted lines
    input_history: BTreeMap<usize, HistoryItem>,
    // Flag to request focus next frame
    input_focus_next_frame: bool,
    // text field id
    input_id: egui::Id,
    // WebSocket for communicating with server/visualizer
    ws_tx: Sender<String>,   // for sending commits
    ws_rx: Receiver<String>, // for receiving history dumps
    client_id: String,
}

impl Model {
    fn new(cfg: Config) -> Self {
        // Generate random client ID
        let client_id = cfg.client.id;
        let client_id_clone = client_id.clone();

        // Prepare WebSocket client channels: one for outgoing, one for incoming history
        let (ws_tx, ws_out_rx) = channel::<String>();
        let (ws_in_tx, ws_rx) = channel::<String>();
        let (status_tx, _status_rx) = channel::<bool>();

        let ws_url = format!("ws://{}:{}", cfg.server.address, cfg.server.port);

        // spawn WebSocket client thread
        std::thread::spawn(move || {
            let mut ws_opt: Option<
                tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
            > = None;
            let mut reconnect_delay = std::time::Duration::from_millis(500);
            let max_reconnect_delay = std::time::Duration::from_secs(10);

            loop {
                // Try to connect if not connected
                if ws_opt.is_none() {
                    match connect(&ws_url) {
                        Ok((mut socket, _)) => {
                            // Set non-blocking mode on the TCP stream
                            if let tungstenite::stream::MaybeTlsStream::Plain(tcp_stream) =
                                socket.get_ref()
                            {
                                let _ = tcp_stream.set_nonblocking(true);
                            }

                            // Register with server
                            let register_msg = format!("register|{}", client_id_clone);
                            let _ = socket.send(Message::Text(register_msg.into()));

                            ws_opt = Some(socket);
                            let _ = status_tx.send(true);
                            reconnect_delay = std::time::Duration::from_millis(100);
                        }
                        Err(_) => {
                            let _ = status_tx.send(false);
                            std::thread::sleep(reconnect_delay);
                            reconnect_delay =
                                std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
                            continue;
                        }
                    }
                }

                // Process WebSocket messages
                let mut should_reconnect = false;

                if let Some(ref mut socket) = ws_opt {
                    // Send pending messages
                    while let Ok(commit) = ws_out_rx.try_recv() {
                        if socket.send(Message::Text(commit.into())).is_err() {
                            should_reconnect = true;
                            break;
                        }
                    }

                    // Read incoming messages
                    if !should_reconnect {
                        match socket.read() {
                            Ok(Message::Text(utf8)) => {
                                let _ = ws_in_tx.send(utf8.to_string());
                            }
                            Err(tungstenite::Error::Io(ref e))
                                if e.kind() == std::io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                should_reconnect = true;
                            }
                            _ => {}
                        }
                    }
                }

                // Handle reconnection if needed
                if should_reconnect {
                    ws_opt = None;
                    let _ = status_tx.send(false);
                }

                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        });

        Self {
            input_text: String::new(),
            input_history: BTreeMap::new(),
            input_focus_next_frame: true,
            input_id: egui::Id::new("input_field"),
            ws_tx,
            ws_rx,
            client_id: client_id.clone(),
        }
    }
}

impl eframe::App for Model {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Build the UI
        let bottom_frame = egui::Frame {
            inner_margin: egui::Margin {
                left: 10,
                right: 10,
                top: 10,
                bottom: 10,
            },
            fill: egui::Color32::BLACK,
            ..Default::default()
        };
        egui::TopBottomPanel::bottom("input_panel")
            .frame(bottom_frame)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Prompt and input field
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("> ")
                                .monospace()
                                .size(20.0)
                                .color(egui::Color32::WHITE),
                        );
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.input_text)
                                .id_source(self.input_id)
                                .font(egui::FontId::new(20.0, FontFamily::Monospace))
                                .desired_width(f32::INFINITY)
                                .frame(false)
                                .text_color(egui::Color32::WHITE),
                        );

                        // handle focus
                        if self.input_focus_next_frame {
                            response.request_focus();
                            self.input_focus_next_frame = false;
                        }

                        // push entry into history
                        if response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // send commit to server
                            let commit = format!("{}:{}\n", self.client_id, self.input_text);
                            let _ = self.ws_tx.send(commit);
                            // clear the input field
                            self.input_text.clear();
                            // keep the focus so they can type again immediately
                            response.request_focus();
                        } else {
                            // Stream current text (without newline = not committed)
                            let payload = format!("{}:{}", self.client_id, self.input_text);
                            let _ = self.ws_tx.send(payload);
                        }

                        // Drain any incoming history dumps
                        while let Ok(dump) = self.ws_rx.try_recv() {
                            if let Ok(History(full_history)) =
                                serde_json::from_str::<History>(&dump)
                            {
                                println!(
                                    "Received history update with {} items",
                                    full_history.len()
                                );
                                self.input_history = full_history;
                                //println!("      {:?}", self.input_history);
                            }
                        }
                    });
                });
            });

        // History panel
        let history_frame = egui::Frame {
            fill: egui::Color32::BLACK,
            inner_margin: egui::Margin {
                left: 10,
                right: 10,
                top: 10,
                bottom: 10,
            },
            ..Default::default()
        };
        egui::CentralPanel::default()
            .frame(history_frame)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .id_salt("history_scroll")
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
                            self.input_focus_next_frame = true;
                        }

                        // Display each entry in history
                        for entry in self.input_history.values() {
                            // First add the author name
                            let author_text = if entry.author == self.client_id {
                                "You:"
                            } else {
                                &entry.author
                            };

                            ui.horizontal(|ui| {
                                // Author name
                                ui.label(
                                    egui::RichText::new(author_text)
                                        .monospace()
                                        .size(20.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                ui.add_space(10.0);
                                // Message with indentation
                                ui.vertical(|ui| {
                                    // Original message
                                    ui.label(
                                        egui::RichText::new(entry.msg.trim_end())
                                            .monospace()
                                            .size(20.0)
                                            .color(egui::Color32::WHITE),
                                    );

                                    // Translation if available
                                    if let Some(trans) = &entry.translation {
                                        ui.label(
                                            egui::RichText::new(format!("({})", trans))
                                                .monospace()
                                                .size(16.0)
                                                .color(egui::Color32::from_rgb(150, 150, 150))
                                                .italics(),
                                        );
                                        ui.add_space(10.0);
                                    } else {
                                        ui.add_space(29.0);
                                    }
                                });
                            });
                        }
                    })
            });

        // After the UI is built, stream the current text live:
        let payload = format!("{}:{}", self.client_id, self.input_text);
        let _ = self.ws_tx.send(payload);
    }
}

fn main() {
    // Load config (if needed for sizing)
    let cfg = Config::load().expect("FAILED TO LOAD CONFIG");
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([cfg.window.width, cfg.window.height])
            .with_active(true),
        ..Default::default()
    };

    eframe::run_native(
        "Auto-훈민정음 Input",
        native_options,
        Box::new(|cc| {
            let fonts = make_gulim_fonts();
            cc.egui_ctx.set_fonts(fonts);
            set_styles(cc);
            Ok(Box::new(Model::new(cfg)))
        }),
    )
    .unwrap();
}

fn make_gulim_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let font_data = FontData::from_static(include_bytes!("../assets/gulim.ttf"));
    let name = "Gulim".to_owned();
    fonts.font_data.insert(name.clone(), font_data.into());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, name.clone());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, name);

    fonts
}

fn set_styles(cc: &CreationContext) {
    let mut style = (*cc.egui_ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(20.0, FontFamily::Monospace),
    );
    cc.egui_ctx.set_style(style);
}

// src/main.rs
//
// auto-hunminjeongak client
//
// handling user input

use autohmjclient::config::Config;
use eframe::{egui, CreationContext};
use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::collections::BTreeMap;

use autohmjclient::client::HMJClient;
use autohmjcommon::{Conversation, ConvoWrapper, HMJMessage};

// The application state for the input-only window
struct Model {
    // input from the user
    input_text: String,
    // History of submitted lines
    conversation: Conversation,
    // Flag to request focus next frame
    input_focus_next_frame: bool,
    // text field id
    input_id: egui::Id,

    // WebSocket Client
    client: HMJClient,
    client_id: String,
}

impl Model {
    fn new(cfg: Config) -> Self {
        // Get client ID from config
        let client_id = cfg.client.id;

        // Create async WebSocket client
        let client = HMJClient::new(&cfg.server.address, cfg.server.port as u32, &client_id);

        Self {
            input_text: String::new(),
            conversation: BTreeMap::new(),
            input_focus_next_frame: true,
            input_id: egui::Id::new("input_field"),

            client,
            client_id: client_id.to_owned(),
        }
    }

    /**************************** Window components **************************************** */
    fn build_input_frame(&mut self, ctx: &egui::Context) {
        // Define UI style
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

        // Build the UI
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

                        if response.lost_focus() {
                            self.input_focus_next_frame = true;
                        }

                        // push entry into conversation
                        if response.lost_focus()
                            && !self.input_text.is_empty()
                            && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            // send commit to server
                            let commit = HMJMessage(
                                self.client_id.to_owned(),
                                self.input_text.trim().to_owned() + "\n",
                            );
                            self.client.send(commit);
                            // clear the input field
                            self.input_text.clear();
                            // keep the focus so they can type again immediately
                            response.request_focus();
                        } /*else {
                              // Stream current text (without newline = not committed)
                              let payload = format!("{}:{}", self.client_id, self.input_text);
                              self.client.send(payload);
                          }*/
                    });
                });
            });
    }

    fn build_conversation_frame(&mut self, ctx: &egui::Context) {
        // Define UI Style
        let convo_frame = egui::Frame {
            fill: egui::Color32::BLACK,
            inner_margin: egui::Margin {
                left: 10,
                right: 10,
                top: 10,
                bottom: 10,
            },
            ..Default::default()
        };

        // build UI
        egui::CentralPanel::default()
            .frame(convo_frame)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .id_salt("conversation_scroll")
                    .show(ui, |ui| {
                        // clickable for focus
                        let convo_bg_rect = ui.available_rect_before_wrap();
                        let convo = ui.interact(
                            convo_bg_rect,
                            ui.id().with("convo_frame_bg"),
                            egui::Sense::click(),
                        );

                        if convo.clicked() {
                            // Set the flag to request focus for input field.
                            self.input_focus_next_frame = true;
                        }

                        // Display each entry in convo
                        for entry in self.conversation.values() {
                            // First add the author name
                            let author_text = if entry.author == self.client_id {
                                "You:".to_string()
                            } else {
                                format!("{}:", &entry.author)
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
                                        egui::RichText::new(entry.message.trim_end())
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
    }
}

impl eframe::App for Model {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process incoming messages from server
        while let Some(msg) = self.client.try_recv() {
            match serde_json::from_str::<ConvoWrapper>(&msg) {
                Ok(ConvoWrapper(conversation)) => {
                    println!(
                        "Received conversation update with {} items",
                        conversation.len()
                    );
                    self.conversation = conversation;
                }
                Err(e) => {
                    eprintln!("Error parsing response from server: {}", e);
                }
            }
        }

        // Display the major UI elements
        self.build_input_frame(ctx);
        self.build_conversation_frame(ctx);

        // After the UI is built, stream the current text live:
        let payload = HMJMessage(self.client_id.to_owned(), self.input_text.to_owned());
        self.client.send(payload);
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

/**************************** Text display style functions ***************************** */

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

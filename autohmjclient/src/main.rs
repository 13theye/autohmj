// src/main.rs
//
// auto-hunminjeongak client
//
// handling user input

use autohmjclient::config::Config;
use eframe::{egui, CreationContext};
use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::collections::BTreeMap;

use autohmjclient::{client::HMJClient, control::Settings};
use autohmjcommon::{CommandMessage, Conversation, ConvoWrapper, HMJMessageWrapper};

// The application state for the input-only window
struct Model {
    // input from the user
    input_text: String,
    // cursor position
    cursor_position: Option<usize>,
    // History of submitted lines
    conversation: Conversation,
    // Flag to request focus next frame
    input_focus_next_frame: bool,
    // text field id
    input_id: egui::Id,

    // WebSocket Client
    client: HMJClient,
    client_id: String,

    // Settings for the vis
    settings: Settings,
}

impl Model {
    fn new(cfg: Config) -> Self {
        // Get client ID from config
        let client_id = cfg.client.id;

        // Create async WebSocket client
        let client = HMJClient::new(&cfg.server.address, cfg.server.port as u32, &client_id);

        Self {
            input_text: String::new(),
            cursor_position: None,
            conversation: BTreeMap::new(),
            input_focus_next_frame: true,
            input_id: egui::Id::new("input_field"),

            client,
            client_id: client_id.to_owned(),

            settings: Settings::default(),
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

        // UI Settings
        let mut osc_left_changed = false;
        let mut osc_right_changed = false;
        let mut osc_human_changed = false;

        // AI Button clicked
        let mut ai_left_clicked = false;
        let mut ai_right_clicked = false;
        let mut ai_both_clicked = false;
        let mut ai_moderator_clicked = false;

        // Clear Grid button clicked
        let mut clear_left_clicked = false;
        let mut clear_human_clicked = false;
        let mut clear_right_clicked = false;
        let mut clear_all_clicked = false;

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
                        let text_edit_output = egui::TextEdit::singleline(&mut self.input_text)
                            .id_source(self.input_id)
                            .font(egui::FontId::new(20.0, FontFamily::Monospace))
                            .desired_width(f32::INFINITY)
                            .frame(false)
                            .text_color(egui::Color32::WHITE)
                            .show(ui);

                        let response = text_edit_output.response;

                        // Extract cursor position
                        if let Some(cursor_range) = text_edit_output.cursor_range {
                            // Get the cursor position (use the primary cursor position)
                            let cursor_pos = cursor_range.primary.ccursor.index;
                            self.cursor_position = Some(cursor_pos);
                        }

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
                            let commit = HMJMessageWrapper {
                                author: self.client_id.to_owned(),
                                message: Some(self.input_text.to_owned() + "\n"),
                                command: None,
                                cursor_position: self.cursor_position,
                            };
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
                    }); // ui horizontal
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_min_width(100.0);
                            ui.label(
                                egui::RichText::new("Send to AI:")
                                    .color(egui::Color32::from_rgb(150, 150, 150)),
                            );
                        });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    ai_left_clicked = ui.add(egui::Button::new("Left")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    ai_right_clicked = ui.add(egui::Button::new("Right")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    ai_both_clicked = ui.add(egui::Button::new("Both")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    ai_moderator_clicked =
                                        ui.add(egui::Button::new("Moderator")).clicked();
                                });
                            }); // ui horizontal for Send to AI
                        }); // ui vertical for label and buttons
                    }); // ui horizontal for label and buttons
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_min_width(100.0);
                            ui.label(
                                egui::RichText::new("OSC Sending:")
                                    .color(egui::Color32::from_rgb(150, 150, 150)),
                            );
                        });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    osc_left_changed = ui
                                        .add(egui::Checkbox::new(
                                            &mut self.settings.send_osc_left,
                                            egui::RichText::new("Left")
                                                .strong()
                                                .color(egui::Color32::from_rgb(150, 150, 150)),
                                        ))
                                        .changed();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    osc_human_changed = ui
                                        .add(egui::Checkbox::new(
                                            &mut self.settings.send_osc_human,
                                            egui::RichText::new("Human")
                                                .strong()
                                                .color(egui::Color32::from_rgb(150, 150, 150)),
                                        ))
                                        .changed();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    osc_right_changed = ui
                                        .add(egui::Checkbox::new(
                                            &mut self.settings.send_osc_right,
                                            egui::RichText::new("Right")
                                                .strong()
                                                .color(egui::Color32::from_rgb(150, 150, 150)),
                                        ))
                                        .changed();
                                });
                            }); // ui horizontal for Send OSC
                        }); // ui vertical for label and buttons
                    }); // ui horizontal for label and buttons
                    ui.add_space(7.0);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_min_width(100.0);
                            ui.label(
                                egui::RichText::new("Clear Grid:")
                                    .color(egui::Color32::from_rgb(150, 150, 150)),
                            );
                        });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    clear_left_clicked =
                                        ui.add(egui::Button::new("Left")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    clear_human_clicked =
                                        ui.add(egui::Button::new("Human")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    clear_right_clicked =
                                        ui.add(egui::Button::new("Right")).clicked();
                                });
                                ui.vertical(|ui| {
                                    ui.set_min_width(70.0);
                                    clear_all_clicked = ui.add(egui::Button::new("All")).clicked();
                                });
                            }); // ui horizontal for Clear
                        }); // ui vertical for label and buttons
                    }); // ui horizontal for label and buttons
                }); // outermost ui vertical
            }); // ui topbottompanel

        // If any of the OSC settings changed, send a message to the server
        if osc_left_changed {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::OscLeftSetting(self.settings.send_osc_left)),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if osc_right_changed {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::OscRightSetting(
                    self.settings.send_osc_right,
                )),
                cursor_position: None,
            };
            self.client.send(payload);
        }
        if osc_human_changed {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::OscHumanSetting(
                    self.settings.send_osc_human,
                )),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if ai_left_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::AISend("Left".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if ai_right_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::AISend("Right".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if ai_both_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::AISend("Both".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if ai_moderator_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::AISend("Moderator".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if clear_left_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::ClearGrid("Left".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if clear_human_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::ClearGrid("Human".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if clear_right_clicked {
            let payload = HMJMessageWrapper {
                author: self.client_id.to_owned(),
                message: None,
                command: Some(CommandMessage::ClearGrid("Right".to_string())),
                cursor_position: None,
            };
            self.client.send(payload);
        }

        if clear_all_clicked {
            for name in ["Left", "Human", "Right"] {
                let payload = HMJMessageWrapper {
                    author: self.client_id.to_owned(),
                    message: None,
                    command: Some(CommandMessage::ClearGrid(name.to_string())),
                    cursor_position: None,
                };
                self.client.send(payload);
            }
        }
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
                                        egui::RichText::new(entry.message.to_owned())
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
        let payload = HMJMessageWrapper {
            author: self.client_id.to_owned(),
            message: Some(self.input_text.to_owned()),
            command: None,
            cursor_position: self.cursor_position,
        };
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
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(150, 150, 150));
    cc.egui_ctx.set_style(style);
}

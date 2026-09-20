use eframe::egui::{self, Color32, Rect, RichText, Rounding, Stroke, Vec2, ViewportBuilder, ViewportId};
use crate::create_f_icon;
use crate::db::tns::{TnsEntry, TnsManager};
use crate::models::ConnectionConfig;
use crate::ui::tns_editor::TnsEditorView;

pub struct ServerListView {
    pub tns_manager: TnsManager,
    pub editor_open: bool,
    pub tns_editor: Option<TnsEditorView>,
    pub login_target: Option<TnsEntry>,
    pub username_input: String,
    pub password_input: String,
    pub remember_me: bool,
    pub error_message: Option<String>,
}

impl ServerListView {
    pub fn new() -> Self {
        let tns_manager = TnsManager::new();
        Self {
            tns_manager,
            editor_open: false,
            tns_editor: None,
            login_target: None,
            username_input: "SCOTT".to_string(),
            password_input: "tiger".to_string(),
            remember_me: true,
            error_message: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<ConnectionConfig> {
        let mut result_config = None;

        // Main Window: The KakaoTalk Server List (NEVER disappears)
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(egui::Margin::symmetric(16.0, 16.0)))
            .show(ctx, |ui| {
                if self.editor_open {
                    if let Some(editor) = &mut self.tns_editor {
                        if editor.show(ui, &mut self.tns_manager) {
                            self.editor_open = false;
                        }
                    }
                } else {
                    // Header Bar
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("접속가능한 서버 목록")
                                .size(16.0)
                                .strong()
                                .color(Color32::from_rgb(24, 24, 27)),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let edit_btn = egui::Button::new(
                                RichText::new("📝 tnsnames.ora").size(11.0).color(Color32::from_rgb(24, 24, 27)),
                            )
                            .fill(Color32::from_rgb(244, 244, 245))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .rounding(Rounding::same(4.0));

                            if ui.add(edit_btn).clicked() {
                                self.tns_editor = Some(TnsEditorView::new(&self.tns_manager));
                                self.editor_open = true;
                            }
                        });
                    });

                    ui.add_space(14.0);

                    // Section Count
                    ui.label(
                        RichText::new(format!("등록된 서버 {}", self.tns_manager.entries.len()))
                            .size(11.0)
                            .color(Color32::from_rgb(113, 113, 122)),
                    );

                    ui.add_space(8.0);

                    // Scrollable Friends List of Servers
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for entry in &self.tns_manager.entries {
                                let row_id = ui.id().with(&entry.alias);
                                let row_width = ui.available_width();
                                let row_height = 54.0;
                                let row_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(row_width, row_height));
                                let response = ui.interact(row_rect, row_id, egui::Sense::click());

                                let is_hovered = response.hovered();
                                if is_hovered {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let row_bg = if is_hovered {
                                    Color32::from_rgb(244, 244, 246)
                                } else {
                                    Color32::WHITE
                                };

                                let stroke = if is_hovered {
                                    Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_rgb(238, 238, 240))
                                };

                                egui::Frame::none()
                                    .fill(row_bg)
                                    .stroke(stroke)
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // Avatar squircle
                                            egui::Frame::none()
                                                .fill(Color32::from_rgb(24, 24, 27))
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(egui::Margin::symmetric(9.0, 7.0))
                                                .show(ui, |ui| {
                                                    let initial = if entry.alias.len() > 2 {
                                                        &entry.alias[..2]
                                                    } else {
                                                        &entry.alias
                                                    };
                                                    ui.label(RichText::new(initial).size(12.0).strong().color(Color32::WHITE));
                                                });

                                            ui.add_space(8.0);

                                            // Server Name & Address
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&entry.alias)
                                                        .size(13.0)
                                                        .strong()
                                                        .color(Color32::from_rgb(24, 24, 27)),
                                                );
                                                ui.label(
                                                    RichText::new(format!("{}:{} / {}", entry.host, entry.port, entry.service_name))
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(113, 113, 122)),
                                                );
                                            });

                                            // Right indicator
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                let badge_color = if is_hovered {
                                                    Color32::from_rgb(24, 24, 27)
                                                } else {
                                                    Color32::from_rgb(113, 113, 122)
                                                };
                                                ui.label(RichText::new("접속 〉").size(11.0).color(badge_color));
                                            });
                                        });
                                    });

                                // When server clicked, open a brand new window!
                                if response.clicked() {
                                    self.login_target = Some(entry.clone());
                                    self.error_message = None;
                                }

                                ui.add_space(6.0);
                            }
                        });
                }
            });

        // Separate Window: Opens a New Window for the Selected Server (KakaoTalk Chat-Room Style)
        if let Some(server) = self.login_target.clone() {
            let viewport_id = ViewportId::from_hash_of(format!("login_{}", server.alias));
            let mut close_dialog = false;
            let mut connect_now = false;

            let username_ref = &mut self.username_input;
            let password_ref = &mut self.password_input;
            let remember_ref = &mut self.remember_me;
            let err_ref = &self.error_message;

            ctx.show_viewport_immediate(
                viewport_id,
                ViewportBuilder::default()
                    .with_title(format!("Flipsy - {}", server.alias))
                    .with_icon(create_f_icon())
                    .with_inner_size(Vec2::new(320.0, 420.0))
                    .with_min_inner_size(Vec2::new(300.0, 360.0))
                    .with_resizable(false),
                |child_ctx, _class| {
                    if child_ctx.input(|i| i.viewport().close_requested()) {
                        close_dialog = true;
                    }

                    egui::CentralPanel::default()
                        .frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(egui::Margin::same(18.0)))
                        .show(child_ctx, |ui| {
                            ui.vertical(|ui| {
                                // Server Alias Title & Status Dot
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&server.alias)
                                            .size(18.0)
                                            .strong()
                                            .color(Color32::from_rgb(24, 24, 27)),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(
                                            RichText::new("● 1521 Open")
                                                .size(11.0)
                                                .strong()
                                                .color(Color32::from_rgb(22, 163, 74)),
                                        );
                                    });
                                });

                                ui.add_space(8.0);

                                // Structured Info (IP, Port, Service Name) - NO black banner
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(248, 249, 250))
                                    .rounding(Rounding::same(6.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                                    .show(ui, |ui| {
                                        egui::Grid::new("server_meta_grid")
                                            .num_columns(2)
                                            .spacing(Vec2::new(10.0, 5.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new("IP").size(11.0).color(Color32::from_rgb(113, 113, 122)).monospace());
                                                ui.label(RichText::new(format!(": {}", server.host)).size(11.0).strong().color(Color32::from_rgb(24, 24, 27)).monospace());
                                                ui.end_row();

                                                ui.label(RichText::new("Port").size(11.0).color(Color32::from_rgb(113, 113, 122)).monospace());
                                                ui.label(RichText::new(format!(": {}", server.port)).size(11.0).strong().color(Color32::from_rgb(24, 24, 27)).monospace());
                                                ui.end_row();

                                                ui.label(RichText::new("Service Name").size(11.0).color(Color32::from_rgb(113, 113, 122)).monospace());
                                                ui.label(RichText::new(format!(": {}", server.service_name)).size(11.0).strong().color(Color32::from_rgb(24, 24, 27)).monospace());
                                                ui.end_row();
                                            });
                                    });

                                ui.add_space(14.0);

                                // Stacked Credentials Box
                                egui::Frame::none()
                                    .fill(Color32::WHITE)
                                    .rounding(Rounding::same(6.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216)))
                                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                    .show(ui, |ui| {
                                        let label_color = Color32::from_rgb(113, 113, 122);
                                        let divider_color = Color32::from_rgb(238, 238, 240);

                                        // Username
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("아이디").size(11.0).color(label_color));
                                            ui.add(
                                                egui::TextEdit::singleline(username_ref)
                                                    .hint_text("SCOTT")
                                                    .desired_width(190.0)
                                                    .frame(false),
                                            );
                                        });

                                        ui.add_space(3.0);
                                        ui.painter().hline(
                                            ui.min_rect().x_range(),
                                            ui.cursor().top(),
                                            Stroke::new(1.0_f32, divider_color),
                                        );
                                        ui.add_space(4.0);

                                        // Password
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("비밀번호").size(11.0).color(label_color));
                                            ui.add(
                                                egui::TextEdit::singleline(password_ref)
                                                    .password(true)
                                                    .hint_text("비밀번호 입력")
                                                    .desired_width(180.0)
                                                    .frame(false),
                                            );
                                        });
                                    });

                                ui.add_space(10.0);

                                // Remember Checkbox
                                ui.checkbox(
                                    remember_ref,
                                    RichText::new("접속 정보 저장").size(11.0).color(Color32::from_rgb(113, 113, 122)),
                                );

                                ui.add_space(14.0);

                                // Login Action Button: Centered, Aesthetic "▶   CONNECT"
                                ui.vertical_centered(|ui| {
                                    let btn_w = 260.0;
                                    let btn_h = 42.0;
                                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(btn_w, btn_h), egui::Sense::click());

                                    let bg = if resp.is_pointer_button_down_on() {
                                        Color32::from_rgb(9, 9, 11)
                                    } else if resp.hovered() {
                                        Color32::from_rgb(39, 39, 42)
                                    } else {
                                        Color32::from_rgb(24, 24, 27)
                                    };

                                    let stroke = if resp.hovered() {
                                        Color32::from_rgb(82, 82, 91)
                                    } else {
                                        Color32::from_rgb(39, 39, 42)
                                    };

                                    ui.painter().rect(
                                        rect,
                                        Rounding::same(8.0),
                                        bg,
                                        Stroke::new(1.0_f32, stroke),
                                    );

                                    // Centered "▶   CONNECT" with high typographic balance
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "▶   CONNECT",
                                        egui::FontId::proportional(14.0),
                                        Color32::WHITE,
                                    );

                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    resp.clone().on_hover_text("Oracle 26ai 접속 (Enter)");

                                    if resp.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        connect_now = true;
                                    }
                                });

                                if let Some(err) = err_ref {
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(err).color(Color32::from_rgb(220, 38, 38)).size(11.0));
                                }
                            });
                        });
                },
            );

            if close_dialog {
                self.login_target = None;
                self.error_message = None;
            }

            if connect_now {
                result_config = Some(ConnectionConfig {
                    alias: server.alias.clone(),
                    host: server.host.clone(),
                    port: server.port,
                    service_name: server.service_name.clone(),
                    username: self.username_input.clone(),
                    password: self.password_input.clone(),
                });
            }
        }

        result_config
    }
}

use eframe::egui::{self, Color32, FontId, Key, RichText, Rounding, Stroke};
use crate::db::session::{format_sql, generate_bind_extraction_query};

pub struct EditorAction {
    pub run_requested: bool,
    pub explain_requested: bool,
    pub execute_custom_sql: Option<String>,
}

pub struct EditorView {
    pub sql: String,
    pub show_bind_panel: bool,
    pub copied_feedback: bool,
}

impl EditorView {
    pub fn new() -> Self {
        let initial_sql = r#"SELECT /*+ GATHER_PLAN_STATISTICS */
       I.CENTER_CD,
       I.ITEM_CD,
       I.ALLOCATED_QTY,
       M.ORDER_DATE,
       M.MOVE_STATUS
  FROM TB_CENTER_INVENTORY I
  JOIN TB_INVENTORY_MOVE_ORDER M
    ON I.ITEM_CD = M.ITEM_CD
 WHERE I.CENTER_CD = 'HUB_01'
   AND M.MOVE_STATUS = 'READY';"#;

        Self {
            sql: initial_sql.to_string(),
            show_bind_panel: false,
            copied_feedback: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorAction {
        let mut action = EditorAction {
            run_requested: false,
            explain_requested: false,
            execute_custom_sql: None,
        };

        let mut format_requested = false;

        ui.input(|i| {
            if i.modifiers.command && i.key_pressed(Key::Enter) {
                action.run_requested = true;
            }
            if i.key_pressed(Key::F10) {
                action.explain_requested = true;
            }
            if i.key_pressed(Key::F8) {
                format_requested = true;
            }
        });

        if format_requested {
            self.sql = format_sql(&self.sql);
        }

        // Check for bind variables in current SQL
        let candidate_bind_query = generate_bind_extraction_query(&self.sql);
        let has_binds = candidate_bind_query.is_some();

        egui::Frame::none()
            .fill(Color32::WHITE)
            .rounding(Rounding::same(6.0))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("SQL Worksheet")
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(24, 24, 27)),
                    );
                    ui.label(
                        RichText::new("(단축키: Ctrl+Enter 실행 | F10 XPlan 계획 | F8 포맷)")
                            .size(11.0)
                            .color(Color32::from_rgb(113, 113, 122)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Format Button
                        let fmt_btn = egui::Button::new(
                            RichText::new("✨ 포맷 (F8)")
                                .size(11.0)
                                .color(Color32::from_rgb(24, 24, 27)),
                        )
                        .fill(Color32::from_rgb(244, 244, 245))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                        .rounding(Rounding::same(4.0));

                        if ui.add(fmt_btn).clicked() {
                            self.sql = format_sql(&self.sql);
                        }

                        // Bind Extractor Button (Visible if bind detected)
                        if has_binds {
                            let bind_btn = egui::Button::new(
                                RichText::new(if self.show_bind_panel { "🔍 바인드 닫기" } else { "🔍 바인드 추출" })
                                    .size(11.0)
                                    .strong()
                                    .color(if self.show_bind_panel { Color32::WHITE } else { Color32::from_rgb(2, 132, 199) }),
                            )
                            .fill(if self.show_bind_panel { Color32::from_rgb(2, 132, 199) } else { Color32::from_rgb(240, 249, 255) })
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(186, 230, 253)))
                            .rounding(Rounding::same(4.0));

                            if ui.add(bind_btn).clicked() {
                                self.show_bind_panel = !self.show_bind_panel;
                                self.copied_feedback = false;
                            }
                        }
                    });
                });

                // Expandable Bind Extraction Panel
                if self.show_bind_panel {
                    if let Some(ref bind_sql) = candidate_bind_query {
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgb(248, 250, 252))
                            .rounding(Rounding::same(4.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("🔍 바인드 변수 후보값 추출 쿼리 (Oracle 실측값)")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(30, 41, 59)),
                                    );

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Execute in DB button
                                        let exec_btn = egui::Button::new(
                                            RichText::new("▶ DB에서 추출 실행")
                                                .size(10.5)
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(Color32::from_rgb(16, 185, 129))
                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(5, 150, 105)))
                                        .rounding(Rounding::same(3.0));

                                        if ui.add(exec_btn).clicked() {
                                            action.execute_custom_sql = Some(bind_sql.clone());
                                        }

                                        // Copy button
                                        let copy_text = if self.copied_feedback { "✓ 복사 완료!" } else { "📋 쿼리 복사" };
                                        let copy_btn = egui::Button::new(
                                            RichText::new(copy_text)
                                                .size(10.5)
                                                .color(Color32::from_rgb(51, 65, 85)),
                                        )
                                        .fill(Color32::WHITE)
                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                                        .rounding(Rounding::same(3.0));

                                        if ui.add(copy_btn).clicked() {
                                            ui.output_mut(|o| o.copied_text = bind_sql.clone());
                                            self.copied_feedback = true;
                                        }
                                    });
                                });

                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(bind_sql)
                                        .size(11.5)
                                        .monospace()
                                        .color(Color32::from_rgb(51, 65, 85)),
                                );
                            });
                    }
                }

                ui.add_space(6.0);

                let text_edit = egui::TextEdit::multiline(&mut self.sql)
                    .font(FontId::monospace(13.0))
                    .desired_width(f32::INFINITY)
                    .desired_rows(8)
                    .frame(true);

                egui::Frame::none()
                    .fill(Color32::from_rgb(250, 250, 252))
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.add(text_edit);
                    });
            });

        action
    }
}

use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke};
use crate::db::tns::TnsManager;

pub struct TnsEditorView {
    pub content: String,
    pub status_message: Option<String>,
}

impl TnsEditorView {
    pub fn new(manager: &TnsManager) -> Self {
        Self {
            content: manager.raw_content.clone(),
            status_message: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, manager: &mut TnsManager) -> bool {
        let mut close_requested = false;

        ui.vertical(|ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("tnsnames.ora 편집기")
                        .size(14.0)
                        .strong()
                        .color(Color32::from_rgb(24, 24, 27)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("닫기 ✕").size(11.0).color(Color32::from_rgb(113, 113, 122))).clicked() {
                        close_requested = true;
                    }

                    if ui.button(RichText::new("원래대로").size(11.0)).clicked() {
                        self.content = manager.raw_content.clone();
                        self.status_message = Some("파일 내용을 다시 불러왔습니다.".to_string());
                    }

                    // Save Button
                    let save_btn = egui::Button::new(
                        RichText::new("저장 및 적용")
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(24, 24, 27))
                    .rounding(Rounding::same(4.0));

                    if ui.add(save_btn).clicked() {
                        match manager.save(&self.content) {
                            Ok(_) => {
                                self.status_message = Some(format!("저장 완료! (총 {}개 서버 인식)", manager.entries.len()));
                            }
                            Err(e) => {
                                self.status_message = Some(e);
                            }
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("파일 경로: {}", manager.file_path.display()))
                    .size(10.0)
                    .color(Color32::from_rgb(113, 113, 122)),
            );

            if let Some(msg) = &self.status_message {
                ui.add_space(2.0);
                ui.label(RichText::new(msg).size(11.0).color(Color32::from_rgb(34, 197, 94)));
            }

            ui.add_space(6.0);

            // Multiline Editor Frame
            egui::Frame::none()
                .fill(Color32::from_rgb(250, 250, 252))
                .rounding(Rounding::same(6.0))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    let text_edit = egui::TextEdit::multiline(&mut self.content)
                        .font(FontId::monospace(12.0))
                        .desired_width(f32::INFINITY)
                        .desired_rows(16)
                        .code_editor();

                    ui.add(text_edit);
                });
        });

        close_requested
    }
}

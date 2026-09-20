use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use crate::models::QueryResult;

pub struct GridView;

impl GridView {
    pub fn show(ui: &mut egui::Ui, result: Option<&QueryResult>) {
        match result {
            Some(res) => {
                // Meta Stats Banner (Monochrome Light)
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("⏱ {:.2} ms", res.elapsed_ms))
                                    .color(Color32::from_rgb(24, 24, 27))
                                    .strong()
                                    .monospace()
                                    .size(12.0),
                            );
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!("총 {} 건 조회됨", res.row_count))
                                    .color(Color32::from_rgb(82, 82, 91))
                                    .monospace()
                                    .size(12.0),
                            );
                            if let Some(sql_id) = &res.sql_id {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!("SQL_ID: {}", sql_id))
                                        .color(Color32::from_rgb(82, 82, 91))
                                        .monospace()
                                        .size(12.0),
                                );
                            }
                            if let Some(hash) = res.plan_hash_value {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!("Plan Hash: {}", hash))
                                        .color(Color32::from_rgb(113, 113, 122))
                                        .monospace()
                                        .size(12.0),
                                );
                            }
                            if let Some(msg) = &res.message {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(msg)
                                        .color(Color32::from_rgb(22, 163, 74))
                                        .strong()
                                        .monospace()
                                        .size(12.0),
                                );
                            }
                        });
                    });

                ui.add_space(6.0);

                // Table Frame
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Grid::new("result_data_grid")
                                    .striped(true)
                                    .min_col_width(90.0)
                                    .spacing(Vec2::new(16.0, 8.0))
                                    .show(ui, |ui| {
                                        // Header Row
                                        ui.label(
                                            RichText::new("#")
                                                .strong()
                                                .color(Color32::from_rgb(113, 113, 122))
                                                .monospace(),
                                        );
                                        for col in &res.columns {
                                            ui.label(
                                                RichText::new(col)
                                                    .strong()
                                                    .color(Color32::from_rgb(24, 24, 27))
                                                    .monospace(),
                                            );
                                        }
                                        ui.end_row();

                                        // Data Rows
                                        for (row_idx, row) in res.rows.iter().enumerate() {
                                            ui.label(
                                                RichText::new(format!("{}", row_idx + 1))
                                                    .color(Color32::from_rgb(161, 161, 170))
                                                    .monospace(),
                                            );
                                            for cell in row {
                                                ui.label(
                                                    RichText::new(cell)
                                                        .monospace()
                                                        .color(Color32::from_rgb(24, 24, 27)),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
            }
            None => {
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(40.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("쿼리를 실행(Ctrl+Enter)하면 여기에 결과 데이터가 출력됩니다.")
                                    .color(Color32::from_rgb(113, 113, 122))
                                    .size(13.0),
                            );
                        });
                    });
            }
        }
    }
}

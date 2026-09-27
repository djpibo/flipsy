use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use crate::models::QueryResult;

#[derive(Debug, Clone)]
pub struct GridView {
    pub current_page: usize,
    pub page_size: usize,
}

impl Default for GridView {
    fn default() -> Self {
        Self {
            current_page: 1,
            page_size: 100,
        }
    }
}

impl GridView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self, ui: &mut egui::Ui, result: Option<&QueryResult>) {
        match result {
            Some(res) => {
                let total_rows = res.rows.len();
                let total_pages = if total_rows == 0 {
                    1
                } else {
                    ((total_rows as f64) / (self.page_size as f64)).ceil() as usize
                };

                if self.current_page > total_pages {
                    self.current_page = total_pages;
                }
                if self.current_page < 1 {
                    self.current_page = 1;
                }

                let start_idx = if total_rows == 0 { 0 } else { (self.current_page - 1) * self.page_size };
                let end_idx = (start_idx + self.page_size).min(total_rows);

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
                                RichText::new(format!("총 {} 건 조회됨", total_rows))
                                    .color(Color32::from_rgb(82, 82, 91))
                                    .monospace()
                                    .size(12.0),
                            );

                            if total_rows > 0 {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "표시: {} - {} 건 (Page {}/{})",
                                        start_idx + 1,
                                        end_idx,
                                        self.current_page,
                                        total_pages
                                    ))
                                    .color(Color32::from_rgb(39, 39, 42))
                                    .strong()
                                    .monospace()
                                    .size(12.0),
                                );
                            }

                            if let Some(sql_id) = &res.sql_id {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!("SQL_ID: {}", sql_id))
                                        .color(Color32::from_rgb(79, 70, 229))
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

                            // Pagination Controls (Right-Aligned) - Always visible when rows exist
                            if total_rows > 0 {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let is_last = self.current_page >= total_pages;
                                    let is_first = self.current_page <= 1;

                                    if ui.add_enabled(!is_last, egui::Button::new(RichText::new("Last >|").size(11.0))).clicked() {
                                        self.current_page = total_pages;
                                    }
                                    if ui.add_enabled(!is_last, egui::Button::new(RichText::new("Next ▶").size(11.0))).clicked() {
                                        self.current_page = (self.current_page + 1).min(total_pages);
                                    }

                                    ui.label(
                                        RichText::new(format!(" {} / {} ", self.current_page, total_pages))
                                            .strong()
                                            .monospace()
                                            .color(Color32::from_rgb(24, 24, 27)),
                                    );

                                    if ui.add_enabled(!is_first, egui::Button::new(RichText::new("◀ Prev").size(11.0))).clicked() {
                                        self.current_page = self.current_page.saturating_sub(1).max(1);
                                    }
                                    if ui.add_enabled(!is_first, egui::Button::new(RichText::new("|< First").size(11.0))).clicked() {
                                        self.current_page = 1;
                                    }

                                    ui.add_space(14.0);

                                    // Page size toggle buttons: includes smaller sizes [500, 200, 100, 50, 20, 10]
                                    for &size in &[500, 200, 100, 50, 20, 10] {
                                        let selected = self.page_size == size;
                                        let text_col = if selected { Color32::WHITE } else { Color32::from_rgb(82, 82, 91) };
                                        let btn = egui::Button::new(RichText::new(format!("{}", size)).size(10.0).color(text_col))
                                            .fill(if selected { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(244, 244, 245) })
                                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216)));
                                        if ui.add(btn).clicked() {
                                            self.page_size = size;
                                            self.current_page = 1;
                                        }
                                    }

                                    ui.label(RichText::new("Page Size:").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                });
                            }
                        });
                    });

                ui.add_space(6.0);

                // Table Frame
                if total_rows == 0 {
                    egui::Frame::none()
                        .fill(Color32::WHITE)
                        .rounding(Rounding::same(6.0))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                        .inner_margin(egui::Margin::same(30.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("조회 결과가 없습니다 (0건 인출).")
                                        .color(Color32::from_rgb(113, 113, 122))
                                        .size(12.5),
                                );
                            });
                        });
                } else {
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

                                        // Sliced Data Rows for Current Page
                                        for (local_idx, row) in res.rows[start_idx..end_idx].iter().enumerate() {
                                            let global_idx = start_idx + local_idx + 1;
                                            ui.label(
                                                RichText::new(format!("{}", global_idx))
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

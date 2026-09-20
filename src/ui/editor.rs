use eframe::egui::{self, Color32, FontId, Key, RichText, Rounding, Stroke};

pub struct EditorView {
    pub sql: String,
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
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let mut run_clicked = false;
        let mut explain_clicked = false;

        ui.input(|i| {
            if i.modifiers.command && i.key_pressed(Key::Enter) {
                run_clicked = true;
            }
            if i.key_pressed(Key::F10) {
                explain_clicked = true;
            }
        });

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
                        RichText::new("(단축키: Ctrl+Enter 실행 | F10 XPlan 실행계획)")
                            .size(11.0)
                            .color(Color32::from_rgb(113, 113, 122)),
                    );
                });

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

        (run_clicked, explain_clicked)
    }
}

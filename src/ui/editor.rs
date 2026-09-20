use eframe::egui::{self, Color32, FontId, Key, RichText, Rounding, Stroke, Vec2};
use crate::db::session::{analyze_query_structure, format_sql, generate_bind_extraction_query};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarMode {
    None,
    Figure,
    Bind,
}

pub struct EditorAction {
    pub run_requested: bool,
    pub explain_requested: bool,
    pub execute_custom_sql: Option<String>,
}

pub struct EditorView {
    pub sql: String,
    pub active_sidebar: SidebarMode,
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
            active_sidebar: SidebarMode::None,
            copied_feedback: false,
        }
    }

    pub fn format_lines(&mut self) {
        self.sql = format_sql(&self.sql);
    }

    pub fn toggle_figure(&mut self) {
        if self.active_sidebar == SidebarMode::Figure {
            self.active_sidebar = SidebarMode::None;
        } else {
            self.active_sidebar = SidebarMode::Figure;
        }
    }

    pub fn toggle_bind(&mut self) {
        if self.active_sidebar == SidebarMode::Bind {
            self.active_sidebar = SidebarMode::None;
            self.copied_feedback = false;
        } else {
            self.active_sidebar = SidebarMode::Bind;
            self.copied_feedback = false;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorAction {
        let mut action = EditorAction {
            run_requested: false,
            explain_requested: false,
            execute_custom_sql: None,
        };

        ui.input(|i| {
            if i.modifiers.command && i.key_pressed(Key::Enter) {
                action.run_requested = true;
            }
            if i.key_pressed(Key::F10) {
                action.explain_requested = true;
            }
            if i.key_pressed(Key::F8) {
                self.format_lines();
            }
        });

        // Outer Container Card
        egui::Frame::none()
            .fill(Color32::WHITE)
            .rounding(Rounding::same(6.0))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                // Header
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

                // Horizontal Split: Left Editor (with Line Numbers), Right Sidebar (Figure or Bind)
                ui.horizontal_top(|ui| {
                    let total_width = ui.available_width();
                    let sidebar_width = 330.0;
                    let has_sidebar = self.active_sidebar != SidebarMode::None;
                    let editor_width = if has_sidebar {
                        (total_width - sidebar_width - 10.0).max(280.0)
                    } else {
                        total_width
                    };

                    // Left Editor Area (with Line Numbers Gutter & Vertical Scrollbar)
                    ui.allocate_ui_with_layout(
                        Vec2::new(editor_width, 245.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(250, 250, 252))
                                .rounding(Rounding::same(4.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                                .inner_margin(egui::Margin::symmetric(8.0, 8.0))
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("sql_editor_scroll_v")
                                        .max_height(229.0)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            ui.horizontal_top(|ui| {
                                                // Line numbers gutter
                                                let line_count = self.sql.split('\n').count().max(1);
                                                let mut num_str = String::new();
                                                for i in 1..=line_count {
                                                    num_str.push_str(&format!(" {:>2} \n", i));
                                                }

                                                ui.vertical(|ui| {
                                                    ui.add_space(1.0);
                                                    ui.label(
                                                        RichText::new(num_str)
                                                            .font(FontId::monospace(12.5))
                                                            .color(Color32::from_rgb(161, 161, 170)),
                                                    );
                                                });

                                                // Gutter separator
                                                let sep_height = (line_count as f32 * 19.0).max(220.0);
                                                let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(1.0, sep_height), egui::Sense::hover());
                                                ui.painter().rect_filled(sep_rect, Rounding::ZERO, Color32::from_rgb(228, 228, 231));

                                                ui.add_space(6.0);

                                                // Multiline TextEdit (without border frame)
                                                let text_edit = egui::TextEdit::multiline(&mut self.sql)
                                                    .font(FontId::monospace(12.5))
                                                    .desired_width(f32::INFINITY)
                                                    .desired_rows(line_count.max(8))
                                                    .frame(false);

                                                ui.add(text_edit);
                                            });
                                        });
                                });
                        },
                    );

                    // Right Sidebar Area (with vertical scrollbar)
                    if has_sidebar {
                        ui.add_space(8.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(sidebar_width, 245.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(255, 255, 255))
                                    .rounding(Rounding::same(4.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216)))
                                    .inner_margin(egui::Margin::same(10.0))
                                    .show(ui, |ui| {
                                        egui::ScrollArea::vertical()
                                            .id_salt("sidebar_scroll_v")
                                            .max_height(225.0)
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                match self.active_sidebar {
                                                    SidebarMode::None => {}
                                                    SidebarMode::Figure => {
                                                        Self::render_figure_sidebar(ui, &self.sql, &mut self.active_sidebar);
                                                    }
                                                    SidebarMode::Bind => {
                                                        Self::render_bind_sidebar(
                                                            ui,
                                                            &self.sql,
                                                            &mut self.active_sidebar,
                                                            &mut self.copied_feedback,
                                                            &mut action.execute_custom_sql,
                                                        );
                                                    }
                                                }
                                            });
                                    });
                            },
                        );
                    }
                });
            });

        action
    }

    fn render_figure_sidebar(ui: &mut egui::Ui, sql: &str, active_sidebar: &mut SidebarMode) {
        let structure = analyze_query_structure(sql);

        // Sidebar Header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("📊 Figure (테이블 구조 분석)")
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(24, 24, 27)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("✕").size(11.0).color(Color32::from_rgb(113, 113, 122))).clicked() {
                    *active_sidebar = SidebarMode::None;
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                // CTEs
                if !structure.ctes.is_empty() {
                    ui.label(RichText::new("서브쿼리 / CTE").size(10.5).strong().color(Color32::from_rgb(113, 113, 122)));
                    ui.add_space(2.0);
                    for cte in &structure.ctes {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(244, 244, 245))
                            .rounding(Rounding::same(3.0))
                            .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(format!("WITH {}", cte)).size(11.0).monospace().strong());
                            });
                        ui.add_space(2.0);
                    }
                    ui.add_space(6.0);
                }

                // Tables & Joins
                ui.label(RichText::new("테이블 & 조인 관계").size(10.5).strong().color(Color32::from_rgb(113, 113, 122)));
                ui.add_space(4.0);

                if structure.tables.is_empty() {
                    ui.label(RichText::new("참조된 테이블이 없습니다.").size(11.0).color(Color32::from_rgb(161, 161, 170)));
                } else {
                    for (_idx, tbl) in structure.tables.iter().enumerate() {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(250, 250, 252))
                            .rounding(Rounding::same(4.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Join Type Tag
                                    let tag_color = if tbl.join_type == "FROM" {
                                        Color32::from_rgb(22, 163, 74)
                                    } else {
                                        Color32::from_rgb(2, 132, 199)
                                    };
                                    egui::Frame::none()
                                        .fill(Color32::from_rgb(244, 244, 245))
                                        .rounding(Rounding::same(3.0))
                                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                                        .show(ui, |ui| {
                                            ui.label(RichText::new(&tbl.join_type).size(9.5).strong().color(tag_color));
                                        });

                                    // Table Name
                                    ui.label(RichText::new(&tbl.name).size(11.5).strong().color(Color32::from_rgb(24, 24, 27)));

                                    // Alias
                                    if let Some(alias) = &tbl.alias {
                                        ui.label(RichText::new(format!("({})", alias)).size(10.5).monospace().color(Color32::from_rgb(113, 113, 122)));
                                    }
                                });

                                // Join condition
                                if let Some(cond) = &tbl.join_condition {
                                    ui.add_space(2.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("└── ON").size(9.5).color(Color32::from_rgb(161, 161, 170)));
                                        ui.label(RichText::new(cond).size(10.5).monospace().color(Color32::from_rgb(51, 65, 85)));
                                    });
                                }
                            });
                        ui.add_space(4.0);
                    }
                }

                // Where Filters
                if !structure.filters.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("WHERE 필터 조건").size(10.5).strong().color(Color32::from_rgb(113, 113, 122)));
                    ui.add_space(4.0);
                    for filter in &structure.filters {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(254, 252, 232))
                            .rounding(Rounding::same(3.0))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(254, 240, 138)))
                            .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(filter).size(10.5).monospace().color(Color32::from_rgb(113, 63, 18)));
                            });
                        ui.add_space(3.0);
                    }
                }
            });
    }

    fn render_bind_sidebar(
        ui: &mut egui::Ui,
        sql: &str,
        active_sidebar: &mut SidebarMode,
        copied_feedback: &mut bool,
        execute_custom_sql: &mut Option<String>,
    ) {
        let candidate = generate_bind_extraction_query(sql);

        // Sidebar Header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("🔍 Bind (바인드 변수 추출)")
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(24, 24, 27)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("✕").size(11.0).color(Color32::from_rgb(113, 113, 122))).clicked() {
                    *active_sidebar = SidebarMode::None;
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        match candidate {
            Some(bind_query) => {
                ui.label(
                    RichText::new("바인드 변수가 감지되었습니다.\n실제 Oracle에서 샘플값을 추출하는 SELECT 쿼리입니다.")
                        .size(11.0)
                        .color(Color32::from_rgb(71, 85, 105)),
                );

                ui.add_space(6.0);

                // Extraction SQL Box
                egui::Frame::none()
                    .fill(Color32::from_rgb(248, 250, 252))
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&bind_query)
                                .size(11.0)
                                .monospace()
                                .color(Color32::from_rgb(30, 41, 59)),
                        );
                    });

                ui.add_space(8.0);

                // Action Buttons: Copy and Run
                ui.horizontal(|ui| {
                    let copy_btn_text = if *copied_feedback { "✓ 복사 완료!" } else { "📋 쿼리 복사" };
                    let copy_btn = egui::Button::new(
                        RichText::new(copy_btn_text).size(11.0).color(Color32::from_rgb(24, 24, 27)),
                    )
                    .fill(Color32::from_rgb(244, 244, 245))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .rounding(Rounding::same(4.0));

                    if ui.add(copy_btn).clicked() {
                        ui.output_mut(|o| o.copied_text = bind_query.clone());
                        *copied_feedback = true;
                    }

                    let exec_btn = egui::Button::new(
                        RichText::new("▶ DB에서 추출")
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(16, 185, 129))
                    .rounding(Rounding::same(4.0));

                    if ui.add(exec_btn).clicked() {
                        *execute_custom_sql = Some(bind_query.clone());
                    }
                });
            }
            None => {
                ui.label(
                    RichText::new("SQL에 바인드 변수(:var_name)가\n감지되지 않았습니다.")
                        .size(11.5)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            }
        }
    }
}

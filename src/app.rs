use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use crate::db::session::{AsyncExecutionHandle, DatabaseSession};
use std::sync::atomic::Ordering;
use crate::ui::editor::{EditorView, SidebarMode};
use crate::ui::grid::GridView;
use crate::ui::plan_tree::PlanTreeView;
use crate::ui::server_list::ServerListView;

pub enum AppState {
    ServerList(ServerListView),
    Workspace {
        session: DatabaseSession,
        editor: EditorView,
        active_bottom_tab: BottomTab,
    },
}

#[derive(PartialEq)]
pub enum BottomTab {
    Grid,
    PlanTree,
}

pub struct FlipsyApp {
    pub state: AppState,
    pub active_query: Option<AsyncExecutionHandle>,
}

fn setup_app_style_and_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_paths = [
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\malgunbd.ttf",
        r"C:\Windows\Fonts\gulim.ttc",
        r"C:\Windows\Fonts\batang.ttc",
    ];

    for path in font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "korean_font".to_owned(),
                egui::FontData::from_owned(data),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "korean_font".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "korean_font".to_owned());

            break;
        }
    }

    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::light();
    visuals.window_fill = Color32::from_rgb(248, 249, 250);
    visuals.panel_fill = Color32::from_rgb(248, 249, 250);
    visuals.widgets.noninteractive.bg_fill = Color32::WHITE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231));
    visuals.widgets.inactive.bg_fill = Color32::WHITE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(244, 244, 245);
    visuals.widgets.active.bg_fill = Color32::from_rgb(228, 228, 231);
    ctx.set_visuals(visuals);
}

impl FlipsyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_app_style_and_fonts(&cc.egui_ctx);
        Self {
            state: AppState::ServerList(ServerListView::new()),
            active_query: None,
        }
    }
}

impl eframe::App for FlipsyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 0. Poll background async query completion
        if let Some(handle) = &self.active_query {
            ctx.request_repaint();
            if let Ok(res) = handle.rx.try_recv() {
                let is_explain = handle.is_explain;
                if let AppState::Workspace { session, active_bottom_tab, .. } = &mut self.state {
                    session.last_query_result = Some(res.query_result);
                    if let Some(nodes) = res.plan_nodes {
                        session.last_plan = Some(nodes);
                    }
                    session.last_plan_hash = res.plan_hash;
                    session.last_sql_id = res.sql_id;

                    if is_explain {
                        *active_bottom_tab = BottomTab::PlanTree;
                    } else {
                        *active_bottom_tab = BottomTab::Grid;
                    }
                }
                self.active_query = None;
            }
        }

        match &mut self.state {
            AppState::ServerList(server_view) => {
                if let Some(config) = server_view.show(ctx) {
                    let mut session = DatabaseSession::new();
                    if let Err(e) = session.connect(config) {
                        server_view.error_message = Some(e);
                    } else {
                        server_view.login_target = None;
                        server_view.error_message = None;
                        // Expand window to full workspace (1280 x 820)
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1280.0, 820.0)));

                        let mut editor = EditorView::new();
                        editor.sql = "SELECT banner FROM v$version;".to_string();

                        self.state = AppState::Workspace {
                            session,
                            editor,
                            active_bottom_tab: BottomTab::Grid,
                            };
                    }
                }
            }
            AppState::Workspace {
                session,
                editor,
                active_bottom_tab,
            } => {
                let mut return_to_servers = false;
                let mut run_requested = false;
                let mut explain_requested = false;

                // Minimalist Light Monochrome Header
                egui::TopBottomPanel::top("top_panel")
                    .frame(
                        egui::Frame::none()
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .inner_margin(egui::Margin::symmetric(14.0, 9.0)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            // Back to Server List Button (KakaoTalk Friends Return)
                            let back_btn = egui::Button::new(
                                RichText::new("◀ 서버 목록").size(11.0).color(Color32::from_rgb(24, 24, 27)),
                            )
                            .fill(Color32::from_rgb(244, 244, 245))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .rounding(Rounding::same(4.0));

                            if ui.add(back_btn).clicked() {
                                return_to_servers = true;
                            }

                            ui.add_space(8.0);

                            // Connection Badge with Live Indicator
                            egui::Frame::none()
                                .fill(Color32::from_rgb(244, 244, 245))
                                .rounding(Rounding::same(4.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                                .inner_margin(egui::Margin::symmetric(9.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (status_text, status_col) = if session.is_real_oracle {
                                            ("● LIVE CONNECTED", Color32::from_rgb(22, 163, 74))
                                        } else {
                                            ("○ SIMULATED", Color32::from_rgb(113, 113, 122))
                                        };

                                        ui.label(RichText::new(status_text).size(10.5).strong().color(status_col));
                                        ui.label(
                                            RichText::new(format!(
                                                "{} @ {} ({}:{})",
                                                session.config.username,
                                                session.config.service_name,
                                                session.config.host,
                                                session.config.port
                                            ))
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(24, 24, 27)),
                                        );

                                        if session.is_real_oracle {
                                            ui.label(
                                                RichText::new("· Oracle 26ai Free (23.26.3)")
                                                    .size(10.5)
                                                    .strong()
                                                    .color(Color32::from_rgb(79, 70, 229)),
                                            );
                                        }
                                    });
                                });

                            // Right Action Buttons: Line-up, Figure, Bind, Run (Unified Monochrome Palette)
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // 1. Run (Primary Solid Action Button with Live State)
                                let is_running = self.active_query.is_some();
                                let run_text = if is_running {
                                    RichText::new("⏳ 실행 중...").size(11.5).strong().color(Color32::from_rgb(250, 204, 21))
                                } else {
                                    RichText::new("▶ Run (Ctrl+Enter)").size(11.5).strong().color(Color32::WHITE)
                                };

                                let run_btn = egui::Button::new(run_text)
                                    .fill(if is_running { Color32::from_rgb(39, 39, 42) } else { Color32::from_rgb(24, 24, 27) })
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 27)))
                                    .rounding(Rounding::same(4.0))
                                    .min_size(Vec2::new(125.0, 28.0));

                                if ui.add_enabled(!is_running, run_btn).clicked() {
                                    run_requested = true;
                                }

                                ui.add_space(5.0);

                                // 2. Bind
                                let is_bind = editor.active_sidebar == SidebarMode::Bind;
                                let bind_btn = egui::Button::new(
                                    RichText::new("Bind (바인드)")
                                        .size(11.0)
                                        .strong()
                                        .color(if is_bind { Color32::WHITE } else { Color32::from_rgb(39, 39, 42) }),
                                )
                                .fill(if is_bind { Color32::from_rgb(39, 39, 42) } else { Color32::WHITE })
                                .stroke(Stroke::new(1.0_f32, if is_bind { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(212, 212, 216) }))
                                .rounding(Rounding::same(4.0))
                                .min_size(Vec2::new(88.0, 28.0));

                                if ui.add(bind_btn).clicked() {
                                    editor.toggle_bind();
                                }

                                ui.add_space(5.0);

                                // 3. Figure
                                let is_fig = editor.active_sidebar == SidebarMode::Figure;
                                let fig_btn = egui::Button::new(
                                    RichText::new("Figure (구조)")
                                        .size(11.0)
                                        .strong()
                                        .color(if is_fig { Color32::WHITE } else { Color32::from_rgb(39, 39, 42) }),
                                )
                                .fill(if is_fig { Color32::from_rgb(39, 39, 42) } else { Color32::WHITE })
                                .stroke(Stroke::new(1.0_f32, if is_fig { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(212, 212, 216) }))
                                .rounding(Rounding::same(4.0))
                                .min_size(Vec2::new(88.0, 28.0));

                                if ui.add(fig_btn).clicked() {
                                    editor.toggle_figure();
                                }

                                ui.add_space(5.0);

                                // 4. Line-up (Leftmost)
                                let fmt_btn = egui::Button::new(
                                    RichText::new("Line-up (F8)")
                                        .size(11.0)
                                        .color(Color32::from_rgb(39, 39, 42)),
                                )
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(212, 212, 216)))
                                .rounding(Rounding::same(4.0))
                                .min_size(Vec2::new(88.0, 28.0));

                                if ui.add(fmt_btn).clicked() {
                                    editor.format_lines();
                                }
                            });
                        });
                    });

                // Central Workspace
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(Color32::from_rgb(248, 249, 250)).inner_margin(egui::Margin::same(12.0)))
                    .show(ctx, |ui| {
                        let action = editor.show(ui);
                        if action.run_requested {
                            run_requested = true;
                        }
                        if action.explain_requested {
                            explain_requested = true;
                        }
                        if let Some(custom_sql) = action.execute_custom_sql {
                            if self.active_query.is_none() {
                                self.active_query = Some(session.execute_async(&custom_sql, false));
                            }
                        }

                        // Live Query Execution Progress Bar
                        if let Some(handle) = &self.active_query {
                            let elapsed = handle.start_time.elapsed().as_secs_f32();
                            let bytes = handle.tracker.bytes_read.load(Ordering::Relaxed);
                            let rows = handle.tracker.rows_read.load(Ordering::Relaxed);
                            let mb = bytes as f32 / (1024.0 * 1024.0);

                            let size_str = if mb >= 1.0 {
                                format!("{:.2} MB", mb)
                            } else {
                                format!("{:.1} KB", bytes as f32 / 1024.0)
                            };

                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgb(24, 24, 27))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.add_space(6.0);
                                        let title = if handle.is_explain {
                                            "XPlan 실행계획 분석 중..."
                                        } else {
                                            "쿼리 실행 및 데이터 인출 중..."
                                        };
                                        ui.label(
                                            RichText::new(title)
                                                .size(12.5)
                                                .strong()
                                                .color(Color32::WHITE),
                                        );
                                        ui.add_space(10.0);
                                        ui.label(
                                            RichText::new(format!("⏱ 경과 시간: {:.2}s", elapsed))
                                                .size(12.0)
                                                .monospace()
                                                .color(Color32::from_rgb(250, 204, 21)),
                                        );
                                        ui.add_space(10.0);
                                        if !handle.is_explain {
                                            ui.label(
                                                RichText::new(format!("📦 인출 현황: {}행 ({})", rows, size_str))
                                                    .size(12.0)
                                                    .monospace()
                                                    .color(Color32::from_rgb(56, 189, 248)),
                                            );
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let cancel_btn = egui::Button::new(
                                                RichText::new("✕ 중단 (Cancel)").size(11.0).strong().color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(220, 38, 38))
                                            .rounding(Rounding::same(4.0));

                                            if ui.add(cancel_btn).clicked() {
                                                handle.tracker.is_cancelled.store(true, Ordering::Relaxed);
                                            }
                                        });
                                    });

                                    ui.add_space(6.0);
                                    let progress = ((elapsed * 1.5) % 1.0).max(0.08);
                                    let pbar = egui::ProgressBar::new(progress)
                                        
                                        .fill(Color32::from_rgb(59, 130, 246));
                                    ui.add(pbar);
                                });
                        }

                        ui.add_space(10.0);

                        // Tabs
                        ui.horizontal(|ui| {
                            let grid_label = format!(
                                "Result Grid {}",
                                session.last_query_result.as_ref().map(|r| format!("({})", r.row_count)).unwrap_or_default()
                            );

                            let is_grid = *active_bottom_tab == BottomTab::Grid;
                            let grid_btn = egui::Button::new(
                                RichText::new(grid_label)
                                    .size(12.0)
                                    .strong()
                                    .color(if is_grid { Color32::WHITE } else { Color32::from_rgb(82, 82, 91) }),
                            )
                            .fill(if is_grid { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(244, 244, 245) })
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .rounding(Rounding::same(4.0));

                            if ui.add(grid_btn).clicked() {
                                *active_bottom_tab = BottomTab::Grid;
                            }

                            ui.add_space(4.0);

                            let is_plan = *active_bottom_tab == BottomTab::PlanTree;
                            let plan_btn = egui::Button::new(
                                RichText::new("실행계획 트리 (XPlan)")
                                    .size(12.0)
                                    .strong()
                                    .color(if is_plan { Color32::WHITE } else { Color32::from_rgb(82, 82, 91) }),
                            )
                            .fill(if is_plan { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(244, 244, 245) })
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                            .rounding(Rounding::same(4.0));

                            if ui.add(plan_btn).clicked() {
                                *active_bottom_tab = BottomTab::PlanTree;
                                if session.last_plan.is_none() && self.active_query.is_none() {
                                    let binds = crate::db::session::extract_bind_variables(&editor.sql);
                                    let sql_to_explain = if !binds.is_empty() {
                                        crate::db::session::substitute_bind_variables(&editor.sql, &editor.bind_values)
                                    } else {
                                        editor.sql.clone()
                                    };
                                    self.active_query = Some(session.execute_async(&sql_to_explain, true));
                                }
                            }
                        });

                        ui.add_space(8.0);

                        match active_bottom_tab {
                            BottomTab::Grid => {
                                GridView::show(ui, session.last_query_result.as_ref());
                            }
                            BottomTab::PlanTree => {
                                PlanTreeView::show(ui, session.last_plan.as_ref(), session.last_sql_id.as_deref(), session.last_plan_hash);
                            }
                        }
                    });

                if run_requested && self.active_query.is_none() {
                    let binds = crate::db::session::extract_bind_variables(&editor.sql);
                    let sql_to_run = if !binds.is_empty() {
                        crate::db::session::substitute_bind_variables(&editor.sql, &editor.bind_values)
                    } else {
                        editor.sql.clone()
                    };
                    self.active_query = Some(session.execute_async(&sql_to_run, false));
                }

                if explain_requested && self.active_query.is_none() {
                    let binds = crate::db::session::extract_bind_variables(&editor.sql);
                    let sql_to_explain = if !binds.is_empty() {
                        crate::db::session::substitute_bind_variables(&editor.sql, &editor.bind_values)
                    } else {
                        editor.sql.clone()
                    };
                    self.active_query = Some(session.execute_async(&sql_to_explain, true));
                }

                if return_to_servers {
                    session.disconnect();
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(360.0, 580.0)));
                    self.state = AppState::ServerList(ServerListView::new());
                }
            }
        }
    }
}

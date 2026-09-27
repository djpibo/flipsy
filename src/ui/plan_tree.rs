use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use crate::models::PlanNode;

pub struct PlanTreeView;

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        let rem = len - 1 - i;
        if rem > 0 && rem.is_multiple_of(3) {
            out.push(',');
        }
    }
    out
}

impl PlanTreeView {
    pub fn show(
        ui: &mut egui::Ui,
        plan: Option<&Vec<PlanNode>>,
        sql_id: Option<&str>,
        plan_hash: Option<u64>,
        is_runtime: bool,
    ) {
        match plan {
            Some(nodes) if !nodes.is_empty() => {
                let root = &nodes[0];
                let total_buffers = root.buffers;
                let total_reads = root.reads;
                let total_time = root.a_time_ms;
                let total_cost = root.cost.or_else(|| nodes.iter().filter_map(|n| n.cost).max());

                // 1. KPI Summary Header (Light Minimalist Toolbar)
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Method Subtitle Badge
                            let (badge_title, badge_color, badge_bg) = if is_runtime {
                                (
                                    "2번 방식: DBMS.XPLAN (ALLSTATS LAST 런타임 실측치)",
                                    Color32::from_rgb(22, 163, 74),
                                    Color32::from_rgb(240, 253, 244),
                                )
                            } else {
                                (
                                    "1번 방식: EXPLAIN PLAN FOR (옵티마이저 예측 계획)",
                                    Color32::from_rgb(37, 99, 235),
                                    Color32::from_rgb(239, 246, 255),
                                )
                            };

                            egui::Frame::none()
                                .fill(badge_bg)
                                .rounding(Rounding::same(3.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                                .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(badge_title).size(10.5).strong().color(badge_color));
                                });

                            ui.add_space(16.0);

                            // Metrics
                            if is_runtime {
                                ui.label(RichText::new("A-Time:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                                ui.label(RichText::new(format!("{:.2} ms", total_time)).size(12.0).strong().monospace().color(Color32::from_rgb(15, 23, 42)));
                                ui.add_space(12.0);

                                ui.label(RichText::new("Buffers:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                                ui.label(RichText::new(format_num(total_buffers)).size(12.0).strong().monospace().color(Color32::from_rgb(15, 23, 42)));
                                ui.add_space(12.0);

                                ui.label(RichText::new("Reads:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                                ui.label(RichText::new(format_num(total_reads)).size(12.0).strong().monospace().color(Color32::from_rgb(15, 23, 42)));
                                ui.add_space(12.0);
                            }

                            let cost_str = total_cost.map(|c| format_num(c.max(0) as u64)).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new("Cost:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(RichText::new(cost_str).size(12.0).strong().monospace().color(Color32::from_rgb(15, 23, 42)));
                            ui.add_space(12.0);

                            ui.label(RichText::new("SQL_ID:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(RichText::new(sql_id.unwrap_or("-")).size(12.0).strong().monospace().color(Color32::from_rgb(79, 70, 229)));
                            ui.add_space(12.0);

                            ui.label(RichText::new("Plan Hash:").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                            let p_hash = plan_hash.map(|h| h.to_string()).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(p_hash).size(12.0).strong().monospace().color(Color32::from_rgb(15, 23, 42)));
                        });
                    });

                ui.add_space(6.0);

                // 2. Single Pane ("한 판") Execution Tree View
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (idx, node) in nodes.iter().enumerate() {
                                    Self::render_unified_row(ui, node, node.depth, is_runtime, idx % 2 == 0);
                                }
                            });
                    });
            }
            _ => {
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(40.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            if is_runtime {
                                ui.label(
                                    RichText::new("2번 방식: DBMS.XPLAN (실행 통계) 데이터가 없습니다.")
                                        .color(Color32::from_rgb(24, 24, 27))
                                        .strong()
                                        .size(13.5),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("상단의 [▶ Run (Ctrl+Enter)] 버튼을 누르면 쿼리 실행 후 실제 ALLSTATS LAST 런타임 통계가 수집됩니다.")
                                        .color(Color32::from_rgb(113, 113, 122))
                                        .size(11.5),
                                );
                            } else {
                                ui.label(
                                    RichText::new("1번 방식: EXPLAIN PLAN FOR (예측 계획) 데이터가 없습니다.")
                                        .color(Color32::from_rgb(24, 24, 27))
                                        .strong()
                                        .size(13.5),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("상단의 [⚡ Explain (F10)] 버튼 또는 F10 키를 누르면 쿼리를 직접 실행하지 않고 옵티마이저 예측 경로를 생성합니다.")
                                        .color(Color32::from_rgb(113, 113, 122))
                                        .size(11.5),
                                );
                            }
                        });
                    });
            }
        }
    }

    fn render_unified_row(
        ui: &mut egui::Ui,
        node: &PlanNode,
        depth: usize,
        is_runtime: bool,
        is_even: bool,
    ) {
        let row_bg = if is_even {
            Color32::from_rgb(253, 253, 254)
        } else {
            Color32::WHITE
        };

        egui::Frame::none()
            .fill(row_bg)
            .inner_margin(egui::Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Line 1: Operation Header Line (Id, Indent, Op, Object, Alias/QBlock, Cost)
                    ui.horizontal(|ui| {
                        // Id column (fixed width)
                        ui.allocate_ui_with_layout(
                            Vec2::new(32.0, 18.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(format!("{:>2}", node.id))
                                        .size(11.5)
                                        .strong()
                                        .monospace()
                                        .color(Color32::from_rgb(100, 116, 139)),
                                );
                            },
                        );

                        // Tree Indentation
                        if depth > 0 {
                            ui.add_space((depth as f32) * 16.0);
                            ui.label(
                                RichText::new("└─")
                                    .monospace()
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                        }

                        // Operation Name & Options
                        ui.label(
                            RichText::new(format!("{} {}", node.operation, node.options.as_deref().unwrap_or("")))
                                .size(12.0)
                                .strong()
                                .color(Color32::from_rgb(15, 23, 42)),
                        );

                        // Object Name (Table / Index in Blue)
                        if let Some(obj) = &node.object_name {
                            ui.label(
                                RichText::new(format!("({})", obj))
                                    .size(11.0)
                                    .strong()
                                    .monospace()
                                    .color(Color32::from_rgb(37, 99, 235)),
                            );
                        }

                        // Object Alias & Query Block Name
                        match (&node.object_alias, &node.qblock_name) {
                            (Some(alias), Some(qb)) => {
                                ui.label(
                                    RichText::new(format!("{} (@{})", alias, qb))
                                        .size(10.5)
                                        .strong()
                                        .monospace()
                                        .color(Color32::from_rgb(109, 40, 217)),
                                );
                            }
                            (Some(alias), None) => {
                                ui.label(
                                    RichText::new(alias)
                                        .size(10.5)
                                        .strong()
                                        .monospace()
                                        .color(Color32::from_rgb(109, 40, 217)),
                                );
                            }
                            (None, Some(qb)) => {
                                ui.label(
                                    RichText::new(format!("(@{})", qb))
                                        .size(10.5)
                                        .monospace()
                                        .color(Color32::from_rgb(124, 58, 237)),
                                );
                            }
                            (None, None) => {}
                        }

                        // Cost directly on Line 1 next to Operation/Object/Alias (NOT pushed to far right)
                        if let Some(c) = node.cost {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!("Cost: {}", format_num(c.max(0) as u64)))
                                    .size(10.5)
                                    .monospace()
                                    .color(Color32::from_rgb(100, 116, 139)),
                            );
                        }
                    });

                    // Line 2: Details Line (Starts, Buffers, A-Rows, A-Time + Filter, Access, Outline)
                    let detail_indent = 32.0 + (if depth > 0 { (depth as f32) * 16.0 + 18.0 } else { 0.0 });
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(detail_indent);

                        if is_runtime {
                            // Starts
                            ui.label(RichText::new("Starts:").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(
                                RichText::new(format_num(node.starts))
                                    .size(10.5)
                                    .strong()
                                    .monospace()
                                    .color(Color32::from_rgb(30, 41, 59)),
                            );

                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));

                            // Buffers
                            ui.label(RichText::new("Buffers:").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(
                                RichText::new(format_num(node.buffers))
                                    .size(10.5)
                                    .strong()
                                    .monospace()
                                    .color(Color32::from_rgb(30, 41, 59)),
                            );

                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));

                            // A-Rows
                            ui.label(RichText::new("A-Rows:").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(
                                RichText::new(format_num(node.a_rows))
                                    .size(10.5)
                                    .strong()
                                    .monospace()
                                    .color(Color32::from_rgb(30, 41, 59)),
                            );

                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));

                            // A-Time
                            ui.label(RichText::new("A-Time:").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(
                                RichText::new(format!("{:.2}ms", node.a_time_ms))
                                    .size(10.5)
                                    .monospace()
                                    .color(Color32::from_rgb(30, 41, 59)),
                            );
                        } else {
                            // 1번 방식 (EXPLAIN PLAN FOR)
                            ui.label(RichText::new("Rows(Est):").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                            ui.label(
                                RichText::new(format_num(node.e_rows))
                                    .size(10.5)
                                    .strong()
                                    .monospace()
                                    .color(Color32::from_rgb(30, 41, 59)),
                            );

                            if let Some(c) = node.cost {
                                ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));
                                ui.label(RichText::new("Cost:").size(10.5).color(Color32::from_rgb(100, 116, 139)));
                                ui.label(
                                    RichText::new(format_num(c.max(0) as u64))
                                        .size(10.5)
                                        .monospace()
                                        .color(Color32::from_rgb(30, 41, 59)),
                                );
                            }
                        }

                        // Access Predicate (without double quotes)
                        if let Some(access) = &node.access_predicates {
                            let clean = access.replace('"', "");
                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));
                            ui.label(RichText::new("Access:").size(10.5).strong().color(Color32::from_rgb(8, 145, 178)));
                            ui.label(RichText::new(clean).size(10.5).monospace().color(Color32::from_rgb(15, 23, 42)));
                        }

                        // Filter Predicate (without double quotes)
                        if let Some(filter) = &node.filter_predicates {
                            let clean = filter.replace('"', "");
                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));
                            ui.label(RichText::new("Filter:").size(10.5).strong().color(Color32::from_rgb(180, 83, 9)));
                            ui.label(RichText::new(clean).size(10.5).monospace().color(Color32::from_rgb(15, 23, 42)));
                        }

                        // Outline Hints (without double quotes)
                        if !node.outline_hints.is_empty() {
                            let clean_hints: Vec<String> = node.outline_hints.iter().map(|h| h.replace('"', "")).collect();
                            ui.label(RichText::new("|").size(10.5).color(Color32::from_rgb(203, 213, 225)));
                            ui.label(RichText::new("Outline:").size(10.5).strong().color(Color32::from_rgb(126, 34, 206)));
                            ui.label(RichText::new(clean_hints.join(" ")).size(10.0).monospace().color(Color32::from_rgb(88, 28, 135)));
                        }
                    });
                });
            });

        ui.add_space(2.0);
    }
}

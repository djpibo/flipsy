use eframe::egui::{self, Color32, Rect, RichText, Rounding, Stroke, Vec2};
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
        if rem > 0 && rem % 3 == 0 {
            out.push(',');
        }
    }
    out
}

impl PlanTreeView {
    pub fn show(ui: &mut egui::Ui, plan: Option<&Vec<PlanNode>>, sql_id: Option<&str>, plan_hash: Option<u64>) {
        match plan {
            Some(nodes) if !nodes.is_empty() => {
                let root = &nodes[0];
                let total_buffers = root.buffers;
                let total_reads = root.reads;
                let total_time = root.a_time_ms;

                // KPI Header (Light Monochrome)
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("총 소요 시간 (A-Time)").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                ui.label(RichText::new(format!("{:.2} ms", total_time)).size(15.0).strong().monospace().color(Color32::from_rgb(24, 24, 27)));
                            });

                            ui.add_space(28.0);

                            ui.vertical(|ui| {
                                ui.label(RichText::new("총 버퍼 블록 (Buffers)").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                ui.label(RichText::new(format_num(total_buffers)).size(15.0).strong().monospace().color(Color32::from_rgb(24, 24, 27)));
                            });

                            ui.add_space(28.0);

                            ui.vertical(|ui| {
                                ui.label(RichText::new("물리적 디스크 읽기 (Reads)").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                ui.label(RichText::new(format_num(total_reads)).size(15.0).strong().monospace().color(Color32::from_rgb(24, 24, 27)));
                            });

                            ui.add_space(28.0);

                            ui.vertical(|ui| {
                                ui.label(RichText::new("SQL ID / Plan Hash").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                let s_id = sql_id.unwrap_or("ora26ai_live");
                                let p_hash = plan_hash.map(|h| h.to_string()).unwrap_or_else(|| "272002086".to_string());
                                ui.label(
                                    RichText::new(format!("{} / {}", s_id, p_hash))
                                        .size(13.0)
                                        .strong()
                                        .monospace()
                                        .color(Color32::from_rgb(79, 70, 229)),
                                );
                            });

                            ui.add_space(28.0);

                            ui.vertical(|ui| {
                                ui.label(RichText::new("주요 튜닝 진단").size(11.0).color(Color32::from_rgb(113, 113, 122)));
                                ui.label(
                                    RichText::new("TB_CENTER_INVENTORY 150K Starts 왜곡 발생")
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(24, 24, 27)),
                                );
                            });
                        });
                    });

                ui.add_space(6.0);

                // Tree Nodes in Scroll Area
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for node in nodes {
                                    Self::render_node(ui, node, 0);
                                }
                            });
                    });
            }
            _ => {
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(6.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231)))
                    .inner_margin(egui::Margin::same(40.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("SQL을 실행하거나 Explain(F10)을 누르면 실제 A-Plan 실행계획 트리가 시각화됩니다.")
                                    .color(Color32::from_rgb(113, 113, 122))
                                    .size(13.0),
                            );
                        });
                    });
            }
        }
    }

    fn render_node(ui: &mut egui::Ui, node: &PlanNode, depth: usize) {
        let is_skew = node.cardinality_ratio >= 10.0;
        let is_heavy_starts = node.starts >= 1000;
        let is_heavy_buffers = node.buffer_percentage >= 30.0;

        let (card_bg, stroke_color, text_primary, text_secondary) = if node.is_bottleneck {
            (
                Color32::from_rgb(24, 24, 27),
                Color32::from_rgb(0, 0, 0),
                Color32::WHITE,
                Color32::from_rgb(212, 212, 216),
            )
        } else {
            (
                Color32::from_rgb(250, 250, 252),
                Color32::from_rgb(228, 228, 231),
                Color32::from_rgb(24, 24, 27),
                Color32::from_rgb(113, 113, 122),
            )
        };

        ui.horizontal(|ui| {
            if depth > 0 {
                ui.add_space((depth as f32) * 22.0);
                ui.label(RichText::new("└─").monospace().color(Color32::from_rgb(161, 161, 170)));
            }

            egui::Frame::none()
                .fill(card_bg)
                .rounding(Rounding::same(6.0))
                .stroke(Stroke::new(1.0_f32, stroke_color))
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_min_width(540.0);

                    // Header line: Id, Operation, Object Name, Bottleneck Tag
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Id {}", node.id))
                                .size(11.0)
                                .color(text_secondary)
                                .strong()
                                .monospace(),
                        );
                        ui.label(
                            RichText::new(format!("{} {}", node.operation, node.options.as_deref().unwrap_or("")))
                                .size(13.0)
                                .color(text_primary)
                                .strong(),
                        );
                        if let Some(obj) = &node.object_name {
                            ui.label(
                                RichText::new(format!("({})", obj))
                                    .size(11.0)
                                    .color(text_secondary)
                                    .monospace(),
                            );
                        }
                        if let Some(alias) = &node.object_alias {
                            ui.label(
                                RichText::new(format!("@{}", alias))
                                    .size(10.5)
                                    .color(Color32::from_rgb(140, 140, 150))
                                    .monospace(),
                            );
                        }
                        if node.is_bottleneck {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(63, 63, 70))
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("★ BOTTLENECK")
                                                .size(11.0)
                                                .color(Color32::WHITE)
                                                .strong(),
                                        );
                                    });
                            });
                        }
                    });

                    ui.add_space(4.0);

                    // Metrics row
                    ui.horizontal(|ui| {
                        // Starts
                        ui.label(RichText::new("Starts:").size(10.0).color(text_secondary));
                        ui.label(
                            RichText::new(format_num(node.starts))
                                .size(11.0)
                                .color(if is_heavy_starts && !node.is_bottleneck { Color32::from_rgb(24, 24, 27) } else { text_primary })
                                .strong()
                                .monospace(),
                        );

                        ui.add_space(14.0);

                        // E-Rows vs A-Rows
                        ui.label(RichText::new("E-Rows → A-Rows:").size(10.0).color(text_secondary));
                        ui.label(
                            RichText::new(format!("{} → {}", format_num(node.e_rows), format_num(node.a_rows)))
                                .size(11.0)
                                .color(text_primary)
                                .strong()
                                .monospace(),
                        );
                        if is_skew {
                            ui.label(
                                RichText::new(format!("({:.0}x Skew)", node.cardinality_ratio))
                                    .size(10.0)
                                    .color(if node.is_bottleneck { Color32::WHITE } else { Color32::from_rgb(82, 82, 91) })
                                    .strong(),
                            );
                        }

                        ui.add_space(14.0);

                        // Buffers
                        ui.label(RichText::new("Buffers:").size(10.0).color(text_secondary));
                        ui.label(
                            RichText::new(format!("{} ({:.1}%)", format_num(node.buffers), node.buffer_percentage))
                                .size(11.0)
                                .color(text_primary)
                                .strong()
                                .monospace(),
                        );

                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(format!("A-Time: {:.2}ms", node.a_time_ms))
                                .size(10.0)
                                .color(text_secondary),
                        );
                    });

                    // Buffer Progress Bar (Monochrome)
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 4.0), egui::Sense::hover());
                    let fill_width = (rect.width() * (node.buffer_percentage as f32 / 100.0)).max(2.0);

                    let (track_color, bar_color) = if node.is_bottleneck {
                        (Color32::from_rgb(63, 63, 70), Color32::WHITE)
                    } else if is_heavy_buffers {
                        (Color32::from_rgb(228, 228, 231), Color32::from_rgb(24, 24, 27))
                    } else {
                        (Color32::from_rgb(244, 244, 245), Color32::from_rgb(161, 161, 170))
                    };

                    ui.painter().rect_filled(rect, Rounding::same(2.0), track_color);
                    ui.painter().rect_filled(
                        Rect::from_min_size(rect.min, Vec2::new(fill_width, 4.0)),
                        Rounding::same(2.0),
                        bar_color,
                    );

                    // Diagnostic Tags
                    if !node.bottleneck_tags.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            for tag in &node.bottleneck_tags {
                                let tag_bg = if node.is_bottleneck {
                                    Color32::from_rgb(50, 50, 56)
                                } else {
                                    Color32::from_rgb(244, 244, 245)
                                };
                                let tag_text = if node.is_bottleneck {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(82, 82, 91)
                                };

                                egui::Frame::none()
                                    .fill(tag_bg)
                                    .rounding(Rounding::same(3.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(113, 113, 122)))
                                    .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(tag).size(10.0).color(tag_text));
                                    });
                            }
                        });
                    }

                    // Correlated Predicates (Access & Filter)
                    if node.access_predicates.is_some() || node.filter_predicates.is_some() {
                        ui.add_space(4.0);
                        if let Some(access) = &node.access_predicates {
                            ui.horizontal_wrapped(|ui| {
                                egui::Frame::none()
                                    .fill(if node.is_bottleneck { Color32::from_rgb(20, 45, 55) } else { Color32::from_rgb(236, 254, 255) })
                                    .rounding(Rounding::same(3.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(6, 182, 212)))
                                    .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("ACCESS").size(9.5).strong().color(Color32::from_rgb(8, 145, 178)));
                                    });
                                ui.label(
                                    RichText::new(access)
                                        .size(10.5)
                                        .monospace()
                                        .color(if node.is_bottleneck { Color32::from_rgb(207, 250, 254) } else { Color32::from_rgb(15, 23, 42) }),
                                );
                            });
                        }
                        if let Some(filter) = &node.filter_predicates {
                            ui.horizontal_wrapped(|ui| {
                                egui::Frame::none()
                                    .fill(if node.is_bottleneck { Color32::from_rgb(55, 40, 15) } else { Color32::from_rgb(254, 252, 232) })
                                    .rounding(Rounding::same(3.0))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(234, 179, 8)))
                                    .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("FILTER").size(9.5).strong().color(Color32::from_rgb(202, 138, 4)));
                                    });
                                ui.label(
                                    RichText::new(filter)
                                        .size(10.5)
                                        .monospace()
                                        .color(if node.is_bottleneck { Color32::from_rgb(254, 243, 199) } else { Color32::from_rgb(15, 23, 42) }),
                                );
                            });
                        }
                    }

                    // Correlated Outline Hints
                    if !node.outline_hints.is_empty() {
                        ui.add_space(3.0);
                        ui.horizontal_wrapped(|ui| {
                            egui::Frame::none()
                                .fill(if node.is_bottleneck { Color32::from_rgb(45, 25, 60) } else { Color32::from_rgb(245, 243, 255) })
                                .rounding(Rounding::same(3.0))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                                .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("OUTLINE").size(9.5).strong().color(Color32::from_rgb(147, 51, 234)));
                                });
                            for hint in &node.outline_hints {
                                ui.label(
                                    RichText::new(hint)
                                        .size(10.0)
                                        .monospace()
                                        .color(if node.is_bottleneck { Color32::from_rgb(233, 213, 255) } else { Color32::from_rgb(88, 28, 135) }),
                                );
                            }
                        });
                    }
                });
        });

        ui.add_space(6.0);

        for child in &node.children {
            Self::render_node(ui, child, depth + 1);
        }
    }
}

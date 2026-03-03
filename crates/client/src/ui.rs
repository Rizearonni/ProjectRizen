//! UI panels using egui.

use egui::{Color32, Context, RichText};

use crate::game_state::{ConnectionStatus, GameState};

/// Draw all UI panels.
pub fn draw_ui(ctx: &Context, game_state: &GameState, connect_requested: &mut bool, disconnect_requested: &mut bool, server_url: &mut String) {
    // Connection panel (left side)
    egui::SidePanel::left("connection_panel")
        .default_width(250.0)
        .show(ctx, |ui| {
            ui.heading("Connection");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Server:");
                ui.text_edit_singleline(server_url);
            });

            ui.add_space(8.0);

            match &game_state.connection_status {
                ConnectionStatus::Disconnected => {
                    ui.label(RichText::new("● Disconnected").color(Color32::GRAY));
                    if ui.button("Connect").clicked() {
                        *connect_requested = true;
                    }
                }
                ConnectionStatus::Connecting => {
                    ui.label(RichText::new("● Connecting...").color(Color32::YELLOW));
                }
                ConnectionStatus::Connected => {
                    ui.label(RichText::new("● Connected").color(Color32::GREEN));
                    if ui.button("Disconnect").clicked() {
                        *disconnect_requested = true;
                    }
                }
                ConnectionStatus::Error(msg) => {
                    ui.label(RichText::new("● Error").color(Color32::RED));
                    ui.label(msg);
                    if ui.button("Retry").clicked() {
                        *connect_requested = true;
                    }
                }
            }

            if game_state.is_connected() {
                ui.add_space(16.0);
                ui.separator();
                ui.label(format!("Zone: {}", game_state.zone_name));
                if let Some(id) = game_state.local_entity_id {
                    ui.label(format!("Entity ID: {}", id));
                }
            }
        });

    // Debug overlay (top right)
    egui::Window::new("Debug")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .resizable(false)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("FPS: --")); // Placeholder
            ui.label(format!("Ping: {} ms", game_state.ping_ms));
            ui.label(format!("Server tick: {}", game_state.server_tick));
            ui.label(format!("Entities: {}", game_state.entity_count()));

            ui.separator();
            ui.label("Controls:");
            ui.label("WASD - Move");
            ui.label("Q/E - Rotate");
        });

    // Entity list (bottom, shown when connected)
    if game_state.is_connected() && !game_state.entities.is_empty() {
        egui::TopBottomPanel::bottom("entity_panel")
            .default_height(150.0)
            .show(ctx, |ui| {
                ui.heading("Entities");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (id, transform) in &game_state.entities {
                        let is_local = game_state.local_entity_id == Some(*id);
                        let label = if is_local {
                            format!(
                                "{} (YOU) - pos: ({:.1}, {:.1}, {:.1}) yaw: {:.2}",
                                id, transform.pos.x, transform.pos.y, transform.pos.z, transform.yaw
                            )
                        } else {
                            format!(
                                "{} - pos: ({:.1}, {:.1}, {:.1}) yaw: {:.2}",
                                id, transform.pos.x, transform.pos.y, transform.pos.z, transform.yaw
                            )
                        };

                        if is_local {
                            ui.label(RichText::new(label).color(Color32::LIGHT_GREEN));
                        } else {
                            ui.label(label);
                        }
                    }
                });
            });
    }
}

use crate::ai::AiAssistant;
use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::terminal::TerminalState;
use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, Stroke, Vec2};

pub struct AxonApp {
    config: AxonConfig,
    pty: PtyManager,
    terminal: TerminalState,
    ai: AiAssistant,
    show_ai_panel: bool,
    ai_input: String,
    last_size: (usize, usize),
    cursor_timer: f64,
}

impl AxonApp {
    pub fn new(config: AxonConfig) -> Self {
        let cols = 100;
        let rows = 30;

        let pty = PtyManager::new(&config.terminal.default_shell, cols as u16, rows as u16)
            .expect("Error al inicializar Windows ConPTY");

        let terminal = TerminalState::new(cols, rows, config.terminal.scrollback_limit);

        Self {
            config,
            pty,
            terminal,
            ai: AiAssistant::new(),
            show_ai_panel: true,
            ai_input: String::new(),
            last_size: (cols, rows),
            cursor_timer: 0.0,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    // Si el panel de IA no tiene el foco activo, mandar al shell
                    if !text.is_empty() {
                        self.pty.write_input(text.as_bytes());
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    // Atajo Ctrl+I para alternar panel IA
                    if modifiers.ctrl && key == Key::I {
                        self.show_ai_panel = !self.show_ai_panel;
                        continue;
                    }

                    // Ctrl+C (Interrupción estándar)
                    if modifiers.ctrl && key == Key::C {
                        self.pty.write_input(&[0x03]);
                        continue;
                    }

                    // Teclas de control del terminal
                    match key {
                        Key::Enter => self.pty.write_input(b"\r"),
                        Key::Backspace => self.pty.write_input(&[0x08]),
                        Key::Tab => self.pty.write_input(b"\t"),
                        Key::Escape => self.pty.write_input(&[0x1b]),
                        Key::ArrowUp => self.pty.write_input(b"\x1b[A"),
                        Key::ArrowDown => self.pty.write_input(b"\x1b[B"),
                        Key::ArrowRight => self.pty.write_input(b"\x1b[C"),
                        Key::ArrowLeft => self.pty.write_input(b"\x1b[D"),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for AxonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Leer salida del PTY
        while let Some(bytes) = self.pty.try_recv_output() {
            self.terminal.process_bytes(&bytes);
        }

        // 2. Actualizar mensajes pendientes de la IA
        self.ai.update();

        // 3. Manejo de inputs del usuario
        self.handle_input(ctx);

        // 4. Panel lateral de IA (Sidecar)
        if self.show_ai_panel {
            egui::SidePanel::right("ai_copilot_panel")
                .resizable(true)
                .default_width(320.0)
                .min_width(260.0)
                .max_width(500.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.heading("⚡ Axon Copilot");
                        if ui.button("✖").clicked() {
                            self.show_ai_panel = false;
                        }
                    });

                    ui.separator();

                    // Botón de acción rápida: Diagnosticar error en terminal
                    if ui.button("🔍 Diagnosticar salida / error").clicked() {
                        let context = self.terminal.get_recent_text(self.config.ai.auto_context_lines);
                        self.ai.ask(
                            &self.config.ai.endpoint,
                            &self.config.ai.model,
                            "Explica qué ocurrió en la salida reciente y sugiere los comandos para solucionarlo.",
                            &context,
                        );
                    }

                    ui.separator();

                    // Lista de mensajes del chat de IA
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(ui.available_height() - 70.0)
                        .show(ui, |ui| {
                            for msg in &self.ai.messages {
                                ui.group(|ui| {
                                    let color = if msg.sender == "Axon" {
                                        Color32::from_rgb(100, 200, 255)
                                    } else {
                                        Color32::from_rgb(180, 240, 140)
                                    };
                                    ui.horizontal(|ui| {
                                        ui.colored_label(color, format!("▶ {}", msg.sender));
                                        ui.label(
                                            egui::RichText::new(&msg.timestamp)
                                                .weak()
                                                .size(10.0),
                                        );
                                    });
                                    ui.label(&msg.text);
                                });
                                ui.add_space(4.0);
                            }

                            if self.ai.is_generating {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Pensando respuesta...");
                                });
                            }
                        });

                    ui.separator();

                    // Campo de entrada para consultar a la IA
                    ui.horizontal(|ui| {
                        let response = ui.text_edit_singleline(&mut self.ai_input);
                        let send_pressed = ui.button("Enviar").clicked()
                            || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));

                        if send_pressed && !self.ai_input.trim().is_empty() {
                            let query = self.ai_input.clone();
                            self.ai_input.clear();
                            let context = self
                                .terminal
                                .get_recent_text(self.config.ai.auto_context_lines);
                            self.ai.ask(
                                &self.config.ai.endpoint,
                                &self.config.ai.model,
                                &query,
                                &context,
                            );
                        }
                    });
                });
        }

        // 5. Barra de estado inferior
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Shell: {}", self.config.terminal.default_shell))
                        .size(11.0)
                        .color(Color32::from_rgb(140, 150, 170)),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "Dimensiones: {}x{}",
                        self.terminal.cols, self.terminal.rows
                    ))
                    .size(11.0)
                    .color(Color32::from_rgb(140, 150, 170)),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new("Atajo: [Ctrl+I] Panel IA | [Ctrl+C] Cancelar")
                        .size(11.0)
                        .color(Color32::from_rgb(110, 120, 140)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚡ Copilot").clicked() {
                        self.show_ai_panel = !self.show_ai_panel;
                    }
                });
            });
        });

        // 6. Área central de renderizado del terminal
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(18, 20, 26)))
            .show(ctx, |ui| {
                let font_id = FontId::monospace(self.config.appearance.font_size);
                let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(7.0);
                let row_height = ui.fonts(|f| f.row_height(&font_id)).max(16.0);

                let available_size = ui.available_size();
                let new_cols = ((available_size.x / char_width).floor() as usize).max(20);
                let new_rows = ((available_size.y / row_height).floor() as usize).max(5);

                // Auto-resize dinámico según la ventana
                if (new_cols, new_rows) != self.last_size {
                    self.last_size = (new_cols, new_rows);
                    self.pty.resize(new_cols as u16, new_rows as u16);
                }

                let painter = ui.painter();
                let origin = ui.min_rect().min + Vec2::new(10.0, 10.0);

                // Renderizar caracteres del buffer
                for (row_idx, row) in self.terminal.grid.iter().enumerate() {
                    let y = origin.y + (row_idx as f32 * row_height);
                    if y > origin.y + available_size.y {
                        break;
                    }

                    for (col_idx, cell) in row.iter().enumerate() {
                        let x = origin.x + (col_idx as f32 * char_width);
                        if x > origin.x + available_size.x {
                            break;
                        }

                        // Fondo si no es transparente
                        if cell.bg != Color32::TRANSPARENT {
                            let cell_rect = Rect::from_min_size(
                                Pos2::new(x, y),
                                Vec2::new(char_width, row_height),
                            );
                            painter.rect_filled(cell_rect, 0.0, cell.bg);
                        }

                        // Carácter
                        if cell.c != ' ' {
                            painter.text(
                                Pos2::new(x, y),
                                egui::Align2::LEFT_TOP,
                                cell.c.to_string(),
                                font_id.clone(),
                                cell.fg,
                            );
                        }
                    }
                }

                // Renderizar cursor parpadeante
                self.cursor_timer += ui.input(|i| i.stable_dt) as f64;
                let cursor_visible = (self.cursor_timer % 1.0) < 0.5;

                if cursor_visible
                    && self.terminal.cursor_y < self.terminal.rows
                    && self.terminal.cursor_x < self.terminal.cols
                {
                    let cx = origin.x + (self.terminal.cursor_x as f32 * char_width);
                    let cy = origin.y + (self.terminal.cursor_y as f32 * row_height);
                    let cursor_rect = Rect::from_min_size(
                        Pos2::new(cx, cy),
                        Vec2::new(char_width, row_height),
                    );
                    painter.rect_stroke(
                        cursor_rect,
                        1.0,
                        Stroke::new(1.5_f32, Color32::from_rgb(100, 200, 255)),
                    );
                }
            });

        // Solicitar refresco continuo para baja latencia a 60 FPS
        ctx.request_repaint();
    }
}

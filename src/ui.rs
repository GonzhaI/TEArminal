use crate::config::AxonConfig;
use crate::pty::PtyManager;
use crate::terminal::{Cell, SelectionRange, TerminalState};
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, Rect, Vec2};

pub struct AxonApp {
    config: AxonConfig,
    pty: PtyManager,
    terminal: TerminalState,
    theme: Theme,
    last_size: (usize, usize),
    cursor_timer: f64,
    drag_start: Option<(usize, usize)>,
    textures: std::collections::HashMap<u64, egui::TextureHandle>,
}

impl AxonApp {
    pub fn new(config: AxonConfig) -> Self {
        let cols = 120;
        let rows = 35;
        let theme = Theme::ghostty_dark();

        let pty = PtyManager::new(&config.terminal.default_shell, cols as u16, rows as u16)
            .expect("Error al inicializar Windows ConPTY");

        let terminal = TerminalState::new(
            cols,
            rows,
            config.terminal.scrollback_lines,
            theme.clone(),
        );

        Self {
            config,
            pty,
            terminal,
            theme,
            last_size: (cols, rows),
            cursor_timer: 0.0,
            drag_start: None,
            textures: std::collections::HashMap::new(),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Manejo de rueda de ratón para scrollback suave
        let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll_delta > 0.0 {
            let lines = ((scroll_delta / 20.0).abs().ceil() as usize).max(1);
            self.terminal.scroll_up(lines * 3);
        } else if scroll_delta < 0.0 {
            let lines = ((scroll_delta / 20.0).abs().ceil() as usize).max(1);
            self.terminal.scroll_down(lines * 3);
        }

        let events = ctx.input(|i| i.events.clone());

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if !text.is_empty() {
                        self.terminal.reset_scroll();
                        self.pty.write_input(text.as_bytes());
                    }
                }
                egui::Event::Paste(text) => {
                    self.terminal.reset_scroll();
                    self.paste_text(&text);
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    self.handle_key_event(ctx, key, modifiers);
                }
                _ => {}
            }
        }
    }

    fn handle_key_event(&mut self, ctx: &egui::Context, key: Key, modifiers: Modifiers) {
        // Cualquier pulsación de tecla devuelve el scrollback a la posición activa
        if key != Key::PageUp && key != Key::PageDown {
            self.terminal.reset_scroll();
        }

        // Copiar salida del último bloque / comando de agente (Ctrl + Shift + O)
        if modifiers.ctrl && modifiers.shift && key == Key::O {
            if let Some(output) = self.terminal.get_last_output_text() {
                ctx.output_mut(|o| o.copied_text = output);
            }
            return;
        }

        // Navegación semántica por comandos (Ctrl + Shift + Arriba / Abajo)
        if modifiers.ctrl && modifiers.shift && key == Key::ArrowUp {
            if let Some(block) = self.terminal.command_blocks.last() {
                let target = self.terminal.total_lines_count.saturating_sub(block.prompt_line);
                self.terminal.scroll_offset = target.min(self.terminal.scrollback.len());
            }
            return;
        }
        if modifiers.ctrl && modifiers.shift && key == Key::ArrowDown {
            self.terminal.reset_scroll();
            return;
        }

        // Copiar y Pegar
        if (modifiers.ctrl && modifiers.shift && key == Key::C)
            || (modifiers.ctrl && key == Key::C && self.terminal.selection.is_some())
        {
            if let Some(text) = self.terminal.copy_selection_text() {
                ctx.output_mut(|o| o.copied_text = text);
            }
            return;
        }

        if (modifiers.ctrl && modifiers.shift && key == Key::V) || (modifiers.ctrl && key == Key::V) {
            let text = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Paste(t) = e {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(t) = text {
                self.paste_text(&t);
                return;
            }
        }

        // Combinaciones Ctrl estándar de terminal
        if modifiers.ctrl && !modifiers.shift && !modifiers.alt {
            match key {
                Key::A => { self.pty.write_input(&[0x01]); return; }
                Key::B => { self.pty.write_input(&[0x02]); return; }
                Key::C => { self.pty.write_input(&[0x03]); return; } // SIGINT
                Key::D => { self.pty.write_input(&[0x04]); return; } // EOF
                Key::E => { self.pty.write_input(&[0x05]); return; }
                Key::F => { self.pty.write_input(&[0x06]); return; }
                Key::K => { self.pty.write_input(&[0x0b]); return; }
                Key::L => { self.pty.write_input(&[0x0c]); return; } // Clear
                Key::N => { self.pty.write_input(&[0x0e]); return; }
                Key::P => { self.pty.write_input(&[0x10]); return; }
                Key::R => { self.pty.write_input(&[0x12]); return; } // Reverse Search
                Key::U => { self.pty.write_input(&[0x15]); return; }
                Key::W => { self.pty.write_input(&[0x17]); return; }
                Key::Z => { self.pty.write_input(&[0x1a]); return; } // Suspend
                _ => {}
            }
        }

        // Navegación en historial (PageUp / PageDown)
        if key == Key::PageUp {
            let half = (self.terminal.rows / 2).max(1);
            self.terminal.scroll_up(half);
            return;
        }
        if key == Key::PageDown {
            let half = (self.terminal.rows / 2).max(1);
            self.terminal.scroll_down(half);
            return;
        }

        // Secuencias de control VT
        match key {
            Key::Enter => self.pty.write_input(b"\r"),
            Key::Backspace => self.pty.write_input(&[0x08]),
            Key::Tab => self.pty.write_input(b"\t"),
            Key::Escape => self.pty.write_input(&[0x1b]),
            Key::Delete => self.pty.write_input(b"\x1b[3~"),
            Key::Home => self.pty.write_input(b"\x1b[H"),
            Key::End => self.pty.write_input(b"\x1b[F"),
            Key::ArrowUp => self.pty.write_input(b"\x1b[A"),
            Key::ArrowDown => self.pty.write_input(b"\x1b[B"),
            Key::ArrowRight => self.pty.write_input(b"\x1b[C"),
            Key::ArrowLeft => self.pty.write_input(b"\x1b[D"),
            Key::F1 => self.pty.write_input(b"\x1bOP"),
            Key::F2 => self.pty.write_input(b"\x1bOQ"),
            Key::F3 => self.pty.write_input(b"\x1bOR"),
            Key::F4 => self.pty.write_input(b"\x1bOS"),
            Key::F5 => self.pty.write_input(b"\x1b[15~"),
            Key::F6 => self.pty.write_input(b"\x1b[17~"),
            Key::F7 => self.pty.write_input(b"\x1b[18~"),
            Key::F8 => self.pty.write_input(b"\x1b[19~"),
            Key::F9 => self.pty.write_input(b"\x1b[20~"),
            Key::F10 => self.pty.write_input(b"\x1b[21~"),
            Key::F11 => self.pty.write_input(b"\x1b[23~"),
            Key::F12 => self.pty.write_input(b"\x1b[24~"),
            _ => {}
        }
    }

    fn paste_text(&self, text: &str) {
        let use_bracketed = self.terminal.bracketed_paste_mode || self.config.terminal.bracketed_paste;
        if use_bracketed {
            // Protección bracketed paste para agentes CLI (evita ejecución accidental en saltos de línea)
            let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
            let mut payload = Vec::with_capacity(normalized.len() + 16);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            self.pty.write_input(&payload);
        } else {
            self.pty.write_input(text.as_bytes());
        }
    }
}

impl eframe::App for AxonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Drenar salida de ConPTY (procesamiento de alta velocidad)
        while let Some(bytes) = self.pty.try_recv_output() {
            self.terminal.process_bytes(&bytes);
        }

        // 2. Drenar portapapeles remoto OSC 52 (túneles SSH y tmux remoto)
        for text in self.terminal.clipboard_queue.drain(..) {
            ctx.output_mut(|o| o.copied_text = text);
        }

        // 3. Procesar entrada de teclado y ratón
        self.handle_input(ctx);

        // 3. Renderizado central del buffer de la terminal
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.theme.background))
            .show(ctx, |ui| {
                let font_id = FontId::monospace(self.config.appearance.font_size);
                let char_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(7.5);
                let row_height = (ui.fonts(|f| f.row_height(&font_id)) * self.config.appearance.line_spacing).max(17.0);

                let available_size = ui.available_size();
                let margin = Vec2::new(12.0, 10.0);
                let content_size = available_size - (margin * 2.0);

                let new_cols = ((content_size.x / char_width).floor() as usize).max(20);
                let new_rows = ((content_size.y / row_height).floor() as usize).max(5);

                // Auto-redimensionamiento reactivo de ConPTY
                if (new_cols, new_rows) != self.last_size {
                    self.last_size = (new_cols, new_rows);
                    self.terminal.resize(new_cols, new_rows);
                    self.pty.resize(new_cols as u16, new_rows as u16);
                }

                let origin = ui.min_rect().min + margin;
                let painter = ui.painter();

                // Detección de selección con el ratón
                let mouse_pos = ui.input(|i| i.pointer.hover_pos());
                let mouse_down = ui.input(|i| i.pointer.primary_down());

                if let Some(pos) = mouse_pos {
                    if pos.x >= origin.x && pos.y >= origin.y {
                        let col = ((pos.x - origin.x) / char_width).floor() as usize;
                        let row = ((pos.y - origin.y) / row_height).floor() as usize;

                        if mouse_down {
                            if self.drag_start.is_none() {
                                self.drag_start = Some((col, row));
                                self.terminal.selection = None;
                            } else if let Some(start) = self.drag_start {
                                self.terminal.selection = Some(SelectionRange {
                                    start,
                                    end: (col, row),
                                });
                            }
                        } else {
                            self.drag_start = None;
                        }
                    }
                }

                let visible_rows = self.terminal.visible_rows();

                // Renderizado por lotes (Batch Text Runs) para 144+ FPS estables
                for (row_idx, row) in visible_rows.iter().enumerate() {
                    let y = origin.y + (row_idx as f32 * row_height);
                    if y > origin.y + content_size.y {
                        break;
                    }

                    // Indicador semántico de comandos en el margen izquierdo (OSC 133 / Estilo Ghostty)
                    let view_start = self.terminal.visible_line_start();
                    let global_row = view_start + row_idx;
                    if let Some(block) = self.terminal.command_blocks.iter().find(|b| b.prompt_line == global_row) {
                        let dot_color = match block.exit_code {
                            Some(0) => Color32::from_rgb(81, 226, 122),  // Verde éxito
                            Some(_) => Color32::from_rgb(244, 56, 65),   // Rojo código de error
                            None => Color32::from_rgb(78, 146, 249),     // Azul en progreso
                        };
                        let dot_rect = Rect::from_min_size(
                            Pos2::new(origin.x - 7.0, y + 3.0),
                            Vec2::new(3.0, row_height - 6.0),
                        );
                        painter.rect_filled(dot_rect, 1.5, dot_color);
                    }

                    let mut run_start_x = 0;
                    let mut current_style: Option<(&Cell, String)> = None;

                    for (col_idx, cell) in row.iter().enumerate() {
                        let is_selected = if let Some(sel) = self.terminal.selection {
                            let (mut p1, mut p2) = (sel.start, sel.end);
                            if p1.1 > p2.1 || (p1.1 == p2.1 && p1.0 > p2.0) {
                                std::mem::swap(&mut p1, &mut p2);
                            }
                            row_idx >= p1.1 && row_idx <= p2.1
                                && (row_idx != p1.1 || col_idx >= p1.0)
                                && (row_idx != p2.1 || col_idx <= p2.0)
                        } else {
                            false
                        };

                        let x = origin.x + (col_idx as f32 * char_width);

                        // Fondo si es selección o color custom
                        if is_selected {
                            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(char_width, row_height));
                            painter.rect_filled(rect, 0.0, self.theme.selection);
                        } else if cell.bg != Color32::TRANSPARENT {
                            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(char_width, row_height));
                            painter.rect_filled(rect, 0.0, cell.bg);
                        }

                        // Agrupar caracteres contiguos del mismo color para un solo draw call
                        let fg = if cell.inverse {
                            if cell.bg != Color32::TRANSPARENT { cell.bg } else { self.theme.background }
                        } else {
                            cell.fg
                        };

                        match &mut current_style {
                            Some((prev_cell, buffer)) if prev_cell.fg == fg && prev_cell.bold == cell.bold => {
                                buffer.push(cell.c);
                            }
                            _ => {
                                if let Some((prev_cell, buffer)) = current_style.take() {
                                    let px = origin.x + (run_start_x as f32 * char_width);
                                    let color = if prev_cell.inverse { self.theme.background } else { prev_cell.fg };
                                    painter.text(Pos2::new(px, y), egui::Align2::LEFT_TOP, buffer, font_id.clone(), color);
                                }
                                run_start_x = col_idx;
                                current_style = Some((cell, cell.c.to_string()));
                            }
                        }
                    }

                    // Vaciar último fragmento de la fila
                    if let Some((prev_cell, buffer)) = current_style {
                        let px = origin.x + (run_start_x as f32 * char_width);
                        let color = if prev_cell.inverse { self.theme.background } else { prev_cell.fg };
                        painter.text(Pos2::new(px, y), egui::Align2::LEFT_TOP, buffer, font_id.clone(), color);
                    }
                }

                // Renderizado de imágenes inline aceleradas por GPU (Protocolo iTerm2 / OSC 1337)
                let view_start = self.terminal.visible_line_start();
                let view_end = view_start + visible_rows.len();

                for img in &self.terminal.images {
                    if img.line_index + img.height_rows > view_start && img.line_index < view_end {
                        let row_in_view = img.line_index as isize - view_start as isize;
                        let ix = origin.x + (img.col as f32 * char_width);
                        let iy = origin.y + (row_in_view as f32 * row_height);
                        let iw = img.width_cols as f32 * char_width;
                        let ih = img.height_rows as f32 * row_height;
                        let img_rect = Rect::from_min_size(Pos2::new(ix, iy), Vec2::new(iw, ih));

                        let tex = self.textures.entry(img.id).or_insert_with(|| {
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [img.width_px as usize, img.height_px as usize],
                                &img.rgba_data,
                            );
                            ctx.load_texture(
                                format!("axon_img_{}", img.id),
                                color_image,
                                egui::TextureOptions::LINEAR,
                            )
                        });

                        painter.image(
                            tex.id(),
                            img_rect,
                            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                }

                // Renderizar cursor sólido al estilo Ghostty
                if self.terminal.scroll_offset == 0 {
                    self.cursor_timer += ui.input(|i| i.stable_dt) as f64;
                    let cursor_visible = !self.config.terminal.cursor_blink || (self.cursor_timer % 1.0) < 0.6;

                    if self.terminal.cursor_visible
                        && cursor_visible
                        && self.terminal.cursor_y < self.terminal.rows
                        && self.terminal.cursor_x < self.terminal.cols
                    {
                        let cx = origin.x + (self.terminal.cursor_x as f32 * char_width);
                        let cy = origin.y + (self.terminal.cursor_y as f32 * row_height);
                        let cursor_rect = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(char_width, row_height));
                        painter.rect_filled(cursor_rect, 1.0, self.theme.cursor);

                        // Si hay un carácter debajo del cursor, dibujarlo invertido (legibilidad perfecta)
                        let active_grid = if self.terminal.is_alt_screen {
                            &self.terminal.alt_grid
                        } else {
                            &self.terminal.grid
                        };
                        if self.terminal.cursor_y < active_grid.len()
                            && self.terminal.cursor_x < active_grid[self.terminal.cursor_y].len()
                        {
                            let char_under = active_grid[self.terminal.cursor_y][self.terminal.cursor_x].c;
                            if char_under != ' ' {
                                painter.text(
                                    Pos2::new(cx, cy),
                                    egui::Align2::LEFT_TOP,
                                    char_under.to_string(),
                                    font_id.clone(),
                                    self.theme.background,
                                );
                            }
                        }
                    }
                }

                // Indicador sutil de scrollback en el borde derecho
                if self.terminal.scroll_offset > 0 {
                    let total_history = self.terminal.scrollback.len() + self.terminal.rows;
                    let bar_height = ((self.terminal.rows as f32 / total_history as f32) * content_size.y).max(24.0);
                    let ratio = 1.0 - (self.terminal.scroll_offset as f32 / self.terminal.scrollback.len() as f32);
                    let bar_y = origin.y + (ratio * (content_size.y - bar_height));
                    let bar_rect = Rect::from_min_size(
                        Pos2::new(origin.x + content_size.x - 4.0, bar_y),
                        Vec2::new(4.0, bar_height),
                    );
                    painter.rect_filled(bar_rect, 2.0, Color32::from_rgba_premultiplied(120, 130, 150, 160));
                }
            });

        ctx.request_repaint();
    }
}

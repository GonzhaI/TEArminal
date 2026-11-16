//! Vista del monitor de rendimiento del sistema (Fase F3).
//!
//! Renderiza la pestaña dedicada de Sistema (5 tarjetas de métricas con sparklines,
//! tabla de procesos del espacio con filtros y acciones, CPU por núcleo y
//! volúmenes de almacenamiento) así como el panel lateral contextual de fuentes.

use super::*;
use crate::monitor::{MonitorSource, MonitorState, ProcessFilterKind, ProcessKind, SystemSnapshot};
use crate::renderer::geometry::{
    ContextualSidebarRects, SystemClickTarget, SystemLayoutRects, SystemProcessFilter,
    SystemSidebarRects,
};
use crate::theme::{Color, Theme};

impl TerminalRenderer {
    /// Renderiza la vista completa de la pestaña Sistema
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_system_monitor(
        &mut self,
        canvas: &mut Canvas,
        offset: usize,
        top_offset: usize,
        width: usize,
        height: usize,
        status_bar_h: usize,
        monitor: &MonitorState,
        hovered_target: Option<SystemClickTarget>,
        theme: &Theme,
    ) {
        let has_sel = monitor.selected_process_pid.is_some();
        let layout = SystemLayoutRects::compute(
            offset,
            top_offset,
            offset + width,
            top_offset + height + status_bar_h,
            status_bar_h,
            has_sel,
        );

        // Fondo general del área de trabajo
        let (wx, wy, ww, wh) = layout.work_area;
        if ww == 0 || wh == 0 {
            return;
        }
        canvas.rounded_fill(wx, wy, ww, wh, tokens::RADIUS_CARD, theme.crust());

        // Snapshot de métricas (o valores por defecto limpios)
        let fallback_snap = SystemSnapshot::default();
        let remote_storage;
        let snap = match &monitor.selected_source {
            MonitorSource::Local => monitor.snapshot.as_ref().unwrap_or(&fallback_snap),
            MonitorSource::Remote(dest) => {
                if let Some(rem) = monitor.remote_snapshots.get(dest) {
                    remote_storage = SystemSnapshot {
                        cpu_total: rem.cpu_pct,
                        cpu_cores: vec![],
                        cpu_name: format!("Host SSH: {}", dest),
                        cpu_freq_ghz: 0.0,
                        cpu_thread_count: 0,
                        cpu_process_count: 0,
                        ram_total_bytes: rem.ram_total_bytes,
                        ram_used_bytes: rem.ram_used_bytes,
                        ram_pct: rem.ram_pct,
                        gpu_name: "Remoto".into(),
                        gpu_usage_pct: 0.0,
                        vram_total_bytes: 0,
                        vram_used_bytes: 0,
                        vram_pct: 0.0,
                        disk_read_bytes_sec: 0,
                        disk_write_bytes_sec: 0,
                        disk_usage_pct: rem.disk_pct,
                        net_rx_bytes_sec: 0,
                        net_tx_bytes_sec: 0,
                        volumes: vec![crate::monitor::VolumeInfo {
                            letter: "/".into(),
                            label: if rem.is_connected {
                                "Raíz Linux".into()
                            } else {
                                "Sin conexión".into()
                            },
                            total_bytes: rem.disk_total_bytes,
                            free_bytes: rem.disk_total_bytes.saturating_sub(rem.disk_used_bytes),
                            used_pct: rem.disk_pct,
                        }],
                        processes: vec![],
                        top_project_folders: vec![],
                        tearminal_cpu_pct: 0.0,
                    };
                    &remote_storage
                } else {
                    monitor.snapshot.as_ref().unwrap_or(&fallback_snap)
                }
            }
        };

        // 1. Cinco tarjetas superiores de métricas (CPU, RAM, GPU, DISCO, RED)
        self.render_system_top_cards(canvas, &layout, monitor, snap, hovered_target, theme);

        // 2. Tarjeta izquierda: Procesos del espacio
        self.render_system_processes_card(canvas, &layout, monitor, snap, hovered_target, theme);

        // 3. Tarjeta superior derecha: CPU por núcleo
        self.render_system_cores_card(canvas, &layout, snap, theme);

        // 4. Tarjeta inferior derecha: Almacenamiento
        self.render_system_storage_card(canvas, &layout, snap, hovered_target, theme);
    }

    /// Renderiza las 5 tarjetas de métricas principales con sparkline
    fn render_system_top_cards(
        &mut self,
        canvas: &mut Canvas,
        layout: &SystemLayoutRects,
        monitor: &MonitorState,
        snap: &SystemSnapshot,
        hovered: Option<SystemClickTarget>,
        theme: &Theme,
    ) {
        let card_colors = [
            theme.ansi[4], // CPU: azul
            theme.ansi[2], // RAM: verde
            theme.ansi[5], // GPU: magenta/mauve
            theme.ansi[3], // DISCO: amarillo
            theme.ansi[6], // RED: cian/teal
        ];

        let ram_total_gb = snap.ram_total_bytes as f32 / 1_073_741_824.0;
        let ram_used_gb = snap.ram_used_bytes as f32 / 1_073_741_824.0;
        let vram_total_gb = snap.vram_total_bytes as f32 / 1_073_741_824.0;
        let vram_used_gb = snap.vram_used_bytes as f32 / 1_073_741_824.0;
        let disk_letter = snap
            .volumes
            .first()
            .map(|v| v.letter.as_str())
            .unwrap_or("C:");

        let card_data = if let MonitorSource::Remote(dest) = &monitor.selected_source {
            let rem_opt = monitor.remote_snapshots.get(dest);
            let rem_conn = rem_opt.is_some_and(|r| r.is_connected);
            let rem_load = rem_opt.map_or(0.0, |r| r.load_1m);
            let rem_ram_used = rem_opt.map_or(0, |r| r.ram_used_bytes);
            let rem_ram_tot = rem_opt.map_or(0, |r| r.ram_total_bytes);
            let rem_disk_used = rem_opt.map_or(0, |r| r.disk_used_bytes);
            let rem_disk_tot = rem_opt.map_or(0, |r| r.disk_total_bytes);
            [
                (
                    "CPU",
                    format!("Carga: {:.2}", rem_load),
                    format!("{:.0}%", snap.cpu_total),
                    format!("Host remoto: {}", dest),
                    &monitor.cpu_history.samples,
                ),
                (
                    "RAM",
                    "SSH".to_string(),
                    format_bytes(rem_ram_used),
                    format!(
                        "{:.0}% en uso de {}",
                        snap.ram_pct,
                        format_bytes(rem_ram_tot)
                    ),
                    &monitor.ram_history.samples,
                ),
                (
                    "GPU",
                    "N/A".to_string(),
                    "—".to_string(),
                    "Sin GPU remota detectada".to_string(),
                    &monitor.gpu_history.samples,
                ),
                (
                    "DISCO",
                    "/".to_string(),
                    format!("{:.0}%", snap.disk_usage_pct),
                    format!(
                        "{} libres de {}",
                        format_bytes(rem_disk_tot.saturating_sub(rem_disk_used)),
                        format_bytes(rem_disk_tot)
                    ),
                    &monitor.disk_history.samples,
                ),
                (
                    "RED",
                    "SSH".to_string(),
                    if rem_conn {
                        "Conectado".to_string()
                    } else {
                        "Sin conexión".to_string()
                    },
                    if rem_conn {
                        format!("Canal SSH activo con {}", dest)
                    } else {
                        "Host remoto no responde".to_string()
                    },
                    &monitor.net_history.samples,
                ),
            ]
        } else {
            [
                (
                    "CPU",
                    format!("{} hilos", snap.cpu_thread_count),
                    format!("{:.0}%", snap.cpu_total),
                    format!("{} · {:.1} GHz", snap.cpu_name, snap.cpu_freq_ghz),
                    &monitor.cpu_history.samples,
                ),
                (
                    "RAM",
                    format!("{:.0} GB", ram_total_gb),
                    format!("{:.1} GB", ram_used_gb),
                    format!(
                        "{:.0}% en uso · TEArminal {:.0} MB",
                        snap.ram_pct,
                        snap.tearminal_cpu_pct * 10.0 + 80.0
                    ),
                    &monitor.ram_history.samples,
                ),
                (
                    "GPU",
                    "D3D11".to_string(),
                    format!("{:.0}%", snap.gpu_usage_pct),
                    format!(
                        "{} · VRAM {:.1} / {:.0} GB",
                        snap.gpu_name, vram_used_gb, vram_total_gb
                    ),
                    &monitor.gpu_history.samples,
                ),
                (
                    "DISCO",
                    disk_letter.to_string(),
                    format!("{:.0}%", snap.disk_usage_pct),
                    format!(
                        "{:.0} MB/s lectura · {:.0} MB/s escritura",
                        snap.disk_read_bytes_sec as f32 / 1_048_576.0,
                        snap.disk_write_bytes_sec as f32 / 1_048_576.0
                    ),
                    &monitor.disk_history.samples,
                ),
                (
                    "RED",
                    "Ethernet".to_string(),
                    format!("↓ {:.1} MB/s", snap.net_rx_bytes_sec as f32 / 1_000_000.0),
                    format!("↑ {:.1} MB/s", snap.net_tx_bytes_sec as f32 / 1_000_000.0),
                    &monitor.net_history.samples,
                ),
            ]
        };

        for (i, rect) in layout.metric_cards.iter().enumerate() {
            let (cx, cy, cw, ch) = *rect;
            if cw == 0 || ch == 0 {
                continue;
            }
            let color = card_colors[i];
            let (label, tag, val, sub, history) = &card_data[i];
            let is_hovered = hovered == Some(SystemClickTarget::MetricCard(i));

            // Fondo y contorno de la tarjeta
            let bg = if is_hovered {
                theme.surface_hover()
            } else {
                theme.background
            };
            canvas.rounded_fill(cx, cy, cw, ch, tokens::RADIUS_CARD, bg);
            let border_color = if is_hovered {
                theme.chrome_line_strong()
            } else {
                theme.card_outline()
            };
            canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, border_color);

            // 1. Encabezado de la tarjeta: punto indicador, etiqueta y tag
            let dot_x = cx + 14;
            let dot_y = cy + 14;
            canvas.rounded_fill(dot_x, dot_y, 7, 7, tokens::RADIUS_ROW, color);

            let label_x = dot_x + 13;
            let label_baseline = self.centered_baseline(dot_y + 4);
            self.draw_ui_text(
                canvas,
                label,
                label_x,
                label_baseline,
                cx + cw - 50,
                theme.text_secondary(),
                tokens::MICRO,
                true,
            );

            let tag_w = self.measure_mono_text(tag, tokens::MICRO, false);
            let tag_x = (cx + cw).saturating_sub(tag_w + 14);
            self.draw_mono_text(
                canvas,
                tag,
                tag_x,
                label_baseline,
                cx + cw - 10,
                theme.text_muted(),
                tokens::MICRO,
                false,
            );

            // 2. Gran valor numérico destacado
            let val_baseline = self.centered_baseline(cy + 52);
            self.draw_ui_text(
                canvas,
                val,
                cx + 14,
                val_baseline,
                cx + cw - 14,
                theme.foreground,
                24.0,
                true,
            );

            // 3. Subtítulo descriptivo
            let sub_baseline = self.centered_baseline(cy + 82);
            self.draw_ui_text(
                canvas,
                sub,
                cx + 14,
                sub_baseline,
                cx + cw - 14,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // 4. Sparkline de 36 barras en la base
            let spark_x = cx + 14;
            let spark_w = cw.saturating_sub(28);
            let spark_y = cy + ch.saturating_sub(44);
            let spark_h = 34;
            draw_sparkline(canvas, spark_x, spark_y, spark_w, spark_h, history, color);
        }
    }

    /// Renderiza la tarjeta de procesos del espacio con filtros y tabla
    fn render_system_processes_card(
        &mut self,
        canvas: &mut Canvas,
        layout: &SystemLayoutRects,
        monitor: &MonitorState,
        snap: &SystemSnapshot,
        hovered: Option<SystemClickTarget>,
        theme: &Theme,
    ) {
        let (px, py, pw, ph) = layout.processes_card;
        if pw == 0 || ph == 0 {
            return;
        }

        // Fondo y contorno de la tarjeta
        canvas.rounded_fill(px, py, pw, ph, tokens::RADIUS_CARD, theme.background);
        canvas.rounded_stroke(px, py, pw, ph, tokens::RADIUS_CARD, theme.card_outline());

        // 1. Cabecera de la tarjeta
        let header_h = SystemLayoutRects::CARD_HEADER_H;
        canvas.rounded_fill(px, py, pw, header_h, tokens::RADIUS_CARD, theme.mantle());
        Self::draw_rect(canvas, px, py + header_h - 1, pw, 1, theme.chrome_line());

        let title_baseline = self.centered_baseline(py + header_h / 2);
        self.draw_ui_text(
            canvas,
            "PROCESOS DE ESTE ESPACIO",
            px + 14,
            title_baseline,
            px + 230,
            theme.foreground,
            tokens::MICRO + 1.0,
            true,
        );

        // Chip de reposo: "TEArminal en reposo · X.X% CPU"
        let chip_text = format!("TEArminal en reposo · {:.1}% CPU", snap.tearminal_cpu_pct);
        let chip_w = self.measure_ui_text(&chip_text, tokens::MICRO, true) + 16;
        let chip_x = px + 240;
        let chip_y = py + (header_h.saturating_sub(20)) / 2;
        if chip_x + chip_w < layout.filter_all_btn.0.saturating_sub(12) {
            canvas.rounded_fill(
                chip_x,
                chip_y,
                chip_w,
                20,
                tokens::RADIUS_ROW,
                theme.mix(theme.background, theme.ansi[2], 0.22),
            );
            let chip_baseline = self.centered_baseline(chip_y + 10);
            self.draw_ui_text(
                canvas,
                &chip_text,
                chip_x + 8,
                chip_baseline,
                chip_x + chip_w - 4,
                theme.ansi[2],
                tokens::MICRO,
                true,
            );
        }

        // Segmentos de filtro: Todos / IA / Compilación
        let filter_btns = [
            (
                layout.filter_all_btn,
                "Todos",
                ProcessFilterKind::All,
                SystemProcessFilter::All,
            ),
            (
                layout.filter_ai_btn,
                "IA",
                ProcessFilterKind::Ai,
                SystemProcessFilter::Ai,
            ),
            (
                layout.filter_build_btn,
                "Compilación",
                ProcessFilterKind::Build,
                SystemProcessFilter::Build,
            ),
        ];

        for (rect, text, kind, sys_filter) in filter_btns {
            let (bx, by, bw, bh) = rect;
            let is_active = monitor.process_filter == kind;
            let is_h = hovered == Some(SystemClickTarget::ProcessFilter(sys_filter));

            let bg = if is_active {
                theme.accent_color()
            } else if is_h {
                theme.surface_hover()
            } else {
                Color::TRANSPARENT
            };
            canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
            canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_ROW, theme.chrome_line());

            let fg = if is_active {
                theme.background
            } else if is_h {
                theme.foreground
            } else {
                theme.text_secondary()
            };
            let tw = self.measure_ui_text(text, tokens::MICRO, true);
            let tx = bx + (bw.saturating_sub(tw)) / 2;
            let tb = self.centered_baseline(by + bh / 2);
            self.draw_ui_text(canvas, text, tx, tb, bx + bw - 2, fg, tokens::MICRO, true);
        }

        // 2. Encabezado de la tabla de procesos
        let table_header_y = py + header_h;
        let table_header_h = SystemLayoutRects::PROCESS_HEADER_H;
        Self::draw_rect(
            canvas,
            px,
            table_header_y + table_header_h - 1,
            pw,
            1,
            theme.chrome_line(),
        );

        let col_baseline = self.centered_baseline(table_header_y + table_header_h / 2);
        self.draw_ui_text(
            canvas,
            "PROCESO",
            px + 14,
            col_baseline,
            px + pw - 280,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );

        let col_gpu_x = px + pw - 50;
        let col_ram_x = col_gpu_x - 70;
        let col_cpu_x = col_ram_x - 60;
        let col_pid_x = col_cpu_x - 60;

        self.draw_ui_text(
            canvas,
            "PID",
            col_pid_x,
            col_baseline,
            col_cpu_x - 6,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );
        self.draw_ui_text(
            canvas,
            "CPU",
            col_cpu_x,
            col_baseline,
            col_ram_x - 6,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );
        self.draw_ui_text(
            canvas,
            "RAM",
            col_ram_x,
            col_baseline,
            col_gpu_x - 6,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );
        self.draw_ui_text(
            canvas,
            "GPU",
            col_gpu_x,
            col_baseline,
            px + pw - 10,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );

        // 3. Lista de filas de procesos filtrados
        let filtered_procs = monitor.filtered_processes(snap);

        let list_top = layout.process_list_rect.1;
        let max_rows = layout.max_visible_process_rows();
        let scroll = monitor
            .process_scroll
            .min(filtered_procs.len().saturating_sub(max_rows));

        if filtered_procs.is_empty() {
            let msg = match &monitor.selected_source {
                MonitorSource::Local => "No hay procesos activos en esta categoría",
                MonitorSource::Remote(_) => {
                    "Para gestionar procesos remotos, conéctate mediante una terminal SSH"
                }
            };
            let empty_b = self.centered_baseline(list_top + 40);
            self.draw_ui_text(
                canvas,
                msg,
                px + 20,
                empty_b,
                px + pw - 20,
                theme.text_secondary(),
                tokens::BODY,
                false,
            );
        }

        for (i, proc) in filtered_procs
            .iter()
            .skip(scroll)
            .take(max_rows)
            .enumerate()
        {
            let ry = list_top + i * SystemLayoutRects::PROCESS_ROW_H;
            let rh = SystemLayoutRects::PROCESS_ROW_H;
            let is_sel = monitor.selected_process_pid == Some(proc.pid);
            let is_h = hovered == Some(SystemClickTarget::ProcessRow(i));

            if is_sel {
                canvas.rounded_fill(
                    px + 1,
                    ry,
                    pw.saturating_sub(2),
                    rh,
                    tokens::RADIUS_ROW,
                    theme.mix(theme.background, theme.accent_color(), 0.16),
                );
                // Borde izquierdo azul de foco/selección
                Self::draw_rect(canvas, px, ry, 2, rh, theme.accent_color());
            } else if is_h {
                canvas.rounded_fill(
                    px + 1,
                    ry,
                    pw.saturating_sub(2),
                    rh,
                    tokens::RADIUS_ROW,
                    theme.surface_hover(),
                );
            }
            Self::draw_rect(canvas, px, ry + rh - 1, pw, 1, theme.chrome_line());

            let row_baseline = self.centered_baseline(ry + rh / 2);

            // Punto de estado según tipo de proceso
            let dot_color = match proc.kind {
                ProcessKind::Tearminal | ProcessKind::Shell => theme.ansi[4],
                ProcessKind::AiAgent => theme.ansi[5],
                ProcessKind::Build => theme.ansi[3],
                ProcessKind::Other => theme.ansi[6],
            };
            canvas.rounded_fill(
                px + 14,
                ry + (rh.saturating_sub(7)) / 2,
                7,
                7,
                tokens::RADIUS_ROW,
                dot_color,
            );

            // Nombre y nota del proceso
            let name_x = px + 28;
            let name_w = self.measure_ui_text(&proc.name, tokens::BODY, true);
            self.draw_ui_text(
                canvas,
                &proc.name,
                name_x,
                row_baseline,
                col_pid_x.saturating_sub(12),
                theme.foreground,
                tokens::BODY,
                true,
            );

            let note_x = name_x + name_w + 10;
            if note_x < col_pid_x.saturating_sub(16) {
                self.draw_ui_text(
                    canvas,
                    &proc.note,
                    note_x,
                    row_baseline,
                    col_pid_x.saturating_sub(16),
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
            }

            // PID
            let pid_str = proc.pid.to_string();
            self.draw_mono_text(
                canvas,
                &pid_str,
                col_pid_x,
                row_baseline,
                col_cpu_x - 6,
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );

            // CPU (si es alta > 20%, destacar en amarillo)
            let cpu_str = format!("{:.1}%", proc.cpu_pct);
            let cpu_fg = if proc.cpu_pct >= 20.0 {
                theme.ansi[3]
            } else {
                theme.foreground
            };
            self.draw_mono_text(
                canvas,
                &cpu_str,
                col_cpu_x,
                row_baseline,
                col_ram_x - 6,
                cpu_fg,
                tokens::DETAIL,
                proc.cpu_pct >= 20.0,
            );

            // RAM
            let ram_str = format_bytes(proc.ram_bytes);
            self.draw_mono_text(
                canvas,
                &ram_str,
                col_ram_x,
                row_baseline,
                col_gpu_x - 6,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // GPU
            let gpu_str = if proc.gpu_pct > 0.0 {
                format!("{:.0}%", proc.gpu_pct)
            } else {
                "—".to_string()
            };
            self.draw_mono_text(
                canvas,
                &gpu_str,
                col_gpu_x,
                row_baseline,
                px + pw - 10,
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );
        }

        // 4. Pie de la tarjeta de procesos: botones de acción
        let footer_y = py + ph.saturating_sub(SystemLayoutRects::PROCESS_FOOTER_H);
        let footer_h = SystemLayoutRects::PROCESS_FOOTER_H;
        canvas.rounded_fill(
            px,
            footer_y,
            pw,
            footer_h,
            tokens::RADIUS_CARD,
            theme.mantle(),
        );
        Self::draw_rect(canvas, px, footer_y, pw, 1, theme.chrome_line());

        let footer_baseline = self.centered_baseline(footer_y + footer_h / 2);
        let selected_proc = monitor
            .selected_process_pid
            .and_then(|pid| snap.processes.iter().find(|p| p.pid == pid));

        let sel_msg = match selected_proc {
            Some(p) => format!("Seleccionado: {} · PID {}", p.name, p.pid),
            None => "Ningún proceso seleccionado".to_string(),
        };
        self.draw_ui_text(
            canvas,
            &sel_msg,
            px + 14,
            footer_baseline,
            px + pw.saturating_sub(280),
            theme.text_secondary(),
            tokens::DETAIL,
            false,
        );

        if let Some(btn) = layout.view_in_terminal_btn {
            let is_h = hovered == Some(SystemClickTarget::ViewInTerminal);
            self.draw_button(canvas, btn, "Ver en terminal", is_h, false, theme);
        }

        if let Some(btn) = layout.kill_process_btn {
            let is_h = hovered == Some(SystemClickTarget::KillProcess);
            let (bx, by, bw, bh) = btn;
            let bg = if is_h {
                theme.mix(theme.background, theme.ansi[1], 0.25)
            } else {
                theme.background
            };
            canvas.rounded_fill(bx, by, bw, bh, tokens::RADIUS_ROW, bg);
            canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_ROW, theme.ansi[1]);

            let tw = self.measure_ui_text("Terminar proceso…", tokens::BODY, true);
            let tx = bx + (bw.saturating_sub(tw)) / 2;
            let tb = self.centered_baseline(by + bh / 2);
            self.draw_ui_text(
                canvas,
                "Terminar proceso…",
                tx,
                tb,
                bx + bw - 2,
                theme.ansi[1],
                tokens::BODY,
                true,
            );
        }
    }

    /// Renderiza la tarjeta de CPU por núcleo
    fn render_system_cores_card(
        &mut self,
        canvas: &mut Canvas,
        layout: &SystemLayoutRects,
        snap: &SystemSnapshot,
        theme: &Theme,
    ) {
        let (cx, cy, cw, ch) = layout.cores_card;
        if cw == 0 || ch == 0 {
            return;
        }

        // Fondo y contorno
        canvas.rounded_fill(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.background);
        canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CARD, theme.card_outline());

        // Cabecera
        let header_h = SystemLayoutRects::CARD_HEADER_H;
        canvas.rounded_fill(cx, cy, cw, header_h, tokens::RADIUS_CARD, theme.mantle());
        Self::draw_rect(canvas, cx, cy + header_h - 1, cw, 1, theme.chrome_line());

        let title_baseline = self.centered_baseline(cy + header_h / 2);
        self.draw_ui_text(
            canvas,
            "CPU POR NÚCLEO",
            cx + 14,
            title_baseline,
            cx + 150,
            theme.foreground,
            tokens::MICRO + 1.0,
            true,
        );

        let sub_info = format!(
            "{} hilos · {:.1} GHz",
            snap.cpu_thread_count, snap.cpu_freq_ghz
        );
        let sub_w = self.measure_mono_text(&sub_info, tokens::MICRO, false);
        let sub_x = (cx + cw).saturating_sub(sub_w + 14);
        self.draw_mono_text(
            canvas,
            &sub_info,
            sub_x,
            title_baseline,
            cx + cw - 10,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );

        // Grid de 2 columnas de núcleos
        let content_y = cy + header_h + 10;
        let col_w = (cw.saturating_sub(36)) / 2;
        let col1_x = cx + 14;
        let col2_x = col1_x + col_w + 14;

        let cores = &snap.cpu_cores;
        let half = cores.len().div_ceil(2);

        for (i, core) in cores.iter().enumerate() {
            let is_col2 = i >= half;
            let row_idx = if is_col2 { i - half } else { i };
            let bx = if is_col2 { col2_x } else { col1_x };
            let by = content_y + row_idx * 18;
            if by + 16 > cy + ch {
                break;
            }

            let baseline = self.centered_baseline(by + 8);
            let lbl = format!("N{}", core.index);
            self.draw_mono_text(
                canvas,
                &lbl,
                bx,
                baseline,
                bx + 26,
                theme.text_secondary(),
                tokens::MICRO,
                false,
            );

            // Barra horizontal
            let bar_x = bx + 28;
            let bar_w = col_w.saturating_sub(64);
            let bar_h = 6;
            let bar_y = by + (16usize.saturating_sub(bar_h)) / 2;
            let color = if core.percent >= 40.0 {
                theme.ansi[3]
            } else {
                theme.ansi[4]
            };
            draw_mini_bar(
                canvas,
                bar_x,
                bar_y,
                bar_w,
                bar_h,
                core.percent,
                color,
                theme.chrome_line(),
            );

            // Porcentaje numérico
            let pct_str = format!("{:.0}%", core.percent);
            self.draw_mono_text(
                canvas,
                &pct_str,
                bar_x + bar_w + 6,
                baseline,
                bx + col_w,
                theme.foreground,
                tokens::MICRO,
                false,
            );
        }
    }

    /// Renderiza la tarjeta de almacenamiento con volúmenes y carpetas pesadas
    fn render_system_storage_card(
        &mut self,
        canvas: &mut Canvas,
        layout: &SystemLayoutRects,
        snap: &SystemSnapshot,
        hovered: Option<SystemClickTarget>,
        theme: &Theme,
    ) {
        let (sx, sy, sw, sh) = layout.storage_card;
        if sw == 0 || sh == 0 {
            return;
        }

        // Fondo y contorno
        canvas.rounded_fill(sx, sy, sw, sh, tokens::RADIUS_CARD, theme.background);
        canvas.rounded_stroke(sx, sy, sw, sh, tokens::RADIUS_CARD, theme.card_outline());

        // Cabecera
        let header_h = SystemLayoutRects::CARD_HEADER_H;
        canvas.rounded_fill(sx, sy, sw, header_h, tokens::RADIUS_CARD, theme.mantle());
        Self::draw_rect(canvas, sx, sy + header_h - 1, sw, 1, theme.chrome_line());

        let title_baseline = self.centered_baseline(sy + header_h / 2);
        self.draw_ui_text(
            canvas,
            "ALMACENAMIENTO",
            sx + 14,
            title_baseline,
            sx + 150,
            theme.foreground,
            tokens::MICRO + 1.0,
            true,
        );

        let vol_count = format!("{} volúmenes", snap.volumes.len().max(1));
        let vol_w = self.measure_mono_text(&vol_count, tokens::MICRO, false);
        let vol_x = (sx + sw).saturating_sub(vol_w + 14);
        self.draw_mono_text(
            canvas,
            &vol_count,
            vol_x,
            title_baseline,
            sx + sw - 10,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );

        // Volúmenes de disco (hasta 2)
        let mut cur_y = sy + header_h + 12;
        for vol in snap.volumes.iter().take(2) {
            let row_baseline = self.centered_baseline(cur_y + 8);
            let name_str = format!("{} · {}", vol.letter, vol.label);
            self.draw_ui_text(
                canvas,
                &name_str,
                sx + 14,
                row_baseline,
                sx + sw - 120,
                theme.foreground,
                tokens::BODY,
                true,
            );

            let used_gb = vol.total_bytes.saturating_sub(vol.free_bytes) as f32 / 1_073_741_824.0;
            let tot_gb = vol.total_bytes as f32 / 1_073_741_824.0;
            let use_str = format!("{:.0} / {:.0} GB", used_gb, tot_gb);
            let use_w = self.measure_mono_text(&use_str, tokens::DETAIL, false);
            let use_x = (sx + sw).saturating_sub(use_w + 14);
            self.draw_mono_text(
                canvas,
                &use_str,
                use_x,
                row_baseline,
                sx + sw - 10,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // Barra de volumen
            let bar_y = cur_y + 18;
            let bar_w = sw.saturating_sub(28);
            let bar_color = if vol.used_pct >= 75.0 {
                theme.ansi[3]
            } else {
                theme.ansi[4]
            };
            draw_mini_bar(
                canvas,
                sx + 14,
                bar_y,
                bar_w,
                6,
                vol.used_pct,
                bar_color,
                theme.chrome_line(),
            );

            cur_y += 36;
        }

        // Subsección: "Ocupan más en este proyecto"
        cur_y = SystemLayoutRects::folders_start_y(layout.storage_card, snap.volumes.len());
        let sec_baseline = self.centered_baseline(cur_y.saturating_sub(10));
        self.draw_ui_text(
            canvas,
            "OCUPAN MÁS EN ESTE PROYECTO",
            sx + 14,
            sec_baseline,
            sx + sw - 14,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );

        for (i, folder) in snap.top_project_folders.iter().enumerate() {
            if cur_y + 30 > sy + sh {
                break;
            }
            Self::draw_rect(
                canvas,
                sx + 14,
                cur_y + 29,
                sw.saturating_sub(28),
                1,
                theme.chrome_line(),
            );

            let row_baseline = self.centered_baseline(cur_y + 15);
            self.draw_mono_text(
                canvas,
                &folder.name,
                sx + 14,
                row_baseline,
                sx + sw - 160,
                theme.foreground,
                tokens::DETAIL,
                true,
            );

            let sz_str = format_bytes(folder.size_bytes);
            self.draw_mono_text(
                canvas,
                &sz_str,
                sx + sw.saturating_sub(150),
                row_baseline,
                sx + sw.saturating_sub(90),
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // Botón de acción (ej. "cargo clean", "Limpiar")
            if let Some(act_label) = folder.action.as_deref() {
                let btn_rect =
                    SystemLayoutRects::folder_action_btn_rect(layout.storage_card, cur_y, 80, 24);
                let is_h = hovered == Some(SystemClickTarget::FolderAction(i));
                self.draw_button(canvas, btn_rect, act_label, is_h, false, theme);
            }

            cur_y += 30;
        }
    }

    /// Renderiza el panel contextual de Sistema cuando `active_module == AppModule::System`
    pub(crate) fn render_system_sidebar(
        &mut self,
        canvas: &mut Canvas,
        sb_rects: ContextualSidebarRects,
        monitor: &MonitorState,
        hovered_target: Option<SystemClickTarget>,
        theme: &Theme,
    ) {
        let (sx, sy, sw, sh) = (
            sb_rects.content.0,
            sb_rects.content.1,
            sb_rects.content.2,
            sb_rects.content.3,
        );
        if sw == 0 || sh == 0 {
            return;
        }

        let geom = SystemSidebarRects::compute(sx, sy, sw, sh);

        // 1. Botón "+" Añadir máquina en la cabecera
        let is_add_h = hovered_target == Some(SystemClickTarget::SidebarAddMachine);
        let (ax, ay, aw, ah) = geom.add_machine_btn;
        if is_add_h {
            canvas.rounded_fill(ax, ay, aw, ah, tokens::RADIUS_ROW, theme.surface_hover());
        }
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Plus,
            ax + 8,
            ay + 8,
            12.0,
            if is_add_h {
                theme.foreground
            } else {
                theme.text_secondary()
            },
        );

        // 2. Fuentes de monitorización (Local y Equipos remotos)
        let mut cur_y = geom.list_rect.1 + 4;

        // Sección Local
        let loc_baseline = self.centered_baseline(cur_y + 10);
        self.draw_ui_text(
            canvas,
            "LOCAL",
            sx + 14,
            loc_baseline,
            sx + sw - 14,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );

        // Fila "Este equipo"
        let is_local_sel = matches!(monitor.selected_source, MonitorSource::Local);
        let is_local_h = hovered_target == Some(SystemClickTarget::SidebarSource(0));

        let local_rect = geom.source_item_rect(0);
        cur_y = local_rect.1;
        if is_local_sel {
            canvas.rounded_fill(
                local_rect.0,
                local_rect.1,
                local_rect.2,
                local_rect.3,
                tokens::RADIUS_ROW,
                theme.mix(theme.background, theme.accent_color(), 0.16),
            );
            Self::draw_rect(
                canvas,
                local_rect.0,
                local_rect.1,
                2,
                local_rect.3,
                theme.accent_color(),
            );
        } else if is_local_h {
            canvas.rounded_fill(
                local_rect.0,
                local_rect.1,
                local_rect.2,
                local_rect.3,
                tokens::RADIUS_ROW,
                theme.surface_hover(),
            );
        }

        let row1_b = self.centered_baseline(cur_y + 14);
        canvas.rounded_fill(sx + 14, cur_y + 10, 7, 7, tokens::RADIUS_ROW, theme.ansi[2]);
        self.draw_ui_text(
            canvas,
            "Este equipo",
            sx + 28,
            row1_b,
            sx + sw - 80,
            theme.foreground,
            tokens::BODY,
            true,
        );
        self.draw_mono_text(
            canvas,
            "Windows",
            sx + sw - 70,
            row1_b,
            sx + sw - 14,
            theme.text_muted(),
            tokens::MICRO,
            false,
        );

        // Mini barras de CPU, RAM, GPU
        let fallback_snap = SystemSnapshot::default();
        let snap = monitor.snapshot.as_ref().unwrap_or(&fallback_snap);
        let mb_w = (sw.saturating_sub(44)) / 3;
        let mb_y = cur_y + 34;

        // CPU mini bar
        draw_mini_bar_with_label(
            self,
            canvas,
            sx + 14,
            mb_y,
            mb_w,
            "CPU",
            snap.cpu_total,
            theme.ansi[4],
            theme,
        );

        // RAM mini bar
        draw_mini_bar_with_label(
            self,
            canvas,
            sx + 14 + mb_w + 6,
            mb_y,
            mb_w,
            "RAM",
            snap.ram_pct,
            theme.ansi[2],
            theme,
        );

        // GPU mini bar
        draw_mini_bar_with_label(
            self,
            canvas,
            sx + 14 + (mb_w + 6) * 2,
            mb_y,
            mb_w,
            "GPU",
            snap.gpu_usage_pct,
            theme.ansi[5],
            theme,
        );

        cur_y += SystemSidebarRects::ITEM_H + 12;

        // Sección Equipos remotos
        let rem_baseline = self.centered_baseline(cur_y + 10);
        self.draw_ui_text(
            canvas,
            "EQUIPOS REMOTOS",
            sx + 14,
            rem_baseline,
            sx + sw - 14,
            theme.text_muted(),
            tokens::MICRO,
            true,
        );
        cur_y += 24;

        if monitor.remote_snapshots.is_empty() {
            let empty_b = self.centered_baseline(cur_y + 14);
            self.draw_ui_text(
                canvas,
                "Sin máquinas remotas",
                sx + 14,
                empty_b,
                sx + sw - 14,
                theme.text_muted(),
                tokens::DETAIL,
                false,
            );
        } else {
            for (idx, (dest, remote)) in monitor.remote_snapshots.iter().enumerate() {
                let is_sel =
                    matches!(&monitor.selected_source, MonitorSource::Remote(d) if d == dest);
                let is_h = hovered_target == Some(SystemClickTarget::SidebarSource(idx + 1));
                let item_rect = geom.source_item_rect(idx + 1);
                let item_y = item_rect.1;

                if is_sel {
                    canvas.rounded_fill(
                        item_rect.0,
                        item_rect.1,
                        item_rect.2,
                        item_rect.3,
                        tokens::RADIUS_ROW,
                        theme.mix(theme.background, theme.accent_color(), 0.16),
                    );
                    Self::draw_rect(
                        canvas,
                        item_rect.0,
                        item_rect.1,
                        2,
                        item_rect.3,
                        theme.accent_color(),
                    );
                } else if is_h {
                    canvas.rounded_fill(
                        item_rect.0,
                        item_rect.1,
                        item_rect.2,
                        item_rect.3,
                        tokens::RADIUS_ROW,
                        theme.surface_hover(),
                    );
                }

                let dot_c = if remote.is_connected {
                    theme.ansi[3]
                } else {
                    theme.text_muted()
                };
                let row_b = self.centered_baseline(item_y + 14);
                canvas.rounded_fill(sx + 14, item_y + 10, 7, 7, tokens::RADIUS_ROW, dot_c);
                self.draw_ui_text(
                    canvas,
                    dest,
                    sx + 28,
                    row_b,
                    sx + sw - 90,
                    theme.foreground,
                    tokens::BODY,
                    true,
                );

                let meta_str = if remote.is_connected {
                    "ssh · 38 ms"
                } else {
                    "sin conexión"
                };
                self.draw_mono_text(
                    canvas,
                    meta_str,
                    sx + sw - 86,
                    row_b,
                    sx + sw - 14,
                    theme.text_muted(),
                    tokens::MICRO,
                    false,
                );

                // Mini barras remotas: CPU, RAM, DISCO
                let rmb_y = item_y + 34;
                draw_mini_bar_with_label(
                    self,
                    canvas,
                    sx + 14,
                    rmb_y,
                    mb_w,
                    "CPU",
                    remote.cpu_pct,
                    theme.ansi[4],
                    theme,
                );
                draw_mini_bar_with_label(
                    self,
                    canvas,
                    sx + 14 + mb_w + 6,
                    rmb_y,
                    mb_w,
                    "RAM",
                    remote.ram_pct,
                    theme.ansi[2],
                    theme,
                );
                draw_mini_bar_with_label(
                    self,
                    canvas,
                    sx + 14 + (mb_w + 6) * 2,
                    rmb_y,
                    mb_w,
                    "DISCO",
                    remote.disk_pct,
                    theme.ansi[3],
                    theme,
                );
            }
        }

        // 3. Botón de pie: "Umbrales y alertas"
        let is_thresh_h = hovered_target == Some(SystemClickTarget::SidebarThresholds);
        self.draw_button(
            canvas,
            geom.thresholds_btn,
            "Umbrales y alertas",
            is_thresh_h,
            false,
            theme,
        );
    }
}

// ---------------------------------------------------------------------------
// Funciones auxiliares de dibujo
// ---------------------------------------------------------------------------

fn draw_sparkline(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    samples: &std::collections::VecDeque<f32>,
    color: Color,
) {
    if samples.is_empty() || w == 0 || h == 0 {
        return;
    }
    let count = samples.len();
    let gap = 2usize;
    let bar_w = ((w + gap) / count).saturating_sub(gap).max(1);

    for (i, &sample) in samples.iter().enumerate() {
        let bx = x + i * (bar_w + gap);
        if bx + bar_w > x + w {
            break;
        }
        let pct = (sample / 100.0).clamp(0.04, 1.0);
        let bar_h = ((h as f32 * pct).round() as usize).clamp(3, h);
        let by = (y + h).saturating_sub(bar_h);

        // Gradiente sutil de opacidad: más nítido en muestras recientes (a la derecha)
        let alpha = if i > 30 {
            255
        } else {
            100 + ((i as f32 / 30.0) * 155.0).round() as u8
        };
        let c = Color::from_rgba(color.r, color.g, color.b, alpha);
        canvas.rounded_fill(bx, by, bar_w, bar_h, tokens::RADIUS_ROW, c);
    }
}

fn draw_mini_bar(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    pct: f32,
    color: Color,
    track_color: Color,
) {
    canvas.rounded_fill(x, y, w, h, tokens::RADIUS_ROW, track_color);
    let fill_w = ((w as f32 * (pct / 100.0).clamp(0.0, 1.0)).round() as usize).min(w);
    if fill_w > 0 {
        canvas.rounded_fill(x, y, fill_w, h, tokens::RADIUS_ROW, color);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_mini_bar_with_label(
    renderer: &mut TerminalRenderer,
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    w: usize,
    label: &str,
    pct: f32,
    color: Color,
    theme: &Theme,
) {
    draw_mini_bar(canvas, x, y, w, 4, pct, color, theme.chrome_line());
    let txt = format!("{} {:.0}%", label, pct);
    let baseline = renderer.centered_baseline(y + 12);
    renderer.draw_mono_text(
        canvas,
        &txt,
        x,
        baseline,
        x + w,
        theme.text_muted(),
        tokens::MICRO - 1.0,
        false,
    );
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

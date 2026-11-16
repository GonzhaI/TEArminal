//! Barra de estado inferior.

use super::*;

/// Hueco entre la etiqueta de un medidor y su carril.
const METER_LABEL_GAP: usize = 8;
/// Hueco entre el carril y el porcentaje.
const METER_PCT_GAP: usize = 8;
/// Separación entre medidores.
const METER_GAP: usize = 17;
/// Cuerpo del porcentaje: un paso por debajo de la etiqueta, como en el bosquejo.
const METER_PCT_SIZE: f32 = 9.0;
/// Etiquetas del grupo de estado (badge y medidores): los 12 px del bosquejo, a la escala por
/// defecto de `ui_font_size`.
const STATUS_LABEL_SIZE: f32 = 10.5;

impl TerminalRenderer {
    /// Renderiza la barra de estado informativa inferior (Status Bar) con tipografía Barlow e iconos Lucide
    #[cfg(test)]
    pub(crate) fn render_status_bar(
        &mut self,
        canvas: &mut Canvas,
        sidebar_offset: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        cols: usize,
        rows: usize,
        ai_status: AiAgentStatus,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        self.render_status_bar_with_ai(
            canvas,
            sidebar_offset,
            cwd,
            git_branch,
            ssh_host,
            cols,
            rows,
            ai_status,
            None,
            &[],
            false,
            explorer_visible,
            theme,
        );
    }

    pub(crate) fn keep_awake_button_width(&self, mode: KeepAwakeMode) -> usize {
        let label_w = self.measure_ui_text(mode.label(), STATUS_LABEL_SIZE, true);
        label_w + 26
    }

    #[cfg(test)]
    pub(crate) fn render_status_bar_with_ai(
        &mut self,
        canvas: &mut Canvas,
        sidebar_offset: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        cols: usize,
        rows: usize,
        ai_status: AiAgentStatus,
        active_model: Option<&str>,
        meters: &[StatusMeter],
        hovered_ai_badge: bool,
        explorer_visible: bool,
        theme: &Theme,
    ) {
        self.render_status_bar_full(
            canvas,
            sidebar_offset,
            cwd,
            git_branch,
            ssh_host,
            cols,
            rows,
            ai_status,
            active_model,
            meters,
            KeepAwakeMode::Off,
            false,
            false,
            hovered_ai_badge,
            explorer_visible,
            theme,
        );
    }

    pub(crate) fn render_status_bar_full(
        &mut self,
        canvas: &mut Canvas,
        _left_x: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        cols: usize,
        rows: usize,
        ai_status: AiAgentStatus,
        active_model: Option<&str>,
        meters: &[StatusMeter],
        keep_awake: KeepAwakeMode,
        keep_awake_working: bool,
        hovered_keep_awake: bool,
        _hovered_ai_badge: bool,
        _explorer_visible: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width == 0 || height < StatusBarRects::BAR_H {
            return;
        }

        let bar_h = StatusBarRects::BAR_H;
        let bar_y = height - bar_h;
        let center_y = bar_y + (bar_h / 2);
        let baseline = self.centered_baseline(center_y);

        // 1. Fondo completo de la barra de estado y línea superior divisoria (DISENO §4)
        Self::draw_rect(canvas, 0, bar_y, width, bar_h, theme.bar_bg());
        Self::draw_rect(canvas, 0, bar_y, width, 1, theme.chrome_line());

        // 2. Cálculo de métricas y segmentos
        let (badge_icon, badge_label, ai_fg) =
            Self::status_bar_badge(ai_status, ssh_host, active_model, theme);
        let ai_text_w = if badge_label.is_empty() {
            0
        } else {
            self.measure_ui_text(&badge_label, STATUS_LABEL_SIZE, true)
        };
        let update_info = crate::updater::get_available_update();
        let update_text = update_info
            .as_ref()
            .map(|u| format!("v{} disponible", u.latest_version));
        let update_w = update_text
            .as_ref()
            .map(|t| self.measure_mono_text(t, tokens::MICRO + 0.5, false));
        let (dims_w, dims_str) = if self.show_dimensions {
            let dims = format!("{cols} × {rows}");
            let w = self.measure_mono_text(&dims, tokens::MICRO, false);
            (w, Some(dims))
        } else {
            (0, None)
        };
        let meters_w = self.status_meters_width(meters);
        let kw_w = self.keep_awake_button_width(keep_awake);
        let rects = StatusBarRects::compute_full(
            width,
            height,
            ai_text_w,
            update_w,
            dims_w,
            meters_w,
            kw_w,
            git_branch.is_some(),
        );

        // --- SEGMENTOS IZQUIERDOS (entorno, rama, ruta) con separadores de 1 px ---
        // Segmento 1: Entorno (LOCAL o SSH host)
        let env_r = rects.env_rect;
        let is_remote = ssh_host.is_some();
        let env_label = ssh_host.unwrap_or("LOCAL");
        let dot_color = if is_remote {
            theme.ansi[3]
        } else {
            theme.ansi[2]
        };
        let dot_y = (bar_h.saturating_sub(7)) / 2;
        Self::draw_rect(canvas, env_r.0 + 12, bar_y + dot_y, 7, 7, dot_color);
        self.draw_ui_text(
            canvas,
            env_label,
            env_r.0 + 25,
            baseline,
            env_r.0 + env_r.2.saturating_sub(8),
            theme.foreground,
            STATUS_LABEL_SIZE,
            true,
        );
        Self::draw_rect(
            canvas,
            env_r.0 + env_r.2 - 1,
            bar_y,
            1,
            bar_h,
            theme.chrome_line(),
        );

        // Segmento 2: Rama Git (si existe)
        if let (Some(br_rect), Some(branch)) = (rects.branch_rect, git_branch) {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::GitBranch,
                br_rect.0 + 16,
                center_y,
                11.0,
                theme.text_secondary(),
            );
            self.draw_mono_text(
                canvas,
                branch,
                br_rect.0 + 26,
                baseline,
                br_rect.0 + br_rect.2.saturating_sub(8),
                theme.text_secondary(),
                tokens::DETAIL,
                true,
            );
            Self::draw_rect(
                canvas,
                br_rect.0 + br_rect.2 - 1,
                bar_y,
                1,
                bar_h,
                theme.chrome_line(),
            );
        }

        // Segmento 3: Ruta actual (cwd)
        if let (Some(p_rect), Some(dir)) = (rects.path_rect, cwd) {
            self.draw_mono_text(
                canvas,
                dir,
                p_rect.0 + 12,
                baseline,
                p_rect.0 + p_rect.2.saturating_sub(8),
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            Self::draw_rect(
                canvas,
                p_rect.0 + p_rect.2 - 1,
                bar_y,
                1,
                bar_h,
                theme.chrome_line(),
            );
        }

        // --- SEGMENTOS DERECHOS con bordes izquierdos de 1 px ---
        // Dimensiones (cols × rows)
        if let (ref dims_r, Some(ref dims)) = (rects.dims_rect, dims_str) {
            if dims_r.2 > 0 {
                Self::draw_rect(canvas, dims_r.0, bar_y, 1, bar_h, theme.chrome_line());
                let dw = self.measure_mono_text(dims, tokens::MICRO, false);
                let dx = dims_r.0 + (dims_r.2.saturating_sub(dw)) / 2;
                self.draw_mono_text(
                    canvas,
                    dims,
                    dx,
                    baseline,
                    dims_r.0 + dims_r.2,
                    theme.text_muted(),
                    tokens::MICRO,
                    false,
                );
            }
        }

        // Medidores / botón del monitor de sistema (F1/F3)
        if rects.meters.2 > 0 {
            Self::draw_rect(canvas, rects.meters.0, bar_y, 1, bar_h, theme.chrome_line());
            if meters.is_empty() {
                // Hueco reservado para monitor de rendimiento (DISENO §4): etiqueta SISTEMA con icono
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Sliders,
                    rects.meters.0 + 14,
                    center_y,
                    12.0,
                    theme.text_muted(),
                );
                let sys_label = "SISTEMA";
                self.draw_ui_text(
                    canvas,
                    sys_label,
                    rects.meters.0 + 24,
                    baseline,
                    rects.meters.0 + rects.meters.2.saturating_sub(6),
                    theme.text_muted(),
                    tokens::MICRO + 1.0,
                    true,
                );
            } else {
                self.draw_status_meters(
                    canvas,
                    meters,
                    rects.meters.0 + rects.meters.2.saturating_sub(12),
                    (bar_y, bar_h),
                    baseline,
                    ai_fg,
                    theme,
                );
            }
        }

        // Chip de IA (solo con agente activo: el entorno Local/SSH ya aparece en el segmento izquierdo)
        if rects.ai_badge.2 > 0 {
            let (bx, _by, bw, _bh) = rects.ai_badge;
            Self::draw_rect(canvas, bx, bar_y, 1, bar_h, theme.chrome_line());
            self.draw_lucide_icon(
                canvas,
                badge_icon.unwrap_or(crate::ui::LucideIcon::Sparkles),
                bx + 14,
                center_y,
                12.0,
                ai_fg,
            );
            self.draw_ui_text(
                canvas,
                &badge_label,
                bx + 28,
                baseline,
                bx + bw.saturating_sub(6),
                theme.foreground,
                STATUS_LABEL_SIZE,
                true,
            );
        }

        // Actualización disponible (si existe)
        if let (Some((ub_x, _ub_y, ub_w, _ub_h)), Some(utext)) = (rects.update_badge, update_text) {
            Self::draw_rect(canvas, ub_x, bar_y, 1, bar_h, theme.chrome_line());
            Self::draw_rect(canvas, ub_x + 10, bar_y + dot_y, 7, 7, theme.ansi[3]);
            self.draw_mono_text(
                canvas,
                &utext,
                ub_x + 22,
                baseline,
                ub_x + ub_w.saturating_sub(6),
                theme.ansi[3],
                tokens::MICRO + 0.5,
                false,
            );
        }

        // Selector Keep-awake
        let (kw_x, _kw_y, kw_btn_w, _kw_btn_h) = rects.keep_awake_btn;
        if kw_btn_w > 0 {
            Self::draw_rect(canvas, kw_x, bar_y, 1, bar_h, theme.chrome_line());
            let (icon, icon_fg, mut text_fg) = match keep_awake {
                KeepAwakeMode::Active => (
                    crate::ui::LucideIcon::Coffee,
                    theme.accent_color(),
                    theme.foreground,
                ),
                KeepAwakeMode::Agent => {
                    let fg = if keep_awake_working {
                        theme.ansi[6]
                    } else {
                        theme.text_secondary()
                    };
                    (crate::ui::LucideIcon::Sparkles, fg, theme.foreground)
                }
                KeepAwakeMode::Off => (
                    crate::ui::LucideIcon::Eye,
                    theme.text_muted(),
                    theme.text_muted(),
                ),
            };
            if hovered_keep_awake {
                text_fg = theme.foreground;
            }

            self.draw_lucide_icon(canvas, icon, kw_x + 14, center_y, 11.0, icon_fg);
            self.draw_ui_text(
                canvas,
                keep_awake.label(),
                kw_x + 28,
                baseline,
                kw_x + kw_btn_w.saturating_sub(6),
                text_fg,
                STATUS_LABEL_SIZE,
                true,
            );
        }
    }

    /// Chip derecho de la barra de estado: muestra únicamente el agente/modelo de IA cuando está
    /// activo, evitando repetir el entorno («LOCAL» / host SSH) que ya vive en el segmento izquierdo.
    pub(crate) fn status_bar_badge(
        ai_status: AiAgentStatus,
        ssh_host: Option<&str>,
        active_model: Option<&str>,
        theme: &Theme,
    ) -> (Option<crate::ui::LucideIcon>, String, Color) {
        if ai_status == AiAgentStatus::None {
            return (None, String::new(), theme.text_muted());
        }
        let (icon, label, fg) =
            Self::status_bar_badge_info(ai_status, ssh_host, active_model, theme);
        (Some(icon), label, fg)
    }

    /// Selector de pantalla encendida: tarjeta elevada que se despliega hacia arriba con las tres opciones,
    /// cada una con su icono, su nombre, una marca en la activa y un texto descriptivo.
    pub(crate) fn render_keep_awake_popup(
        &mut self,
        canvas: &mut Canvas,
        current: KeepAwakeMode,
        hovered: Option<usize>,
        working: bool,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        if width < 120 || height < StatusBarRects::BAR_H + 60 {
            return;
        }
        let (x, y, w, h) = StatusBarRects::keep_awake_popup_rect(width, height);
        canvas.rounded_fill(
            x + 1,
            y + 3,
            w,
            h,
            tokens::RADIUS_CONTROL,
            Color::from_rgba(0, 0, 0, 80),
        );
        canvas.rounded_fill(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_bg_elevated(),
        );
        canvas.rounded_stroke(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );

        let modes = [
            KeepAwakeMode::Off,
            KeepAwakeMode::Agent,
            KeepAwakeMode::Active,
        ];
        for (i, mode) in modes.into_iter().enumerate() {
            let (rx, ry, rw, rh) = StatusBarRects::keep_awake_option_rect(width, height, i);
            let selected = mode == current;
            if hovered == Some(i) {
                canvas.rounded_fill(rx, ry, rw, rh, tokens::RADIUS_ROW, theme.surface_hover());
            }
            let icon_fg = match mode {
                KeepAwakeMode::Off => theme.text_muted(),
                KeepAwakeMode::Agent if working => theme.ansi[6],
                KeepAwakeMode::Agent => theme.text_secondary(),
                KeepAwakeMode::Active => theme.accent_color(),
            };
            let icon = match mode {
                KeepAwakeMode::Agent => crate::ui::LucideIcon::Sparkles,
                KeepAwakeMode::Active => crate::ui::LucideIcon::Coffee,
                KeepAwakeMode::Off => crate::ui::LucideIcon::Eye,
            };
            let title_y = ry + 19;
            self.draw_lucide_icon(canvas, icon, rx + 16, ry + rh / 2, 12.0, icon_fg);
            self.draw_ui_text(
                canvas,
                mode.label(),
                rx + 34,
                title_y,
                rx + rw - 30,
                theme.foreground,
                11.5,
                true,
            );
            self.draw_ui_text(
                canvas,
                mode.description(),
                rx + 34,
                ry + 35,
                rx + rw - 12,
                theme.text_secondary(),
                10.0,
                false,
            );
            if selected {
                self.draw_lucide_icon(
                    canvas,
                    crate::ui::LucideIcon::Check,
                    rx + rw - 16,
                    title_y - 4,
                    12.0,
                    theme.accent_color(),
                );
            }
        }
    }

    /// Dibuja los medidores terminando en `right_limit` y devuelve el borde izquierdo que ocupan.
    /// El porcentaje va siempre como texto: el color de la barra solo refuerza.
    #[allow(clippy::too_many_arguments)]
    fn draw_status_meters(
        &mut self,
        canvas: &mut Canvas,
        meters: &[StatusMeter],
        right_limit: usize,
        (bar_y, bar_h): (usize, usize),
        baseline: usize,
        fill_color: Color,
        theme: &Theme,
    ) -> usize {
        let mut x = right_limit;
        // De derecha a izquierda: el primer medidor de la lista queda a la izquierda del segundo.
        for (i, meter) in meters.iter().enumerate().rev() {
            if i + 1 < meters.len() {
                x = x.saturating_sub(METER_GAP);
            }
            let pct = format!("{:.0}%", meter.percent.clamp(0.0, 100.0));
            let pct_w = self.measure_ui_text(&pct, METER_PCT_SIZE, false);
            let label_w = self.measure_ui_text(&meter.label, STATUS_LABEL_SIZE, false);
            let total =
                label_w + METER_LABEL_GAP + StatusBarRects::METER_TRACK_W + METER_PCT_GAP + pct_w;
            let start = x.saturating_sub(total);
            self.draw_ui_text(
                canvas,
                &meter.label,
                start,
                baseline,
                start + label_w,
                theme.text_secondary(),
                STATUS_LABEL_SIZE,
                false,
            );
            let track_x = start + label_w + METER_LABEL_GAP;
            let track_y = bar_y + (bar_h.saturating_sub(StatusBarRects::METER_TRACK_H)) / 2;
            canvas.rounded_fill(
                track_x,
                track_y,
                StatusBarRects::METER_TRACK_W,
                StatusBarRects::METER_TRACK_H,
                tokens::RADIUS_ROW,
                theme.chrome_line(),
            );
            let fill = ((meter.percent.clamp(0.0, 100.0) / 100.0)
                * StatusBarRects::METER_TRACK_W as f32)
                .round() as usize;
            if fill > 0 {
                // El primer carril (5 h) lleva el color del agente; el semanal, el acento de la UI,
                // salvo aviso de límite.
                let base = if i == 0 {
                    fill_color
                } else {
                    theme.accent_color()
                };
                let color = if meter.percent >= 90.0
                    || meter.alert_at.is_some_and(|limit| meter.percent >= limit)
                {
                    theme.ansi[1]
                } else if meter.percent >= 70.0 {
                    theme.ansi[3]
                } else {
                    base
                };
                canvas.rounded_fill(
                    track_x,
                    track_y,
                    fill.max(3),
                    StatusBarRects::METER_TRACK_H,
                    tokens::RADIUS_ROW,
                    color,
                );
            }
            self.draw_ui_text(
                canvas,
                &pct,
                track_x + StatusBarRects::METER_TRACK_W + METER_PCT_GAP,
                baseline,
                x,
                theme.text_secondary(),
                METER_PCT_SIZE,
                false,
            );
            x = start;
        }
        x
    }

    /// Ancho total de los medidores con sus separaciones, para reservarles sitio en la barra.
    pub(crate) fn status_meters_width(&self, meters: &[StatusMeter]) -> usize {
        let mut total = 0;
        for (i, meter) in meters.iter().enumerate() {
            if i > 0 {
                total += METER_GAP;
            }
            let pct = format!("{:.0}%", meter.percent.clamp(0.0, 100.0));
            total += self.measure_ui_text(&meter.label, STATUS_LABEL_SIZE, false)
                + METER_LABEL_GAP
                + StatusBarRects::METER_TRACK_W
                + METER_PCT_GAP
                + self.measure_ui_text(&pct, METER_PCT_SIZE, false);
        }
        total
    }

    /// Medidores de la barra de estado a partir de los límites reales del agente de la pestaña.
    pub(crate) fn status_meters_for(
        snapshot: &crate::app::AiUsageSnapshot,
        kind: AiAgentStatus,
    ) -> Vec<StatusMeter> {
        snapshot
            .limits_for(kind)
            .iter()
            .take(2)
            .map(|l| StatusMeter {
                label: short_limit_label(&l.label),
                percent: l.percent,
                alert_at: None,
            })
            .collect()
    }

    /// Medidores de la barra de estado a partir de los datos muestreados del monitor del sistema (F3).
    #[allow(dead_code)]
    pub(crate) fn system_status_meters(monitor: &crate::monitor::MonitorState) -> Vec<StatusMeter> {
        Self::system_status_meters_configured(monitor, &crate::config::PerformanceConfig::default())
    }

    /// Medidores de la barra de estado según la configuración de rendimiento activa (F8).
    pub(crate) fn system_status_meters_configured(
        monitor: &crate::monitor::MonitorState,
        perf: &crate::config::PerformanceConfig,
    ) -> Vec<StatusMeter> {
        let Some(snap) = &monitor.snapshot else {
            return Vec::new();
        };
        let alert = |threshold: u8| perf.enable_alerts.then_some(f32::from(threshold));
        let net_pct = ((snap.net_rx_bytes_sec + snap.net_tx_bytes_sec) as f32 / 12_500_000.0
            * 100.0)
            .min(100.0); // respecto a 100 Mbit/s, la misma escala que el historial del monitor
        let mut meters = Vec::new();
        let mut push = |enabled: bool, label: &str, percent: f32, alert_at: Option<f32>| {
            if enabled {
                meters.push(StatusMeter {
                    label: label.to_string(),
                    percent,
                    alert_at,
                });
            }
        };
        push(
            perf.show_cpu,
            "CPU",
            snap.cpu_total,
            alert(perf.cpu_alert_threshold),
        );
        push(
            perf.show_ram,
            "RAM",
            snap.ram_pct,
            alert(perf.ram_alert_threshold),
        );
        push(perf.show_gpu, "GPU", snap.gpu_usage_pct, None);
        push(
            perf.show_disk,
            "DISCO",
            snap.disk_usage_pct,
            alert(perf.disk_alert_threshold),
        );
        push(perf.show_net, "RED", net_pct, None);
        meters
    }

    pub(crate) fn status_bar_badge_info(
        ai_status: AiAgentStatus,
        ssh_host: Option<&str>,
        active_model: Option<&str>,
        theme: &Theme,
    ) -> (crate::ui::LucideIcon, String, Color) {
        let (icon, base, fg) = match ai_status {
            AiAgentStatus::Claude => (crate::ui::LucideIcon::Sparkles, "Claude", theme.ansi[5]),
            AiAgentStatus::Agy => (crate::ui::LucideIcon::Sparkles, "AGY", theme.ansi[6]),
            AiAgentStatus::Gemini => (crate::ui::LucideIcon::Sparkles, "Gemini", theme.ansi[4]),
            AiAgentStatus::Aider => (crate::ui::LucideIcon::Bot, "Aider", theme.ansi[2]),
            AiAgentStatus::Copilot => (crate::ui::LucideIcon::Bot, "Copilot", theme.ansi[6]),
            AiAgentStatus::Ollama => (crate::ui::LucideIcon::Bot, "Ollama", theme.ansi[3]),
            AiAgentStatus::None => {
                if ssh_host.is_some() {
                    return (
                        crate::ui::LucideIcon::Server,
                        "SSH".to_string(),
                        theme.ansi[2],
                    );
                }
                return (
                    crate::ui::LucideIcon::Terminal,
                    "Local".to_string(),
                    theme.text_muted(),
                );
            }
        };
        // El nombre legible, no el identificador: «opus-4.6» se muestra como «Opus 4.6».
        let label = match active_model.filter(|m| !m.is_empty()) {
            Some(model) => format!("{base} {}", pretty_model_name(model)),
            None => base.to_string(),
        };
        (icon, label, fg)
    }

    /// Alto de una fila de límite: etiqueta + barra + reinicio
    #[cfg(test)]
    pub(crate) const LIMIT_ROW_H: usize = 38;

    /// Alto del bloque de un agente: cabecera + límites reales + (ventana 5h y sesión, o un aviso si no
    /// hay tokens) + hueco
    #[cfg(test)]
    fn ai_agent_block_height(usage: &crate::app::AiAgentUsage, limits: usize) -> usize {
        let tokens = if usage.has_tokens() {
            16 + 18 + 16 + 18
        } else {
            18
        };
        20 + limits * Self::LIMIT_ROW_H + tokens + 10
    }

    /// Alto dinámico en píxeles de la tarjeta flotante de consumo de IA.
    #[cfg(test)]
    pub(crate) fn ai_usage_popup_height(snapshot: &crate::app::AiUsageSnapshot) -> usize {
        let mut h = 14; // padding superior
        h += 20; // cabecera: icono + título
        h += 8 + 1 + 10; // divisor 1

        // Un bloque por agente activo en la pestaña
        if snapshot.usages.is_empty() {
            h += 20 + 10; // mensaje "Sin agente de IA activo"
        } else {
            h += snapshot
                .usages
                .iter()
                .map(|u| Self::ai_agent_block_height(u, snapshot.limits_for(u.kind).len()))
                .sum::<usize>();
        }
        h += 1 + 10; // divisor 2 (el hueco de 8 px ya lo deja el bloque)
        h += 16; // Últimas 24h

        // Agentes y herramientas configurados
        if !snapshot.agents.is_empty() {
            h += 8 + 1 + 10; // divisor 3
            h += 16; // título "AGENTES Y HERRAMIENTAS"
            h += snapshot.agents.len() * 20;
        }

        if snapshot.terminal_hint.is_some() {
            h += 8 + 16;
        }

        h += 14; // padding inferior
        h
    }

    /// Renderiza el menú flotante de consumo de IA: un bloque por agente de la pestaña (cada uno con su
    /// color, modelo y ventanas), el total de 24 h y los agentes configurados.
    #[cfg(test)]
    pub(crate) fn render_ai_usage_popup(
        &mut self,
        canvas: &mut Canvas,
        snapshot: &crate::app::AiUsageSnapshot,
        theme: &Theme,
    ) {
        let (width, height) = canvas.size();
        let popup_h = Self::ai_usage_popup_height(snapshot);
        if width < 220 || height < popup_h + StatusBarRects::BAR_H + 12 {
            return;
        }
        let (x, y, w, h) = StatusBarRects::popup_rect(width, height, popup_h);

        // Sombra corta, superficie elevada y hairline
        canvas.rounded_fill(
            x + 1,
            y + 3,
            w,
            h,
            tokens::RADIUS_CONTROL,
            Color::from_rgba(0, 0, 0, 80),
        );
        canvas.rounded_fill(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_bg_elevated(),
        );
        canvas.rounded_stroke(
            x,
            y,
            w,
            h,
            tokens::RADIUS_CONTROL,
            theme.chrome_line_strong(),
        );

        let pad_x = x + 16;
        let max_x = x + w - 16;
        let divider_w = w.saturating_sub(32);
        let accent = theme.accent_color();
        let mut cur_y = y + 14;

        // Cabecera: icono + título
        self.draw_lucide_icon(
            canvas,
            crate::ui::LucideIcon::Sparkles,
            pad_x + 6,
            cur_y + 5,
            13.0,
            accent,
        );
        self.draw_ui_text(
            canvas,
            "Consumo de IA",
            pad_x + 18,
            cur_y + 10,
            max_x,
            theme.foreground,
            11.5,
            true,
        );
        cur_y += 20;

        cur_y += 8;
        Self::draw_rect(canvas, pad_x, cur_y, divider_w, 1, theme.border_color());
        cur_y += 1 + 10;

        if snapshot.usages.is_empty() {
            self.draw_ui_text(
                canvas,
                "Sin agente de IA activo en esta pestaña",
                pad_x,
                cur_y + 10,
                max_x,
                theme.text_secondary(),
                11.0,
                false,
            );
            cur_y += 20 + 10;
        }

        for u in &snapshot.usages {
            let (_, name, brand) = Self::status_bar_badge_info(u.kind, None, None, theme);

            // Cabecera: barra de color + agente + modelo + plan/modo a la derecha
            canvas.rounded_fill(pad_x, cur_y + 3, 2, 14, tokens::RADIUS_ROW, brand);
            let name_end = self.draw_ui_text_tracked(
                canvas,
                &name,
                pad_x + 10,
                cur_y + 10,
                max_x,
                brand,
                9.5,
                true,
                0.8,
            );
            let plan_w = u
                .plan
                .as_deref()
                .map_or(0, |p| self.measure_ui_text(p, tokens::MICRO, true) + 20);
            self.draw_ui_text(
                canvas,
                &u.model,
                name_end + 8,
                cur_y + 10,
                max_x.saturating_sub(plan_w),
                theme.foreground,
                tokens::MICRO,
                true,
            );
            if let Some(plan) = u.plan.as_deref() {
                let plan_w = self.measure_ui_text(plan, tokens::MICRO, false);
                self.draw_ui_text(
                    canvas,
                    plan,
                    max_x.saturating_sub(plan_w),
                    cur_y + 10,
                    max_x,
                    theme.text_muted(),
                    tokens::MICRO,
                    false,
                );
            }
            cur_y += 20;

            // Límites reales de la cuenta: porcentaje, barra y reinicio
            for lim in snapshot.limits_for(u.kind) {
                let bar_fg = if lim.percent >= 85.0 {
                    theme.ansi[1]
                } else if lim.percent >= 60.0 {
                    theme.ansi[3]
                } else {
                    theme.ansi[2]
                };
                let bar_x = pad_x + 10;
                let bar_w = divider_w.saturating_sub(10);
                let pct = format!("{:.0}% usado", lim.percent);
                let pct_w = self.measure_ui_text(&pct, 10.5, true);
                self.draw_ui_text(
                    canvas,
                    &lim.label,
                    bar_x,
                    cur_y + 10,
                    max_x.saturating_sub(pct_w + 8),
                    theme.foreground,
                    11.0,
                    false,
                );
                self.draw_ui_text(
                    canvas,
                    &pct,
                    max_x.saturating_sub(pct_w),
                    cur_y + 10,
                    max_x,
                    bar_fg,
                    10.5,
                    true,
                );
                let bar_y = cur_y + 15;
                canvas.rounded_fill(
                    bar_x,
                    bar_y,
                    bar_w,
                    3,
                    tokens::RADIUS_ROW,
                    theme.chrome_line(),
                );
                let fill_w = ((bar_w as f32 * lim.percent / 100.0).round() as usize)
                    .clamp(usize::from(lim.percent > 0.2) * 4, bar_w);
                if fill_w > 0 {
                    canvas.rounded_fill(bar_x, bar_y, fill_w, 3, tokens::RADIUS_ROW, bar_fg);
                }
                let reset = lim.resets_in_secs().map_or_else(
                    || "Sin fecha de reinicio".to_string(),
                    |s| format!("Reinicia en {}", crate::app::format_duration(s)),
                );
                self.draw_ui_text(
                    canvas,
                    &reset,
                    bar_x,
                    cur_y + 28,
                    max_x,
                    theme.text_secondary(),
                    10.0,
                    false,
                );
                cur_y += Self::LIMIT_ROW_H;
            }

            if !u.has_tokens() {
                self.draw_ui_text(
                    canvas,
                    "Sin consumo local registrado",
                    pad_x + 10,
                    cur_y + 10,
                    max_x,
                    theme.text_secondary(),
                    10.5,
                    false,
                );
                cur_y += 18 + 10;
                continue;
            }

            // Ventana de 5 h y sesión actual: etiqueta, valor de color y detalle
            let window_label = match u.window_5h.reset_in_secs {
                Some(rem) if rem > 0 => format!(
                    "Ventana 5h · reinicia en {}",
                    crate::app::format_duration(rem)
                ),
                _ => "Ventana 5h".to_string(),
            };
            let rows = [
                (
                    window_label,
                    &u.window_5h,
                    brand,
                    format!(
                        "Entrada: {} · Caché: {} · Salida: {}",
                        crate::app::format_tokens(u.window_5h.input_tokens),
                        crate::app::format_tokens(u.window_5h.cache_tokens),
                        crate::app::format_tokens(u.window_5h.output_tokens),
                    ),
                ),
                (
                    "Sesión actual".to_string(),
                    &u.session,
                    accent,
                    format!(
                        "Entrada: {} · Salida: {}",
                        crate::app::format_tokens(u.session.input_tokens),
                        crate::app::format_tokens(u.session.output_tokens),
                    ),
                ),
            ];
            for (label, win, value_fg, detail) in rows {
                self.draw_ui_text(
                    canvas,
                    &label,
                    pad_x + 10,
                    cur_y + 10,
                    max_x,
                    theme.foreground,
                    11.0,
                    false,
                );
                let value = format!(
                    "{} tok ({} turnos)",
                    crate::app::format_tokens(win.total_tokens()),
                    win.turns
                );
                let value_w = self.measure_ui_text(&value, 11.0, true);
                self.draw_ui_text(
                    canvas,
                    &value,
                    max_x.saturating_sub(value_w),
                    cur_y + 10,
                    max_x,
                    value_fg,
                    11.0,
                    true,
                );
                cur_y += 16;
                self.draw_ui_text(
                    canvas,
                    &detail,
                    pad_x + 10,
                    cur_y + 10,
                    max_x,
                    theme.text_secondary(),
                    10.0,
                    false,
                );
                cur_y += 18;
            }
            cur_y += 10;
        }

        // Divisor y total de 24 h de todos los agentes
        Self::draw_rect(canvas, pad_x, cur_y, divider_w, 1, theme.border_color());
        cur_y += 1 + 10;

        self.draw_ui_text(
            canvas,
            "Últimas 24h · todos los agentes",
            pad_x,
            cur_y + 10,
            max_x,
            theme.text_secondary(),
            11.0,
            false,
        );
        let d24_line = format!(
            "{} tok ({} sesiones)",
            crate::app::format_tokens(snapshot.window_24h_tokens),
            snapshot.window_24h_sessions
        );
        let d24_lw = self.measure_ui_text(&d24_line, tokens::MICRO, true);
        self.draw_ui_text(
            canvas,
            &d24_line,
            max_x.saturating_sub(d24_lw),
            cur_y + 10,
            max_x,
            theme.foreground,
            tokens::MICRO,
            true,
        );
        cur_y += 16;

        // Agentes y modelos configurados
        if !snapshot.agents.is_empty() {
            cur_y += 8;
            Self::draw_rect(canvas, pad_x, cur_y, divider_w, 1, theme.border_color());
            cur_y += 1 + 10;

            self.draw_ui_text(
                canvas,
                "AGENTES Y HERRAMIENTAS",
                pad_x,
                cur_y + 10,
                max_x,
                theme.text_secondary(),
                tokens::MICRO,
                true,
            );
            cur_y += 16;

            for ag in &snapshot.agents {
                let brand_fg = match ag.agent.to_uppercase().as_str() {
                    "AGY" => theme.ansi[6],
                    "CLAUDE" => theme.ansi[5],
                    "GEMINI" => theme.ansi[4],
                    _ => theme.text_secondary(),
                };
                let tag = format!("{}: ", ag.agent);
                let next_x = self.draw_ui_text(
                    canvas,
                    &tag,
                    pad_x,
                    cur_y + 10,
                    max_x,
                    brand_fg,
                    tokens::MICRO,
                    true,
                );
                let dw = self.measure_ui_text(&ag.detail, tokens::MICRO, false);
                let model_max_x = max_x.saturating_sub(dw + 8);
                self.draw_ui_text(
                    canvas,
                    &ag.model,
                    next_x,
                    cur_y + 10,
                    model_max_x,
                    theme.foreground,
                    tokens::MICRO,
                    false,
                );
                self.draw_ui_text(
                    canvas,
                    &ag.detail,
                    max_x.saturating_sub(dw),
                    cur_y + 10,
                    max_x,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                cur_y += 20;
            }
        }

        if let Some(ref hint) = snapshot.terminal_hint {
            cur_y += 8;
            self.draw_ui_text(
                canvas,
                hint,
                pad_x,
                cur_y + 10,
                max_x,
                theme.ansi[6],
                10.0,
                false,
            );
        }
    }

    /// Renderiza la barra de estado inferior específica para la vista del Monitor de Sistema (F3 / Monitor.dc.html).
    pub(crate) fn render_system_monitor_status_bar(
        &mut self,
        canvas: &mut Canvas,
        width: usize,
        height: usize,
        monitor: &crate::monitor::MonitorState,
        theme: &Theme,
    ) {
        let bar_h = 28usize;
        let bar_y = height.saturating_sub(bar_h);
        let center_y = bar_y + bar_h / 2;
        let baseline = self.centered_baseline(center_y);

        // Fondo y separador superior (DISENO §4 / Monitor.dc.html)
        Self::draw_rect(canvas, 0, bar_y, width, bar_h, theme.bar_bg());
        Self::draw_rect(canvas, 0, bar_y, width, 1, theme.chrome_line());

        let fallback_snap = crate::monitor::SystemSnapshot::default();
        let snap = monitor.snapshot.as_ref().unwrap_or(&fallback_snap);

        // Segmento 1: Fuente e intervalo de actualización
        let (dot_c, source_text) = match &monitor.selected_source {
            crate::monitor::MonitorSource::Local => (
                theme.ansi[2], // verde
                "Este equipo · actualiza cada 1 s".to_string(),
            ),
            crate::monitor::MonitorSource::Remote(dest) => {
                let is_conn = monitor
                    .remote_snapshots
                    .get(dest)
                    .is_some_and(|r| r.is_connected);
                let c = if is_conn {
                    theme.ansi[3]
                } else {
                    theme.ansi[1]
                };
                (c, format!("{} · actualiza cada 3 s", dest))
            }
        };

        let dot_size = 7usize;
        let dot_y = bar_y + (bar_h.saturating_sub(dot_size)) / 2;
        canvas.rounded_fill(14, dot_y, dot_size, dot_size, tokens::RADIUS_ROW, dot_c);

        let s1_w = self.measure_ui_text(&source_text, tokens::DETAIL, true) + 36;
        self.draw_ui_text(
            canvas,
            &source_text,
            26,
            baseline,
            s1_w.saturating_sub(8),
            theme.foreground,
            tokens::DETAIL,
            true,
        );
        Self::draw_rect(canvas, s1_w, bar_y, 1, bar_h, theme.chrome_line());

        // Segmento 2: Resumen del hardware
        let hw_summary = match &monitor.selected_source {
            crate::monitor::MonitorSource::Local => {
                let cpu_short = snap
                    .cpu_name
                    .split("with")
                    .next()
                    .unwrap_or(&snap.cpu_name)
                    .trim();
                let ram_gb = (snap.ram_total_bytes as f32 / 1_073_741_824.0).round();
                format!(
                    "Windows 11 · {} · {:.0} GB · {}",
                    cpu_short, ram_gb, snap.gpu_name
                )
            }
            crate::monitor::MonitorSource::Remote(dest) => {
                if let Some(rem) = monitor.remote_snapshots.get(dest) {
                    let ram_gb = rem.ram_total_bytes as f32 / 1_073_741_824.0;
                    format!(
                        "Linux remoto · {} · {:.1} GB RAM · Carga: {:.2}",
                        dest, ram_gb, rem.load_1m
                    )
                } else {
                    format!("Host remoto: {}", dest)
                }
            }
        };

        let s2_x = s1_w + 14;
        let s2_w = self.measure_ui_text(&hw_summary, tokens::DETAIL, false) + 24;
        self.draw_ui_text(
            canvas,
            &hw_summary,
            s2_x,
            baseline,
            width.saturating_sub(180),
            theme.text_secondary(),
            tokens::DETAIL,
            false,
        );
        Self::draw_rect(canvas, s2_x + s2_w, bar_y, 1, bar_h, theme.chrome_line());

        // Segmento derecho: Historial
        let hist_text = "Historial: últimos 60 s";
        let hist_w = self.measure_mono_text(hist_text, tokens::MICRO, false);
        let hist_x = width.saturating_sub(hist_w + 20);
        Self::draw_rect(
            canvas,
            hist_x.saturating_sub(12),
            bar_y,
            1,
            bar_h,
            theme.chrome_line(),
        );
        self.draw_mono_text(
            canvas,
            hist_text,
            hist_x,
            baseline,
            width.saturating_sub(10),
            theme.text_muted(),
            tokens::MICRO,
            false,
        );
    }
}

/// Nombre legible de un modelo: `claude-sonnet-5-5` o `opus-4.6` pasan a «Sonnet 5.5» y «Opus 4.6».
/// Un nombre ya legible («Gemini 3.8 Flash (Medium)») se devuelve igual.
fn pretty_model_name(raw: &str) -> String {
    let mut name = raw.trim();
    for prefix in ["anthropic/", "anthropic.", "claude-", "claude/", "claude."] {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest;
        }
    }
    let tokens: smallvec::SmallVec<[&str; 8]> = name
        .split(['-', '_', ' '])
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return raw.trim().to_string();
    }

    let mut head: smallvec::SmallVec<[&str; 4]> = smallvec::SmallVec::new();
    let mut version: smallvec::SmallVec<[&str; 2]> = smallvec::SmallVec::new();
    let mut tail: smallvec::SmallVec<[&str; 4]> = smallvec::SmallVec::new();
    for token in tokens {
        let numeric = token.chars().all(|c| c.is_ascii_digit() || c == '.');
        if numeric {
            // La fecha del snapshot (`20251101`) no es parte del nombre del modelo.
            if token.len() >= 8 {
                continue;
            }
            if tail.is_empty() {
                version.push(token);
            } else {
                tail.push(token);
            }
        } else if version.is_empty() {
            head.push(token);
        } else {
            tail.push(token);
        }
    }

    let mut out = String::with_capacity(name.len());
    for word in &head {
        if !out.is_empty() {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if !version.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&version.join("."));
    }
    if !tail.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&tail.join(" "));
    }
    if out.is_empty() {
        raw.trim().to_string()
    } else {
        out
    }
}

/// Etiqueta corta del medidor: «Sesión (5 h)» pasa a «5 h» y el límite semanal a «Semana».
pub(crate) fn short_limit_label(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.contains("5 h") {
        "5 h".to_string()
    } else if lower.contains("sem") || lower.contains("7 d") {
        "Semana".to_string()
    } else {
        label.chars().take(10).collect()
    }
}

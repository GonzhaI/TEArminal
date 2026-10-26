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
    /// Renderiza la barra de estado informativa inferior (Status Bar) con tipografía Inter e iconos Lucide
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

    pub(crate) fn render_status_bar_with_ai(
        &mut self,
        canvas: &mut Canvas,
        _sidebar_offset: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        cols: usize,
        rows: usize,
        ai_status: AiAgentStatus,
        active_model: Option<&str>,
        meters: &[StatusMeter],
        hovered_ai_badge: bool,
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
        let baseline = center_y + 4;

        // 1. Fondo: la barra pertenece al chrome y se separa del lienzo por el tono, sin hairline.
        Self::draw_rect(canvas, 0, bar_y, width, bar_h, theme.bar_bg());

        // 2. Grupo derecho anclado al borde: badge del agente · medidores · dimensiones de la rejilla.
        let (badge_icon, badge_label, ai_fg) =
            Self::status_bar_badge_info(ai_status, ssh_host, active_model, theme);
        let ai_text_w = self.measure_ui_text(&badge_label, STATUS_LABEL_SIZE, true);
        let update_info = crate::updater::get_available_update();
        let update_text = update_info
            .as_ref()
            .map(|u| format!("v{} disponible", u.latest_version));
        let update_w = update_text
            .as_ref()
            .map(|t| self.measure_mono_text(t, tokens::MICRO + 0.5, false));
        let dims = format!("{cols} × {rows}");
        let dims_w = self.measure_mono_text(&dims, tokens::MICRO, false);
        let meters_w = self.status_meters_width(meters);
        let rects = StatusBarRects::compute_with_status(
            width, height, ai_text_w, update_w, dims_w, meters_w,
        );

        // Dimensiones como dato, en el extremo derecho.
        let dims_x = width.saturating_sub(StatusBarRects::RIGHT_PAD + dims_w);
        self.draw_mono_text(
            canvas,
            &dims,
            dims_x,
            baseline,
            width.saturating_sub(StatusBarRects::RIGHT_PAD),
            theme.text_muted(),
            tokens::MICRO,
            false,
        );

        // Medidores de los límites reales del agente (solo Claude los tiene), a la izquierda de las
        // dimensiones: etiqueta, barra y porcentaje. La geometría decide si caben.
        if rects.meters.2 > 0 {
            self.draw_status_meters(
                canvas,
                meters,
                rects.meters.0 + rects.meters.2,
                (bar_y, bar_h),
                baseline,
                ai_fg,
                theme,
            );
        }

        let (bx, by, bw, bh) = rects.ai_badge;

        // 3. Izquierda: SSH, carpeta y rama. Sin separadores: el espacio en blanco ya agrupa.
        let mut cur_x = 14;
        let text_limit = rects
            .update_badge
            .map_or(bx, |(ux, _, _, _)| ux)
            .saturating_sub(18);

        if let Some(ssh) = ssh_host {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Server,
                cur_x + 5,
                center_y,
                11.0,
                theme.text_secondary(),
            );
            cur_x += 15;
            let adv = self.draw_mono_text(
                canvas,
                ssh,
                cur_x,
                baseline,
                text_limit,
                theme.text_secondary(),
                tokens::DETAIL,
                true,
            );
            cur_x = adv + 14;
        }

        if let Some(dir) = cwd {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Folder,
                cur_x + 5,
                center_y,
                11.0,
                theme.text_muted(),
            );
            cur_x += 15;
            let adv = self.draw_mono_text(
                canvas,
                dir,
                cur_x,
                baseline,
                text_limit,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            cur_x = adv + 14;
        }

        if let Some(branch) = git_branch {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::GitBranch,
                cur_x + 5,
                center_y,
                10.5,
                theme.accent_color(),
            );
            cur_x += 15;
            self.draw_mono_text(
                canvas,
                branch,
                cur_x,
                baseline,
                text_limit,
                theme.accent_color(),
                tokens::DETAIL,
                true,
            );
        }

        // 4. Actualización disponible: icono + texto de acento, sin relleno.
        if let (Some((ub_x, _ub_y, ub_w, _ub_h)), Some(utext)) = (rects.update_badge, update_text) {
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::ArrowUp,
                ub_x + 6,
                center_y,
                10.5,
                theme.accent_color(),
            );
            self.draw_mono_text(
                canvas,
                &utext,
                ub_x + 15,
                baseline,
                ub_x + ub_w,
                theme.accent_color(),
                tokens::MICRO + 0.5,
                false,
            );
        }

        // 5. Badge de agente: icono del agente + nombre legible y modelo. El color de marca es solo
        // del icono; el texto es el del chrome (legible), como en el bosquejo.
        if hovered_ai_badge {
            canvas.rounded_fill(
                bx,
                by + 3,
                bw,
                bh.saturating_sub(6),
                6,
                theme.accent_soft(26),
            );
        }
        self.draw_lucide_icon(canvas, badge_icon, bx + 8, center_y, 10.0, ai_fg);
        let label_fg = if ai_status == AiAgentStatus::None {
            ai_fg
        } else {
            theme.foreground
        };
        self.draw_ui_text(
            canvas,
            &badge_label,
            bx + 22,
            baseline,
            bx + bw,
            label_fg,
            STATUS_LABEL_SIZE,
            true,
        );
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
                3,
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
                let color = if meter.percent >= 90.0 {
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
                    3,
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
            })
            .collect()
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
                        "Remoto".to_string(),
                        theme.ansi[2],
                    );
                }
                return (
                    crate::ui::LucideIcon::Dot,
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
    #[allow(dead_code)]
    pub(crate) const LIMIT_ROW_H: usize = 38;

    /// Alto del bloque de un agente: cabecera + límites reales + (ventana 5h y sesión, o un aviso si no
    /// hay tokens) + hueco
    #[allow(dead_code)]
    fn ai_agent_block_height(usage: &crate::app::AiAgentUsage, limits: usize) -> usize {
        let tokens = if usage.has_tokens() {
            16 + 18 + 16 + 18
        } else {
            18
        };
        let note = if Self::ai_limits_note(usage.kind).is_some() {
            18
        } else {
            0
        };
        20 + note + limits * Self::LIMIT_ROW_H + tokens + 10
    }

    /// Aviso de los agentes sin fuente legítima de límites: AGY no guarda su cuota en disco y su servidor
    /// local exige un token CSRF interno, así que no se inventa un porcentaje.
    #[allow(dead_code)]
    fn ai_limits_note(kind: AiAgentStatus) -> Option<&'static str> {
        (kind == AiAgentStatus::Agy).then_some("Límites no disponibles para AGY")
    }

    /// Alto dinámico en píxeles de la tarjeta flotante de consumo de IA.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        canvas.rounded_fill(x + 1, y + 3, w, h, 6, Color::from_rgba(0, 0, 0, 80));
        canvas.rounded_fill(x, y, w, h, 6, theme.chrome_bg_elevated());
        canvas.rounded_stroke(x, y, w, h, 6, theme.chrome_line_strong());

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
            canvas.rounded_fill(pad_x, cur_y + 3, 2, 14, 1, brand);
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
                .map_or(0, |p| self.measure_ui_text(p, 10.0, true) + 20);
            self.draw_ui_text(
                canvas,
                &u.model,
                name_end + 8,
                cur_y + 10,
                max_x.saturating_sub(plan_w),
                theme.foreground,
                11.0,
                true,
            );
            if let Some(plan) = u.plan.as_deref() {
                let plan_w = self.measure_ui_text(plan, 10.0, false);
                self.draw_ui_text(
                    canvas,
                    plan,
                    max_x.saturating_sub(plan_w),
                    cur_y + 10,
                    max_x,
                    theme.text_muted(),
                    10.0,
                    false,
                );
            }
            cur_y += 20;

            if let Some(note) = Self::ai_limits_note(u.kind) {
                self.draw_ui_text(
                    canvas,
                    note,
                    pad_x + 10,
                    cur_y + 10,
                    max_x,
                    theme.text_secondary(),
                    10.5,
                    false,
                );
                cur_y += 18;
            }

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
                canvas.rounded_fill(bar_x, bar_y, bar_w, 3, 1, theme.chrome_line());
                let fill_w = ((bar_w as f32 * lim.percent / 100.0).round() as usize)
                    .clamp(usize::from(lim.percent > 0.2) * 4, bar_w);
                if fill_w > 0 {
                    canvas.rounded_fill(bar_x, bar_y, fill_w, 3, 1, bar_fg);
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
        let d24_lw = self.measure_ui_text(&d24_line, 10.5, true);
        self.draw_ui_text(
            canvas,
            &d24_line,
            max_x.saturating_sub(d24_lw),
            cur_y + 10,
            max_x,
            theme.foreground,
            10.5,
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
                10.0,
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
                let next_x =
                    self.draw_ui_text(canvas, &tag, pad_x, cur_y + 10, max_x, brand_fg, 10.5, true);
                let dw = self.measure_ui_text(&ag.detail, 10.0, false);
                let model_max_x = max_x.saturating_sub(dw + 8);
                self.draw_ui_text(
                    canvas,
                    &ag.model,
                    next_x,
                    cur_y + 10,
                    model_max_x,
                    theme.foreground,
                    10.5,
                    false,
                );
                self.draw_ui_text(
                    canvas,
                    &ag.detail,
                    max_x.saturating_sub(dw),
                    cur_y + 10,
                    max_x,
                    theme.text_secondary(),
                    10.0,
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

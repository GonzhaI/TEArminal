//! Barra de estado inferior.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra de estado informativa inferior (Status Bar) con tipografía Inter e iconos Lucide
    #[cfg(test)]
    pub fn render_status_bar(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
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
            buffer,
            width,
            height,
            sidebar_offset,
            cwd,
            git_branch,
            ssh_host,
            cols,
            rows,
            ai_status,
            None,
            false,
            explorer_visible,
            theme,
        );
    }

    pub fn render_status_bar_with_ai(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        _sidebar_offset: usize,
        cwd: Option<&str>,
        git_branch: Option<&str>,
        ssh_host: Option<&str>,
        cols: usize,
        rows: usize,
        ai_status: AiAgentStatus,
        active_model: Option<&str>,
        hovered_ai_badge: bool,
        _explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height < 26 {
            return;
        }

        let bar_h = StatusBarRects::BAR_H;
        let bar_y = height - bar_h;
        let center_y = bar_y + (bar_h / 2);
        let baseline = center_y + 4;

        // 1. Fondo y hairline superior: la barra pertenece al chrome, no al lienzo.
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            bar_y,
            width,
            bar_h,
            theme.chrome_bg(),
        );
        Self::draw_rect(
            buffer,
            width,
            height,
            0,
            bar_y,
            width,
            1,
            theme.chrome_line(),
        );

        // 2. Badge derecho: punto de estado + agente y modelo. Sin cápsula: solo el texto.
        let (_, badge_label, ai_fg) =
            Self::status_bar_badge_info(ai_status, ssh_host, active_model, theme);
        let ai_text_w = self.measure_ui_text(&badge_label, 11.0, true);
        let update_info = crate::updater::get_available_update();
        let update_text = update_info
            .as_ref()
            .map(|u| format!("v{} disponible", u.latest_version));
        let update_w = update_text
            .as_ref()
            .map(|t| self.measure_ui_text(t, 10.5, true));
        let rects = StatusBarRects::compute(width, height, ai_text_w, update_w);
        let (bx, by, bw, bh) = rects.ai_badge;
        let right_limit = rects
            .update_badge
            .map_or(bx, |(ux, _, _, _)| ux)
            .saturating_sub(10);

        // Dimensiones de la rejilla como dato, en el extremo derecho del grupo de estado.
        let dims = format!("{cols} × {rows}");
        let dims_w = self.measure_ui_text(&dims, 10.0, false);
        let dims_x = right_limit.saturating_sub(dims_w);
        let dims_x = dims_x.max(width.saturating_sub(320));
        self.draw_ui_text(
            buffer,
            width,
            height,
            &dims,
            dims_x,
            baseline,
            right_limit,
            theme.text_muted(),
            10.0,
            false,
        );
        let dims_sep_x = dims_x.saturating_sub(10);

        // 3. Izquierda: SSH, carpeta y rama. Segmentos con separador hairline, sin iconos de relleno.
        let mut cur_x = 12;
        let text_limit = dims_sep_x.saturating_sub(18);

        if let Some(ssh) = ssh_host {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Server,
                cur_x + 5,
                center_y,
                11.0,
                theme.text_secondary(),
            );
            cur_x += 15;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                ssh,
                cur_x,
                baseline,
                text_limit,
                theme.text_secondary(),
                11.0,
                true,
            );
            cur_x = adv + 12;
            Self::draw_rect(
                buffer,
                width,
                height,
                cur_x,
                bar_y + 7,
                1,
                bar_h.saturating_sub(14),
                theme.chrome_line(),
            );
            cur_x += 11;
        }

        if let Some(dir) = cwd {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Folder,
                cur_x + 5,
                center_y,
                11.0,
                theme.text_muted(),
            );
            cur_x += 15;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                dir,
                cur_x,
                baseline,
                text_limit,
                theme.text_secondary(),
                10.5,
                false,
            );
            cur_x = adv + 12;
            if git_branch.is_some() {
                Self::draw_rect(
                    buffer,
                    width,
                    height,
                    cur_x,
                    bar_y + 7,
                    1,
                    bar_h.saturating_sub(14),
                    theme.chrome_line(),
                );
                cur_x += 11;
            }
        }

        if let Some(branch) = git_branch {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::GitBranch,
                cur_x + 5,
                center_y,
                10.5,
                theme.accent_color(),
            );
            cur_x += 15;
            self.draw_ui_text(
                buffer,
                width,
                height,
                branch,
                cur_x,
                baseline,
                text_limit,
                theme.accent_color(),
                10.5,
                true,
            );
        }

        // 4. Separador antes de las dimensiones.
        Self::draw_rect(
            buffer,
            width,
            height,
            dims_sep_x,
            bar_y + 7,
            1,
            bar_h.saturating_sub(14),
            theme.chrome_line(),
        );

        // 5. Actualización disponible: icono + texto de acento, sin relleno.
        if let (Some((ub_x, _ub_y, ub_w, _ub_h)), Some(utext)) = (rects.update_badge, update_text) {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::ArrowUp,
                ub_x + 6,
                center_y,
                10.5,
                theme.accent_color(),
            );
            self.draw_ui_text(
                buffer,
                width,
                height,
                &utext,
                ub_x + 15,
                baseline,
                ub_x + ub_w,
                theme.accent_color(),
                10.5,
                true,
            );
            Self::draw_rect(
                buffer,
                width,
                height,
                ub_x + ub_w + 3,
                bar_y + 7,
                1,
                bar_h.saturating_sub(14),
                theme.chrome_line(),
            );
        }

        // 6. Badge de agente: punto de color y etiqueta; el modelo va en tinta secundaria.
        if hovered_ai_badge {
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                bx,
                by + 3,
                bw,
                bh.saturating_sub(6),
                3,
                theme.accent_soft(26),
            );
        }
        crate::ui::draw_rounded_rect(buffer, width, height, bx + 8, center_y - 2, 4, 4, 2, ai_fg);
        let (name, model) = badge_label
            .split_once(" · ")
            .map_or((badge_label.as_str(), None), |(n, m)| (n, Some(m)));
        let name_end = self.draw_ui_text_tracked(
            buffer,
            width,
            height,
            name,
            bx + 18,
            baseline,
            bx + bw,
            ai_fg,
            9.5,
            true,
            0.8,
        );
        if let Some(model) = model {
            self.draw_ui_text(
                buffer,
                width,
                height,
                model,
                name_end + 6,
                baseline,
                bx + bw - 8,
                theme.text_secondary(),
                10.5,
                false,
            );
        }
    }

    pub fn status_bar_badge_info(
        ai_status: AiAgentStatus,
        ssh_host: Option<&str>,
        active_model: Option<&str>,
        theme: &Theme,
    ) -> (crate::ui::LucideIcon, String, Color) {
        let (icon, base, fg) = match ai_status {
            AiAgentStatus::Claude => (crate::ui::LucideIcon::Sparkles, "CLAUDE", theme.ansi[5]),
            AiAgentStatus::Agy => (crate::ui::LucideIcon::Sparkles, "AGY", theme.ansi[6]),
            AiAgentStatus::Gemini => (crate::ui::LucideIcon::Sparkles, "GEMINI", theme.ansi[4]),
            AiAgentStatus::Aider => (crate::ui::LucideIcon::Bot, "AIDER", theme.ansi[2]),
            AiAgentStatus::Copilot => (crate::ui::LucideIcon::Bot, "COPILOT", theme.ansi[6]),
            AiAgentStatus::Ollama => (crate::ui::LucideIcon::Bot, "OLLAMA", theme.ansi[3]),
            AiAgentStatus::None => {
                if ssh_host.is_some() {
                    return (
                        crate::ui::LucideIcon::Server,
                        "REMOTO".to_string(),
                        theme.ansi[2],
                    );
                }
                return (
                    crate::ui::LucideIcon::Dot,
                    "LOCAL".to_string(),
                    theme.text_muted(),
                );
            }
        };
        let label = match active_model.filter(|m| !m.is_empty()) {
            Some(m) => format!("{base} · {m}"),
            None => base.to_string(),
        };
        (icon, label, fg)
    }

    /// Alto de una fila de límite: etiqueta + barra + reinicio
    pub(crate) const LIMIT_ROW_H: usize = 38;

    /// Alto del bloque de un agente: cabecera + límites reales + (ventana 5h y sesión, o un aviso si no
    /// hay tokens) + hueco
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
    fn ai_limits_note(kind: AiAgentStatus) -> Option<&'static str> {
        (kind == AiAgentStatus::Agy).then_some("Límites no disponibles para AGY")
    }

    /// Alto dinámico en píxeles de la tarjeta flotante de consumo de IA.
    pub fn ai_usage_popup_height(snapshot: &crate::app::AiUsageSnapshot) -> usize {
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
    pub fn render_ai_usage_popup(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        snapshot: &crate::app::AiUsageSnapshot,
        theme: &Theme,
    ) {
        let popup_h = Self::ai_usage_popup_height(snapshot);
        if width < 220 || height < popup_h + StatusBarRects::BAR_H + 12 {
            return;
        }
        let (x, y, w, h) = StatusBarRects::popup_rect(width, height, popup_h);

        // Sombra corta, superficie elevada y hairline
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x + 1,
            y + 3,
            w,
            h,
            6,
            Color::from_rgba(0, 0, 0, 80),
        );
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x,
            y,
            w,
            h,
            6,
            theme.chrome_bg_elevated(),
        );
        crate::ui::stroke_rounded_rect(
            buffer,
            width,
            height,
            x,
            y,
            w,
            h,
            6,
            theme.chrome_line_strong(),
        );

        let pad_x = x + 16;
        let max_x = x + w - 16;
        let divider_w = w.saturating_sub(32);
        let accent = theme.accent_color();
        let mut cur_y = y + 14;

        // Cabecera: icono + título
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Sparkles,
            pad_x + 6,
            cur_y + 5,
            13.0,
            accent,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
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
        Self::draw_rect(
            buffer,
            width,
            height,
            pad_x,
            cur_y,
            divider_w,
            1,
            theme.border_color(),
        );
        cur_y += 1 + 10;

        if snapshot.usages.is_empty() {
            self.draw_ui_text(
                buffer,
                width,
                height,
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
            crate::ui::draw_rounded_rect(buffer, width, height, pad_x, cur_y + 3, 2, 14, 1, brand);
            let name_end = self.draw_ui_text_tracked(
                buffer,
                width,
                height,
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
                buffer,
                width,
                height,
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
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
                    &lim.label,
                    bar_x,
                    cur_y + 10,
                    max_x.saturating_sub(pct_w + 8),
                    theme.foreground,
                    11.0,
                    false,
                );
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &pct,
                    max_x.saturating_sub(pct_w),
                    cur_y + 10,
                    max_x,
                    bar_fg,
                    10.5,
                    true,
                );
                let bar_y = cur_y + 15;
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    bar_x,
                    bar_y,
                    bar_w,
                    3,
                    1,
                    theme.chrome_line(),
                );
                let fill_w = ((bar_w as f32 * lim.percent / 100.0).round() as usize)
                    .clamp(usize::from(lim.percent > 0.2) * 4, bar_w);
                if fill_w > 0 {
                    crate::ui::draw_rounded_rect(
                        buffer, width, height, bar_x, bar_y, fill_w, 3, 1, bar_fg,
                    );
                }
                let reset = lim.resets_in_secs().map_or_else(
                    || "Sin fecha de reinicio".to_string(),
                    |s| format!("Reinicia en {}", crate::app::format_duration(s)),
                );
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
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
                    buffer,
                    width,
                    height,
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
        Self::draw_rect(
            buffer,
            width,
            height,
            pad_x,
            cur_y,
            divider_w,
            1,
            theme.border_color(),
        );
        cur_y += 1 + 10;

        self.draw_ui_text(
            buffer,
            width,
            height,
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
            buffer,
            width,
            height,
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
            Self::draw_rect(
                buffer,
                width,
                height,
                pad_x,
                cur_y,
                divider_w,
                1,
                theme.border_color(),
            );
            cur_y += 1 + 10;

            self.draw_ui_text(
                buffer,
                width,
                height,
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
                let next_x = self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &tag,
                    pad_x,
                    cur_y + 10,
                    max_x,
                    brand_fg,
                    10.5,
                    true,
                );
                let dw = self.measure_ui_text(&ag.detail, 10.0, false);
                let model_max_x = max_x.saturating_sub(dw + 8);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &ag.model,
                    next_x,
                    cur_y + 10,
                    model_max_x,
                    theme.foreground,
                    10.5,
                    false,
                );
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
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
                buffer,
                width,
                height,
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

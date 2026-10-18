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
        _cols: usize,
        _rows: usize,
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
        let bar_bg = theme.sidebar_bg();
        let border_c = theme.border_color();

        let bar_x = 0;
        let bar_w = width;

        // 1. Fondo y borde superior 1px abarcando todo el ancho inferior
        Self::draw_rect(buffer, width, height, bar_x, bar_y, bar_w, bar_h, bar_bg);
        Self::draw_rect(buffer, width, height, bar_x, bar_y, bar_w, 1, border_c);

        let center_y = bar_y + (bar_h / 2);
        let baseline = center_y + 4;
        let mut cur_x = 12;

        // 2. Izquierda: SSH Host y CWD con iconos Lucide
        if let Some(ssh) = ssh_host {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Server,
                cur_x + 6,
                center_y,
                12.0,
                theme.ansi[2],
            );
            cur_x += 16;
            let label = format!("ssh: {}", ssh);
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                &label,
                cur_x,
                baseline,
                width.saturating_sub(260),
                theme.ansi[2],
                11.0,
                true,
            );
            cur_x = adv + 16;
        }

        if let Some(dir) = cwd {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::Folder,
                cur_x + 6,
                center_y,
                12.0,
                theme.ansi[8],
            );
            cur_x += 16;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                dir,
                cur_x,
                baseline,
                (cur_x + 320).min(width.saturating_sub(180)),
                theme.ansi[7],
                11.0,
                false,
            );
            cur_x = adv + 16;
        }

        // Git branch con icono Lucide GitBranch
        if let Some(branch) = git_branch {
            self.draw_lucide_icon(
                buffer,
                width,
                height,
                crate::ui::LucideIcon::GitBranch,
                cur_x + 6,
                center_y,
                11.0,
                theme.accent_color(),
            );
            cur_x += 16;
            let adv = self.draw_ui_text(
                buffer,
                width,
                height,
                branch,
                cur_x,
                baseline,
                width.saturating_sub(150),
                theme.accent_color(),
                11.0,
                true,
            );
            let _ = adv;
        }

        // 3. Derecha: Badge de Agente/Modelo IA o estado de conexión (REMOTO / LOCAL)
        let (ai_icon, badge_label, ai_fg) =
            Self::status_bar_badge_info(ai_status, ssh_host, active_model, theme);

        let ai_text_w = self.measure_ui_text(&badge_label, 11.0, true);
        let rects = StatusBarRects::compute(width, height, ai_text_w);
        let (bx, by, bw, bh) = rects.ai_badge;

        if hovered_ai_badge {
            let accent = theme.accent_color();
            crate::ui::draw_rounded_rect(
                buffer,
                width,
                height,
                bx,
                by + 3,
                bw,
                bh.saturating_sub(5),
                5,
                Color::from_rgba(accent.r(), accent.g(), accent.b(), 34),
            );
        }

        let badge_x = width.saturating_sub(ai_text_w + 32);
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            ai_icon,
            badge_x + 6,
            center_y,
            11.0,
            ai_fg,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &badge_label,
            badge_x + 16,
            baseline,
            width,
            ai_fg,
            11.0,
            true,
        );
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
                if let Some(m) = active_model.filter(|m| !m.is_empty()) {
                    return (
                        crate::ui::LucideIcon::Sparkles,
                        format!("IA · {m}"),
                        theme.ansi[6],
                    );
                }
                return (
                    crate::ui::LucideIcon::Dot,
                    "LOCAL".to_string(),
                    theme.ansi[8],
                );
            }
        };
        let label = match active_model.filter(|m| !m.is_empty()) {
            Some(m) => format!("{base} · {m}"),
            None => base.to_string(),
        };
        (icon, label, fg)
    }

    /// Alto dinámico en píxeles de la tarjeta flotante de consumo de IA.
    pub fn ai_usage_popup_height(snapshot: &crate::app::AiUsageSnapshot) -> usize {
        let mut h = 58; // Cabecera + modelo activo
        let quota_rows = snapshot.quotas.len().max(1);
        h += 22 + quota_rows * 40; // Sección de cuotas y ventanas por modelo
        h += 22 + 58; // Sección de tokens (Ventana 5h · Sesión · 24h)
        if !snapshot.agents.is_empty() {
            h += 20 + snapshot.agents.len() * 20;
        }
        if snapshot.terminal_hint.is_some() {
            h += 22;
        }
        h + 12
    }

    /// Renderiza el menú flotante con todos los consumos de IA, ventanas de 5 horas y cuotas por modelo.
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

        // Sombra suave, fondo y borde redondeado
        crate::ui::draw_rounded_rect(
            buffer,
            width,
            height,
            x + 2,
            y + 4,
            w,
            h,
            10,
            Color::from_rgba(0, 0, 0, 85),
        );
        crate::ui::draw_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.surface_bg());
        crate::ui::stroke_rounded_rect(buffer, width, height, x, y, w, h, 10, theme.border_color());

        let pad_x = x + 14;
        let max_x = x + w - 14;
        let accent = theme.accent_color();
        let mut cur_y = y + 14;

        // 1. Cabecera: Icono + Título + Plan/Estado a la derecha
        self.draw_lucide_icon(
            buffer,
            width,
            height,
            crate::ui::LucideIcon::Sparkles,
            pad_x + 6,
            cur_y + 6,
            13.0,
            accent,
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            "Consumo y Cuotas de IA",
            pad_x + 18,
            cur_y + 10,
            max_x,
            theme.foreground,
            12.0,
            true,
        );
        if let Some(ref plan) = snapshot.active_plan {
            let pw = self.measure_ui_text(plan, 10.5, true);
            self.draw_ui_text(
                buffer,
                width,
                height,
                plan,
                max_x.saturating_sub(pw),
                cur_y + 10,
                max_x,
                theme.ansi[6],
                10.5,
                true,
            );
        }
        cur_y += 20;

        let active_model_label = match snapshot.active_model.as_deref() {
            Some(m) => format!("Modelo en uso: {m}"),
            None => "Modelo en uso: Sin agente activo".to_string(),
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &active_model_label,
            pad_x,
            cur_y + 8,
            max_x,
            theme.ansi[7],
            11.0,
            false,
        );
        cur_y += 18;

        // Divisor horizontal
        Self::draw_rect(
            buffer,
            width,
            height,
            pad_x,
            cur_y,
            w.saturating_sub(28),
            1,
            theme.border_color(),
        );
        cur_y += 8;

        // 2. Sección: Ventanas de 5h y Cuotas por Modelo
        self.draw_ui_text(
            buffer,
            width,
            height,
            "VENTANAS Y CUOTAS POR MODELO",
            pad_x,
            cur_y + 9,
            max_x,
            theme.ansi[8],
            10.0,
            true,
        );
        cur_y += 16;

        if snapshot.quotas.is_empty() {
            self.draw_ui_text(
                buffer,
                width,
                height,
                "Sin cuotas activas reportadas en esta sesión",
                pad_x,
                cur_y + 14,
                max_x,
                theme.ansi[8],
                11.0,
                false,
            );
            cur_y += 40;
        } else {
            let bar_w = w.saturating_sub(28);
            for q in &snapshot.quotas {
                let used_pct = (q.used_fraction * 100.0).round().clamp(0.0, 100.0);
                let rem_pct = (100.0 - used_pct).max(0.0);
                let bar_color = if used_pct >= 85.0 {
                    theme.ansi[1]
                } else if used_pct >= 60.0 {
                    theme.ansi[3]
                } else {
                    theme.ansi[2]
                };

                let name_fg = if q.is_active {
                    theme.foreground
                } else {
                    theme.ansi[7]
                };
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &q.name,
                    pad_x,
                    cur_y + 10,
                    max_x.saturating_sub(95),
                    name_fg,
                    11.0,
                    q.is_active,
                );

                let pct_str = format!("{used_pct:.1}% usado");
                let pct_w = self.measure_ui_text(&pct_str, 10.5, true);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &pct_str,
                    max_x.saturating_sub(pct_w),
                    cur_y + 10,
                    max_x,
                    bar_color,
                    10.5,
                    true,
                );

                // Barra de progreso de cuota
                let bar_y = cur_y + 15;
                crate::ui::draw_rounded_rect(
                    buffer,
                    width,
                    height,
                    pad_x,
                    bar_y,
                    bar_w,
                    5,
                    2,
                    theme.border_color(),
                );
                let fill_w = ((bar_w as f32 * q.used_fraction).round() as usize)
                    .clamp(if q.used_fraction > 0.002 { 4 } else { 0 }, bar_w);
                if fill_w > 0 {
                    crate::ui::draw_rounded_rect(
                        buffer, width, height, pad_x, bar_y, fill_w, 5, 2, bar_color,
                    );
                }

                // Subtítulo: ventana de reinicio y % disponible
                let sub = format!("{} · {rem_pct:.1}% libre", q.reset_label);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &sub,
                    pad_x,
                    cur_y + 32,
                    max_x,
                    theme.ansi[8],
                    10.0,
                    false,
                );
                cur_y += 40;
            }
        }

        // Divisor horizontal
        Self::draw_rect(
            buffer,
            width,
            height,
            pad_x,
            cur_y,
            w.saturating_sub(28),
            1,
            theme.border_color(),
        );
        cur_y += 6;

        // 3. Sección: Consumo de Tokens (Ventana 5h · Sesión · 24h)
        let win5_header = match snapshot.window_5h.reset_in_secs {
            Some(rem) if rem > 0 => format!(
                "TOKENS · VENTANA 5H (REINICIA EN {})",
                crate::app::format_duration(rem).to_uppercase()
            ),
            _ => "TOKENS · VENTANA 5H Y SESIÓN".to_string(),
        };
        self.draw_ui_text(
            buffer,
            width,
            height,
            &win5_header,
            pad_x,
            cur_y + 9,
            max_x,
            theme.ansi[8],
            10.0,
            true,
        );
        cur_y += 16;

        let w5 = &snapshot.window_5h;
        let w5_line = format!(
            "Ventana 5h: {} tok ({} turnos)",
            crate::app::format_tokens(w5.total_tokens()),
            w5.turns
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &w5_line,
            pad_x,
            cur_y + 10,
            max_x,
            theme.foreground,
            11.0,
            true,
        );
        let w5_detail = format!(
            "Ent: {} · Caché: {} · Sal: {}",
            crate::app::format_tokens(w5.input_tokens),
            crate::app::format_tokens(w5.cache_tokens),
            crate::app::format_tokens(w5.output_tokens),
        );
        let w5_dw = self.measure_ui_text(&w5_detail, 10.0, false);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &w5_detail,
            max_x.saturating_sub(w5_dw),
            cur_y + 10,
            max_x,
            theme.ansi[8],
            10.0,
            false,
        );
        cur_y += 19;

        let ses = &snapshot.current_session;
        let ses_line = format!(
            "Sesión actual: {} tok ({} turnos)",
            crate::app::format_tokens(ses.total_tokens()),
            ses.turns
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &ses_line,
            pad_x,
            cur_y + 10,
            max_x,
            theme.ansi[7],
            11.0,
            false,
        );
        let ses_detail = format!(
            "Ent: {} · Sal: {}",
            crate::app::format_tokens(ses.input_tokens),
            crate::app::format_tokens(ses.output_tokens),
        );
        let ses_dw = self.measure_ui_text(&ses_detail, 10.0, false);
        self.draw_ui_text(
            buffer,
            width,
            height,
            &ses_detail,
            max_x.saturating_sub(ses_dw),
            cur_y + 10,
            max_x,
            theme.ansi[8],
            10.0,
            false,
        );
        cur_y += 19;

        let d24_line = format!(
            "Últimas 24h: {} tok ({} sesiones)",
            crate::app::format_tokens(snapshot.window_24h_tokens),
            snapshot.window_24h_sessions
        );
        self.draw_ui_text(
            buffer,
            width,
            height,
            &d24_line,
            pad_x,
            cur_y + 10,
            max_x,
            theme.ansi[8],
            10.5,
            false,
        );
        cur_y += 18;

        // 4. Agentes y Modelos configurados
        if !snapshot.agents.is_empty() {
            Self::draw_rect(
                buffer,
                width,
                height,
                pad_x,
                cur_y,
                w.saturating_sub(28),
                1,
                theme.border_color(),
            );
            cur_y += 14;
            for ag in &snapshot.agents {
                let left = format!("{}: {}", ag.agent, ag.model);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &left,
                    pad_x,
                    cur_y + 4,
                    max_x.saturating_sub(110),
                    theme.ansi[7],
                    10.5,
                    false,
                );
                let dw = self.measure_ui_text(&ag.detail, 10.0, false);
                self.draw_ui_text(
                    buffer,
                    width,
                    height,
                    &ag.detail,
                    max_x.saturating_sub(dw),
                    cur_y + 4,
                    max_x,
                    theme.ansi[8],
                    10.0,
                    false,
                );
                cur_y += 20;
            }
        }

        if let Some(ref hint) = snapshot.terminal_hint {
            self.draw_ui_text(
                buffer,
                width,
                height,
                hint,
                pad_x,
                cur_y + 8,
                max_x,
                theme.ansi[6],
                10.0,
                false,
            );
        }
    }
}

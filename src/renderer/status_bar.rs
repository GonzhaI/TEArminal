//! Barra de estado inferior.

use super::*;

impl TerminalRenderer {
    /// Renderiza la barra de estado informativa inferior (Status Bar) con tipografía Inter e iconos Lucide
    pub fn render_status_bar(
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
        _explorer_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height < 26 {
            return;
        }

        let bar_h = 26;
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

        // 2. Izquierda: CWD o SSH Host con icono Lucide
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
                width.saturating_sub(200),
                theme.ansi[2],
                11.0,
                true,
            );
            cur_x = adv + 16;
        } else if let Some(dir) = cwd {
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
                (cur_x + 320).min(width.saturating_sub(200)),
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

        // 3. Derecha: Badge de Agente IA o estado de conexión (REMOTO / LOCAL)
        let (ai_icon, ai_name, ai_fg) = match ai_status {
            AiAgentStatus::Claude => (crate::ui::LucideIcon::Sparkles, "CLAUDE", theme.ansi[5]),
            AiAgentStatus::Agy => (crate::ui::LucideIcon::Sparkles, "AGY", theme.ansi[6]),
            AiAgentStatus::Gemini => (crate::ui::LucideIcon::Sparkles, "GEMINI", theme.ansi[4]),
            AiAgentStatus::Aider => (crate::ui::LucideIcon::Bot, "AIDER", theme.ansi[2]),
            AiAgentStatus::Copilot => (crate::ui::LucideIcon::Bot, "COPILOT", theme.ansi[6]),
            AiAgentStatus::Ollama => (crate::ui::LucideIcon::Bot, "OLLAMA", theme.ansi[3]),
            AiAgentStatus::None => {
                if ssh_host.is_some() {
                    (crate::ui::LucideIcon::Server, "REMOTO", theme.ansi[2])
                } else {
                    (crate::ui::LucideIcon::Dot, "LOCAL", theme.ansi[8])
                }
            }
        };

        let ai_text_w = self.measure_ui_text(ai_name, 11.0, true);
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
            ai_name,
            badge_x + 16,
            baseline,
            width,
            ai_fg,
            11.0,
            true,
        );
    }
}

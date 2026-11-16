//! Renderizado de vistas del módulo de bases de datos (DISENO §5, pantallas 05 a 08 y 10).
//!
//! Cubre:
//! - Barra contextual lateral para `AppModule::Data` (conexión activa, esquema/consultas, tablas e historial).
//! - Sub-navegación superior (Conexiones 05, Tabla/Browse 06, Editor SQL 07, Esquema 08).
//! - Banner de producción con cuenta atrás de ventana de escritura y retorno a solo lectura.
//! - Vistas completas de Conexiones, Cuadrícula de datos con cambios pendientes e inspector, Editor SQL con resultados y Esquema ER.

use super::*;
use crate::app::db_tab::{
    BrowseFilterField, DbBrowseState, DbConnField, DbConnFormTab, DbConnectionsState,
    DbQueryResultsTab, DbSchemaState, DbTabState,
};
use crate::db::store::{Engine, Env, SslMode};
use crate::renderer::geometry::{DbSidebarRects, DbViewMode, DbViewRects, Rect};
use crate::theme::{Color, Theme};
use crate::ui::LucideIcon;

impl TerminalRenderer {
    /// Renderiza el panel lateral contextual de Datos (cuando `active_module == AppModule::Data`).
    pub(crate) fn render_db_sidebar(
        &mut self,
        canvas: &mut Canvas,
        rects: crate::renderer::ContextualSidebarRects,
        db_state: Option<&DbTabState>,
        theme: &Theme,
    ) {
        let (cx, cy, cw, ch) = rects.content;
        if cw == 0 || ch == 0 {
            return;
        }

        // Fondo mantle y borde separador derecho
        Self::draw_rect(canvas, cx, cy, cw, ch, theme.bar_bg());
        Self::draw_rect(canvas, cx + cw - 1, cy, 1, ch, theme.chrome_line());

        let sb = DbSidebarRects::compute(rects.content);
        let default_db = DbTabState::default();
        let db = db_state.unwrap_or(&default_db);

        // 1. Selector de conexión activa (tarjeta con motor, nombre y badge de entorno)
        let (sx, sy, sw, sh) = sb.conn_selector;
        Self::draw_rect(canvas, sx, sy, sw, sh, theme.background);
        canvas.rounded_stroke(sx, sy, sw, sh, tokens::RADIUS_CONTROL, theme.border_color());

        // Badge de motor (PG, MY, MS)
        let (engine_tag, engine_bg) = match db.active_connection_engine {
            Engine::Postgres => ("PG", theme.accent_color()),
            Engine::Mysql | Engine::Mariadb => ("MY", theme.yellow()),
            Engine::SqlServer => ("MS", theme.red()),
        };
        let badge_rect = (sx + 4, sy + 4, 26, 26);
        Self::draw_rect(
            canvas,
            badge_rect.0,
            badge_rect.1,
            badge_rect.2,
            badge_rect.3,
            theme.mix(theme.mantle(), engine_bg, 0.16),
        );
        let badge_base = self.centered_baseline(sy + 17);
        self.draw_mono_text(
            canvas,
            engine_tag,
            badge_rect.0 + 5,
            badge_base,
            badge_rect.0 + badge_rect.2,
            engine_bg,
            tokens::MICRO,
            true,
        );

        // Nombre de conexión y base
        let name_base = self.centered_baseline(sy + 12);
        self.draw_ui_text(
            canvas,
            &db.active_connection_name,
            sx + 36,
            name_base,
            sx + sw - 60,
            theme.foreground,
            tokens::BODY,
            true,
        );
        let db_base = self.centered_baseline(sy + 25);
        self.draw_mono_text(
            canvas,
            &db.active_connection_db,
            sx + 36,
            db_base,
            sx + sw - 60,
            theme.text_secondary(),
            tokens::MICRO,
            false,
        );

        // Chip de entorno (PROD, STG, LOCAL)
        let (env_text, env_bg, env_fg) = match db.active_connection_env {
            Env::Prod => ("PROD", theme.red(), theme.red()),
            Env::Stg => ("STG", theme.yellow(), theme.yellow()),
            Env::Local => ("LOCAL", theme.accent_color(), theme.accent_color()),
        };
        let env_chip_rect = (sx + sw - 52, sy + 7, 46, 20);
        Self::draw_rect(
            canvas,
            env_chip_rect.0,
            env_chip_rect.1,
            env_chip_rect.2,
            env_chip_rect.3,
            theme.mix(theme.mantle(), env_bg, 0.16),
        );
        let env_base = self.centered_baseline(sy + 17);
        self.draw_mono_text(
            canvas,
            env_text,
            env_chip_rect.0 + 6,
            env_base,
            env_chip_rect.0 + env_chip_rect.2,
            env_fg,
            tokens::MICRO,
            true,
        );

        // 2. Conmutador de sub-pestañas: Esquema / Consultas
        let (sw_x, sw_y, sw_w, sw_h) = sb.subtab_switch;
        Self::draw_rect(canvas, sw_x, sw_y, sw_w, sw_h, theme.crust());
        canvas.rounded_stroke(
            sw_x,
            sw_y,
            sw_w,
            sw_h,
            tokens::RADIUS_CONTROL,
            theme.border_color(),
        );

        let is_queries = db.sidebar_subtab_queries;
        let sch_rect = sb.schema_tab_btn;
        let qry_rect = sb.query_tab_btn;

        if !is_queries {
            Self::draw_rect(
                canvas,
                sch_rect.0,
                sch_rect.1,
                sch_rect.2,
                sch_rect.3,
                theme.background,
            );
        } else {
            Self::draw_rect(
                canvas,
                qry_rect.0,
                qry_rect.1,
                qry_rect.2,
                qry_rect.3,
                theme.background,
            );
        }

        let tab_base = self.centered_baseline(sw_y + sw_h / 2);
        self.draw_ui_text(
            canvas,
            "Esquema",
            sch_rect.0 + 12,
            tab_base,
            sch_rect.0 + sch_rect.2,
            if !is_queries {
                theme.foreground
            } else {
                theme.text_secondary()
            },
            tokens::DETAIL,
            !is_queries,
        );
        self.draw_ui_text(
            canvas,
            "Consultas",
            qry_rect.0 + 10,
            tab_base,
            qry_rect.0 + qry_rect.2,
            if is_queries {
                theme.foreground
            } else {
                theme.text_secondary()
            },
            tokens::DETAIL,
            is_queries,
        );

        // 3. Campo de búsqueda/filtro
        let (fx, fy, fw, fh) = sb.search_input;
        Self::draw_rect(canvas, fx, fy, fw, fh, theme.background);
        let search_border = if db.sidebar_search_focused {
            theme.accent_color()
        } else {
            theme.border_color()
        };
        canvas.rounded_stroke(fx, fy, fw, fh, tokens::RADIUS_CONTROL, search_border);
        self.draw_lucide_icon(
            canvas,
            LucideIcon::Search,
            fx + 14,
            fy + fh / 2,
            14.0,
            if db.sidebar_search_focused {
                theme.accent_color()
            } else {
                theme.text_secondary()
            },
        );
        let search_base = self.centered_baseline(fy + fh / 2);
        let query_text = db.sidebar_search.text.trim();
        let (display_txt, txt_color) = if query_text.is_empty() {
            let ph = if !is_queries {
                "Filtrar tablas…"
            } else {
                "Filtrar consultas…"
            };
            (ph, theme.text_secondary())
        } else {
            (db.sidebar_search.text.as_str(), theme.foreground)
        };
        self.draw_ui_text(
            canvas,
            display_txt,
            fx + 28,
            search_base,
            fx + fw - 8,
            txt_color,
            tokens::DETAIL,
            false,
        );

        // 4. Lista de elementos filtrados y con scroll
        let (ix, iy, iw, ih) = sb.items_area;
        if !is_queries {
            // Lista de tablas del esquema
            let row_h = 30;
            let filtered = db.filtered_tables();
            let mut cur_y = iy;
            for (tbl_name, row_count) in filtered.iter().skip(db.sidebar_scroll) {
                if cur_y + row_h > iy + ih {
                    break;
                }
                let is_selected = tbl_name == &db.browse.table_name;
                if is_selected {
                    Self::draw_rect(
                        canvas,
                        ix + 8,
                        cur_y,
                        iw.saturating_sub(16),
                        row_h,
                        theme.background,
                    );
                }
                let r_base = self.centered_baseline(cur_y + row_h / 2);
                self.draw_lucide_icon(
                    canvas,
                    LucideIcon::Columns2,
                    ix + 14,
                    cur_y + 8,
                    14.0,
                    if is_selected {
                        theme.accent_color()
                    } else {
                        theme.text_secondary()
                    },
                );
                self.draw_ui_text(
                    canvas,
                    tbl_name,
                    ix + 34,
                    r_base,
                    ix + iw - 60,
                    theme.foreground,
                    tokens::BODY,
                    is_selected,
                );
                self.draw_mono_text(
                    canvas,
                    row_count,
                    ix + iw - 54,
                    r_base,
                    ix + iw - 10,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                cur_y += row_h;
            }
        } else {
            // Lista de consultas guardadas e historial filtradas
            let mut cur_y = iy;
            let header_h = 24;
            let row_h = 38;
            let saved = db.filtered_saved_queries();
            let history = db.filtered_history();

            if !saved.is_empty() {
                let h_base = self.centered_baseline(cur_y + header_h / 2);
                self.draw_ui_text(
                    canvas,
                    "GUARDADAS",
                    ix + 12,
                    h_base,
                    ix + iw - 12,
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                );
                cur_y += header_h;

                for item in &saved {
                    if cur_y + row_h > iy + ih {
                        break;
                    }
                    let dot_rect = (ix + 12, cur_y + 13, 6, 6);
                    Self::draw_rect(
                        canvas,
                        dot_rect.0,
                        dot_rect.1,
                        dot_rect.2,
                        dot_rect.3,
                        theme.green(),
                    );
                    let t_base = self.centered_baseline(cur_y + 12);
                    self.draw_ui_text(
                        canvas,
                        &item.title,
                        ix + 26,
                        t_base,
                        ix + iw - 12,
                        theme.foreground,
                        tokens::BODY,
                        false,
                    );
                    let m_base = self.centered_baseline(cur_y + 26);
                    self.draw_mono_text(
                        canvas,
                        &item.meta,
                        ix + 26,
                        m_base,
                        ix + iw - 12,
                        theme.text_secondary(),
                        tokens::MICRO,
                        false,
                    );
                    cur_y += row_h;
                }
            }

            if !history.is_empty() && cur_y + header_h + row_h <= iy + ih {
                let h_base = self.centered_baseline(cur_y + header_h / 2);
                self.draw_ui_text(
                    canvas,
                    "HISTORIAL · HOY",
                    ix + 12,
                    h_base,
                    ix + iw - 12,
                    theme.text_secondary(),
                    tokens::MICRO,
                    true,
                );
                cur_y += header_h;

                for item in history.iter().skip(db.sidebar_scroll) {
                    if cur_y + row_h > iy + ih {
                        break;
                    }
                    let dot_rect = (ix + 12, cur_y + 13, 6, 6);
                    let dot_c = if item.is_slow {
                        theme.yellow()
                    } else {
                        theme.accent_color()
                    };
                    Self::draw_rect(
                        canvas, dot_rect.0, dot_rect.1, dot_rect.2, dot_rect.3, dot_c,
                    );

                    let t_base = self.centered_baseline(cur_y + 12);
                    self.draw_mono_text(
                        canvas,
                        &item.title,
                        ix + 26,
                        t_base,
                        ix + iw - 12,
                        theme.foreground,
                        tokens::DETAIL,
                        false,
                    );
                    let m_base = self.centered_baseline(cur_y + 26);
                    self.draw_mono_text(
                        canvas,
                        &item.meta,
                        ix + 26,
                        m_base,
                        ix + iw - 12,
                        theme.text_secondary(),
                        tokens::MICRO,
                        false,
                    );
                    cur_y += row_h;
                }
            }
        }

        // 5. Botón inferior: Nueva conexión o Nueva consulta
        let (bx, by, bw, bh) = sb.new_conn_btn;
        Self::draw_rect(canvas, bx, by, bw, bh, theme.background);
        canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_CONTROL, theme.border_color());
        self.draw_lucide_icon(
            canvas,
            LucideIcon::Plus,
            bx + 10,
            by + 10,
            15.0,
            theme.foreground,
        );
        let btn_base = self.centered_baseline(by + bh / 2);
        let btn_label = if is_queries {
            "Nueva consulta"
        } else {
            "Nueva conexión"
        };
        self.draw_ui_text(
            canvas,
            btn_label,
            bx + 32,
            btn_base,
            bx + bw - 8,
            theme.foreground,
            tokens::BODY,
            true,
        );
    }

    /// Renderiza la vista principal de la pestaña de datos en el área de trabajo.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_db_view(
        &mut self,
        canvas: &mut Canvas,
        offset: usize,
        top_offset: usize,
        width: usize,
        height: usize,
        db: &DbTabState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        let container = (offset, top_offset, width, height);
        let has_prod_banner = db.active_connection_env == Env::Prod && db.is_write_enabled();
        let has_pending = !db.browse.pending_changes.is_empty();

        let rects = DbViewRects::compute(
            container,
            db.active_view,
            has_prod_banner,
            true,
            has_pending,
            &db.connections.store.connections,
        );

        // Fondo general del área de datos (crust)
        Self::draw_rect(canvas, offset, top_offset, width, height, theme.crust());

        // 1. Barra superior de sub-navegación
        self.render_db_sub_nav(canvas, &rects, db, theme);

        // 2. Banner de producción (si la escritura está habilitada con cuenta atrás)
        if let Some(banner_rect) = rects.prod_banner {
            if let Some(ref countdown) = db.prod_countdown_str() {
                self.render_db_prod_banner(canvas, banner_rect, countdown, theme);
            }
        }

        // 3. Renderizar la vista activa
        match db.active_view {
            DbViewMode::Connections => {
                self.render_db_view_connections(
                    canvas,
                    &rects,
                    &db.connections,
                    cursor_visible,
                    theme,
                );
            }
            DbViewMode::Browse => {
                self.render_db_view_browse(
                    canvas,
                    &rects,
                    &db.browse,
                    has_prod_banner,
                    cursor_visible,
                    theme,
                );
            }
            DbViewMode::Query => {
                self.render_db_view_query(canvas, &rects, db, cursor_visible, theme);
            }
            DbViewMode::Schema => {
                self.render_db_view_schema(canvas, &rects, &db.schema, theme);
            }
        }
    }

    /// Renderiza la barra de sub-navegación superior con las 4 pestañas de datos y el estado derecho.
    fn render_db_sub_nav(
        &mut self,
        canvas: &mut Canvas,
        rects: &DbViewRects,
        db: &DbTabState,
        theme: &Theme,
    ) {
        let (nx, ny, nw, nh) = rects.sub_nav;
        Self::draw_rect(canvas, nx, ny, nw, nh, theme.bar_bg());
        Self::draw_rect(canvas, nx, ny + nh - 1, nw, 1, theme.chrome_line());

        let table_tab = if db.browse.table_name.is_empty() {
            "Explorar".to_string()
        } else {
            format!("Tabla: {}", db.browse.table_name)
        };
        let tab_titles = ["Conexiones", table_tab.as_str(), "Editor SQL", "Esquema"];

        let active_idx = match db.active_view {
            DbViewMode::Connections => 0,
            DbViewMode::Browse => 1,
            DbViewMode::Query => 2,
            DbViewMode::Schema => 3,
        };

        for (i, &tab_rect) in rects.nav_tabs.iter().enumerate() {
            if i >= tab_titles.len() {
                break;
            }
            let is_active = i == active_idx;
            let (tx, ty, tw, th) = tab_rect;

            if is_active {
                // Fondo activo sutil y barra indicadora inferior
                Self::draw_rect(
                    canvas,
                    tx,
                    ty,
                    tw,
                    th,
                    theme.mix(theme.bar_bg(), theme.accent_color(), 0.14),
                );
                Self::draw_rect(canvas, tx, ty + th - 2, tw, 2, theme.accent_color());
            }

            let text_base = self.centered_baseline(ty + th / 2);
            self.draw_ui_text(
                canvas,
                tab_titles[i],
                tx + 16,
                text_base,
                tx + tw - 8,
                if is_active {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
                tokens::BODY,
                is_active,
            );
        }

        // Estado derecho: conexión y latencia
        let st_base = self.centered_baseline(ny + nh / 2);
        let st_str = if !db.is_connected {
            if db.active_connection_name.is_empty() {
                "Sin conexión".into()
            } else {
                format!("{} · desconectado", db.active_connection_name)
            }
        } else {
            format!("{} · {} ms", db.active_connection_name, db.ping_ms)
        };
        let st_w = self.measure_mono_text(&st_str, tokens::DETAIL, false);
        let status_x = (nx + nw).saturating_sub(st_w + 30);
        let dot_color = if db.is_connected {
            theme.accent_color()
        } else {
            theme.red()
        };
        Self::draw_rect(canvas, status_x, ny + 15, 8, 8, dot_color);
        self.draw_mono_text(
            canvas,
            &st_str,
            status_x + 14,
            st_base,
            nx + nw - 10,
            theme.text_secondary(),
            tokens::DETAIL,
            false,
        );
    }

    /// Renderiza el banner de alerta en producción (DISENO §5 pantalla 06 y 10).
    fn render_db_prod_banner(
        &mut self,
        canvas: &mut Canvas,
        banner_rect: Rect,
        countdown: &str,
        theme: &Theme,
    ) {
        let (bx, by, bw, bh) = banner_rect;
        let red = theme.red();

        // Fondo rojizo tintado y borde inferior
        Self::draw_rect(canvas, bx, by, bw, bh, theme.mix(theme.bar_bg(), red, 0.14));
        Self::draw_rect(canvas, bx, by + bh - 1, bw, 1, red);

        // Ícono de bloqueo/seguridad
        self.draw_lucide_icon(canvas, LucideIcon::Lock, bx + 16, by + bh / 2, 16.0, red);

        let text_base = self.centered_baseline(by + bh / 2);
        let msg = format!("Escritura habilitada en PROD · expira en {countdown}");
        self.draw_ui_text(
            canvas,
            &msg,
            bx + 38,
            text_base,
            bx + bw - 180,
            theme.foreground,
            tokens::BODY,
            true,
        );

        // Botón: Volver a solo lectura
        let btn_w = 140;
        let btn_h = 24;
        let btn_x = bx + bw.saturating_sub(btn_w + 14);
        let btn_y = by + (bh - btn_h) / 2;
        Self::draw_rect(canvas, btn_x, btn_y, btn_w, btn_h, theme.background);
        canvas.rounded_stroke(
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            tokens::RADIUS_CONTROL,
            theme.border_color(),
        );

        let btn_base = self.centered_baseline(btn_y + btn_h / 2);
        self.draw_ui_text(
            canvas,
            "Volver a solo lectura",
            btn_x + 10,
            btn_base,
            btn_x + btn_w - 4,
            theme.foreground,
            tokens::MICRO,
            false,
        );
    }

    fn draw_form_input_field(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        label: &str,
        value: &str,
        is_active: bool,
        is_password: bool,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        if y > 14 {
            let lbl_base = self.centered_baseline(y.saturating_sub(11));
            self.draw_ui_text(
                canvas,
                label,
                x,
                lbl_base,
                x + w,
                theme.text_secondary(),
                tokens::MICRO,
                true,
            );
        }
        Self::draw_rect(canvas, x, y, w, h, theme.background);
        let border_c = if is_active {
            theme.accent_color()
        } else {
            theme.border_color()
        };
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CONTROL, border_c);

        let val_display = if is_password && !value.is_empty() {
            "••••••••••••".to_string()
        } else {
            value.to_string()
        };
        let val_base = self.centered_baseline(y + h / 2);
        self.draw_mono_text(
            canvas,
            &val_display,
            x + 10,
            val_base,
            x + w - 10,
            theme.foreground,
            tokens::DETAIL,
            false,
        );

        if is_active && cursor_visible {
            let cursor_x = x + 10 + val_display.len() * 7;
            if cursor_x + 2 < x + w {
                Self::draw_rect(
                    canvas,
                    cursor_x,
                    y + 5,
                    2,
                    h.saturating_sub(10),
                    theme.accent_color(),
                );
            }
        }
    }

    fn draw_choice_chip(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        text: &str,
        is_selected: bool,
        accent: Color,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        let (bg, border, fg) = if is_selected {
            (
                theme.mix(theme.background, accent, 0.16),
                accent,
                theme.foreground,
            )
        } else {
            (
                theme.background,
                theme.border_color(),
                theme.text_secondary(),
            )
        };
        Self::draw_rect(canvas, x, y, w, h, bg);
        canvas.rounded_stroke(x, y, w, h, tokens::RADIUS_CONTROL, border);
        let base = self.centered_baseline(y + h / 2);
        self.draw_ui_text(
            canvas,
            text,
            x + 8,
            base,
            x + w - 4,
            fg,
            tokens::DETAIL,
            is_selected,
        );
    }

    fn draw_toggle_checkbox(
        &mut self,
        canvas: &mut Canvas,
        rect: Rect,
        label: &str,
        is_checked: bool,
        theme: &Theme,
    ) {
        let (x, y, w, h) = rect;
        if w == 0 || h == 0 {
            return;
        }
        let box_size = 18;
        let box_y = y + (h.saturating_sub(box_size)) / 2;
        let box_rect = (x, box_y, box_size, box_size);

        if is_checked {
            Self::draw_rect(
                canvas,
                box_rect.0,
                box_rect.1,
                box_rect.2,
                box_rect.3,
                theme.accent_color(),
            );
            canvas.rounded_stroke(
                box_rect.0,
                box_rect.1,
                box_rect.2,
                box_rect.3,
                tokens::RADIUS_CONTROL,
                theme.accent_color(),
            );
            self.draw_lucide_icon(
                canvas,
                crate::ui::LucideIcon::Check,
                x + box_size / 2,
                box_y + box_size / 2,
                11.0,
                theme.background,
            );
        } else {
            Self::draw_rect(
                canvas,
                box_rect.0,
                box_rect.1,
                box_rect.2,
                box_rect.3,
                theme.background,
            );
            canvas.rounded_stroke(
                box_rect.0,
                box_rect.1,
                box_rect.2,
                box_rect.3,
                tokens::RADIUS_CONTROL,
                theme.border_color(),
            );
        }

        let lbl_base = self.centered_baseline(y + h / 2);
        self.draw_ui_text(
            canvas,
            label,
            x + box_size + 10,
            lbl_base,
            x + w,
            theme.foreground,
            tokens::BODY,
            false,
        );
    }

    /// Vista 05: Conexiones agrupadas por entorno + Formulario de configuración interactivo.
    fn render_db_view_connections(
        &mut self,
        canvas: &mut Canvas,
        rects: &DbViewRects,
        state: &DbConnectionsState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let Some((lx, ly, lw, lh)) = rects.conn_list_rect else {
            return;
        };
        let Some((fx, fy, fw, fh)) = rects.conn_form_rect else {
            return;
        };

        // 1. Panel izquierdo: Lista de conexiones
        Self::draw_rect(canvas, lx, ly, lw, lh, theme.mantle());
        Self::draw_rect(canvas, lx + lw - 1, ly, 1, lh, theme.chrome_line());

        if state.store.connections.is_empty() {
            let msg_y = ly + 40;
            let m_base = self.centered_baseline(msg_y);
            self.draw_ui_text(
                canvas,
                "Sin conexiones guardadas",
                lx + 16,
                m_base,
                lx + lw - 16,
                theme.text_secondary(),
                tokens::BODY,
                true,
            );
            let s_base = self.centered_baseline(msg_y + 24);
            self.draw_ui_text(
                canvas,
                "Completa el formulario y pulsa",
                lx + 16,
                s_base,
                lx + lw - 16,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            let s2_base = self.centered_baseline(msg_y + 44);
            self.draw_ui_text(
                canvas,
                "«Guardar» o «Conectar».",
                lx + 16,
                s2_base,
                lx + lw - 16,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
        } else {
            let envs = [
                (Env::Prod, "PRODUCCIÓN"),
                (Env::Stg, "STAGING"),
                (Env::Local, "LOCAL"),
            ];

            let mut cur_y = (ly + 12) as isize - (state.list_scroll * 38) as isize;
            for (env, header_title) in envs {
                let conns_in_env: Vec<_> = state
                    .store
                    .connections
                    .iter()
                    .filter(|c| c.env == env)
                    .collect();
                if conns_in_env.is_empty() {
                    continue;
                }

                // Cabecera de grupo
                if cur_y >= ly as isize && (cur_y as usize + 24) <= ly + lh {
                    let cy_u = cur_y as usize;
                    let grp_base = self.centered_baseline(cy_u + 10);
                    self.draw_ui_text_tracked(
                        canvas,
                        header_title,
                        lx + 14,
                        grp_base,
                        lx + lw - 10,
                        theme.text_secondary(),
                        tokens::MICRO,
                        true,
                        1.5,
                    );
                }
                cur_y += 24;

                for c in conns_in_env {
                    if cur_y >= ly as isize && (cur_y as usize + 34) <= ly + lh {
                        let cy_u = cur_y as usize;
                        let is_sel = state.selected_id.as_deref() == Some(&c.id);
                        if is_sel {
                            Self::draw_rect(
                                canvas,
                                lx + 8,
                                cy_u,
                                lw.saturating_sub(16),
                                34,
                                theme.background,
                            );
                            canvas.rounded_stroke(
                                lx + 8,
                                cy_u,
                                lw.saturating_sub(16),
                                34,
                                tokens::RADIUS_CONTROL,
                                theme.accent_color(),
                            );
                        }

                        let (tag, tag_c) = match c.engine {
                            Engine::Postgres => ("PG", theme.accent_color()),
                            Engine::Mysql | Engine::Mariadb => ("MY", theme.yellow()),
                            Engine::SqlServer => ("MS", theme.red()),
                        };
                        Self::draw_rect(
                            canvas,
                            lx + 12,
                            cy_u + 5,
                            24,
                            24,
                            theme.mix(theme.mantle(), tag_c, 0.16),
                        );
                        let tag_base = self.centered_baseline(cy_u + 17);
                        self.draw_mono_text(
                            canvas,
                            tag,
                            lx + 16,
                            tag_base,
                            lx + 36,
                            tag_c,
                            tokens::MICRO,
                            true,
                        );

                        let name_base = self.centered_baseline(cy_u + 17);
                        self.draw_ui_text(
                            canvas,
                            &c.name,
                            lx + 44,
                            name_base,
                            lx + lw - 30,
                            theme.foreground,
                            tokens::BODY,
                            is_sel,
                        );

                        let dot_c = if c.env == Env::Prod {
                            theme.red()
                        } else {
                            theme.accent_color()
                        };
                        Self::draw_rect(canvas, lx + lw - 22, cy_u + 14, 6, 6, dot_c);
                    }
                    cur_y += 38;
                }
                cur_y += 8;
            }
        }

        // 2. Tarjeta derecha: Formulario de conexión interactivo
        Self::draw_rect(canvas, fx, fy, fw, fh, theme.background);
        canvas.rounded_stroke(fx, fy, fw, fh, tokens::RADIUS_CONTROL, theme.border_color());

        // Cabecera del formulario (nombre editable, chips y botones de acción)
        let head_h = 62;
        Self::draw_rect(canvas, fx, fy, fw, head_h, theme.mantle());
        Self::draw_rect(canvas, fx, fy + head_h - 1, fw, 1, theme.chrome_line());

        // Nombre de conexión editable
        if let Some(ref fields) = rects.conn_fields {
            self.draw_form_input_field(
                canvas,
                fields.name_input,
                "NOMBRE DE CONEXIÓN",
                &state.name_input.text,
                state.active_field == Some(DbConnField::Name),
                false,
                cursor_visible,
                theme,
            );
        } else {
            let h_base = self.centered_baseline(fy + head_h / 2);
            self.draw_mono_text(
                canvas,
                &state.name_input.text,
                fx + 16,
                h_base,
                fx + 160,
                theme.foreground,
                tokens::TITLE,
                true,
            );
        }

        // Botones de acción en cabecera: Conectar, Probar conexión y Guardar
        if let Some((cx, cy, cw, ch)) = rects.conn_connect_btn {
            Self::draw_rect(canvas, cx, cy, cw, ch, theme.accent_color());
            canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CONTROL, theme.accent_color());
            let b_base = self.centered_baseline(cy + ch / 2);
            self.draw_ui_text(
                canvas,
                "Conectar",
                cx + 14,
                b_base,
                cx + cw - 6,
                theme.background,
                tokens::DETAIL,
                true,
            );
        }

        if let Some((tx, ty, tw, th)) = rects.conn_test_btn {
            Self::draw_rect(canvas, tx, ty, tw, th, theme.background);
            canvas.rounded_stroke(tx, ty, tw, th, tokens::RADIUS_CONTROL, theme.border_color());
            let b_base = self.centered_baseline(ty + th / 2);
            self.draw_ui_text(
                canvas,
                "Probar conexión",
                tx + 10,
                b_base,
                tx + tw - 6,
                theme.foreground,
                tokens::DETAIL,
                false,
            );
        }

        if let Some((sx, sy, sw, sh)) = rects.conn_save_btn {
            Self::draw_rect(canvas, sx, sy, sw, sh, theme.background);
            canvas.rounded_stroke(sx, sy, sw, sh, tokens::RADIUS_CONTROL, theme.border_color());
            let b_base = self.centered_baseline(sy + sh / 2);
            self.draw_ui_text(
                canvas,
                "Guardar",
                sx + 14,
                b_base,
                sx + sw - 6,
                theme.foreground,
                tokens::DETAIL,
                false,
            );
        }

        if let Some((dx, dy, dw, dh)) = rects.conn_delete_btn {
            Self::draw_rect(canvas, dx, dy, dw, dh, theme.background);
            canvas.rounded_stroke(
                dx,
                dy,
                dw,
                dh,
                tokens::RADIUS_CONTROL,
                Color::from_rgba(theme.red().r, theme.red().g, theme.red().b, 140),
            );
            self.draw_lucide_icon(
                canvas,
                LucideIcon::Trash2,
                dx + dw / 2,
                dy + dh / 2,
                14.0,
                theme.red(),
            );
        }

        // Subtabs del formulario (General, SSL / TLS, Túnel SSH, Avanzado)
        let tab_names = ["General", "SSL / TLS", "Túnel SSH", "Avanzado"];
        for (i, &t_rect) in rects.conn_form_tabs.iter().enumerate() {
            if i >= tab_names.len() {
                break;
            }
            let is_cur = matches!(
                (i, state.form_tab),
                (0, DbConnFormTab::General)
                    | (1, DbConnFormTab::Tls)
                    | (2, DbConnFormTab::Tunnel)
                    | (3, DbConnFormTab::Advanced)
            );
            let (tx, ty, tw, th) = t_rect;
            if is_cur {
                Self::draw_rect(canvas, tx, ty + th - 2, tw, 2, theme.accent_color());
            }
            let t_base = self.centered_baseline(ty + th / 2);
            self.draw_ui_text(
                canvas,
                tab_names[i],
                tx + 12,
                t_base,
                tx + tw - 6,
                if is_cur {
                    theme.foreground
                } else {
                    theme.text_secondary()
                },
                tokens::BODY,
                is_cur,
            );
        }

        // 3. Controles interactivos del formulario
        if let Some(ref fields) = rects.conn_fields {
            match state.form_tab {
                DbConnFormTab::General => {
                    // Botones de Motor
                    for &(eng, btn_rect) in &fields.engine_btns {
                        let is_sel = state.engine == eng;
                        let (lbl, accent) = match eng {
                            Engine::Postgres => ("PostgreSQL", theme.accent_color()),
                            Engine::Mysql | Engine::Mariadb => ("MySQL", theme.yellow()),
                            Engine::SqlServer => ("SQL Server", theme.red()),
                        };
                        self.draw_choice_chip(canvas, btn_rect, lbl, is_sel, accent, theme);
                    }

                    // Botones de Entorno
                    for &(env, btn_rect) in &fields.env_btns {
                        let is_sel = state.env == env;
                        let (lbl, accent) = match env {
                            Env::Local => ("Local", theme.accent_color()),
                            Env::Stg => ("Staging", theme.yellow()),
                            Env::Prod => ("Producción", theme.red()),
                        };
                        self.draw_choice_chip(canvas, btn_rect, lbl, is_sel, accent, theme);
                    }

                    // Campos de texto: Host, Puerto, Database, User, Password
                    self.draw_form_input_field(
                        canvas,
                        fields.host_input,
                        "HOST / SERVIDOR",
                        &state.host_input.text,
                        state.active_field == Some(DbConnField::Host),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.port_input,
                        "PUERTO",
                        &state.port_input.text,
                        state.active_field == Some(DbConnField::Port),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.database_input,
                        "BASE DE DATOS",
                        &state.database_input.text,
                        state.active_field == Some(DbConnField::Database),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.user_input,
                        "USUARIO",
                        &state.user_input.text,
                        state.active_field == Some(DbConnField::User),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.password_input,
                        "CONTRASEÑA",
                        &state.password_input.text,
                        state.active_field == Some(DbConnField::Password),
                        true,
                        cursor_visible,
                        theme,
                    );

                    // Toggle solo lectura
                    self.draw_toggle_checkbox(
                        canvas,
                        fields.read_only_toggle,
                        "Solo lectura (bloquea UPDATE, DELETE y transacciones de escritura)",
                        state.read_only,
                        theme,
                    );
                }
                DbConnFormTab::Tls => {
                    // Botones de Modo SSL
                    for &(mode, btn_rect) in &fields.ssl_mode_btns {
                        let is_sel = state.ssl_mode == mode;
                        let lbl = match mode {
                            SslMode::Disable => "Deshabilitado",
                            SslMode::Require => "Requerido",
                            SslMode::VerifyCa => "Verificar CA",
                            SslMode::VerifyFull => "Verificar Completo",
                        };
                        self.draw_choice_chip(
                            canvas,
                            btn_rect,
                            lbl,
                            is_sel,
                            theme.accent_color(),
                            theme,
                        );
                    }

                    // Inputs TLS
                    self.draw_form_input_field(
                        canvas,
                        fields.tls_server_name,
                        "NOMBRE DE SERVIDOR (SNI / VERIFY-FULL)",
                        &state.tls_server_name.text,
                        state.active_field == Some(DbConnField::TlsServerName),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tls_ca_path,
                        "CERTIFICADO CA (.PEM)",
                        &state.tls_ca_path.text,
                        state.active_field == Some(DbConnField::TlsCaPath),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tls_cert_path,
                        "CERTIFICADO DE CLIENTE (.PEM / .CRT)",
                        &state.tls_cert_path.text,
                        state.active_field == Some(DbConnField::TlsCertPath),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tls_key_path,
                        "CLAVE PRIVADA DE CLIENTE (.PEM / .KEY)",
                        &state.tls_key_path.text,
                        state.active_field == Some(DbConnField::TlsKeyPath),
                        false,
                        cursor_visible,
                        theme,
                    );
                }
                DbConnFormTab::Tunnel => {
                    // Toggle Habilitar túnel SSH
                    self.draw_toggle_checkbox(
                        canvas,
                        fields.tunnel_toggle,
                        "Habilitar túnel SSH intermedio (bastión / jump host)",
                        state.tunnel_enabled,
                        theme,
                    );

                    // Inputs de túnel
                    self.draw_form_input_field(
                        canvas,
                        fields.tunnel_host,
                        "HOST SSH",
                        &state.tunnel_host.text,
                        state.active_field == Some(DbConnField::TunnelHost),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tunnel_port,
                        "PUERTO SSH",
                        &state.tunnel_port.text,
                        state.active_field == Some(DbConnField::TunnelPort),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tunnel_user,
                        "USUARIO SSH",
                        &state.tunnel_user.text,
                        state.active_field == Some(DbConnField::TunnelUser),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tunnel_key_path,
                        "LLAVE PRIVADA SSH (.PEM / .KEY)",
                        &state.tunnel_key_path.text,
                        state.active_field == Some(DbConnField::TunnelKeyPath),
                        false,
                        cursor_visible,
                        theme,
                    );
                    self.draw_form_input_field(
                        canvas,
                        fields.tunnel_fingerprint,
                        "HUELLA SHA256 CONOCIDA (OPCIONAL)",
                        &state.tunnel_fingerprint.text,
                        state.active_field == Some(DbConnField::TunnelFingerprint),
                        false,
                        cursor_visible,
                        theme,
                    );
                }
                DbConnFormTab::Advanced => {
                    let adv_y = fy + 120;
                    let adv_base = self.centered_baseline(adv_y);
                    self.draw_ui_text(
                        canvas,
                        "Configuración avanzada de red y seguridad",
                        fx + 24,
                        adv_base,
                        fx + fw - 24,
                        theme.foreground,
                        tokens::BODY,
                        true,
                    );
                    let sub_base = self.centered_baseline(adv_y + 24);
                    let creds_msg = if state.selected_id.is_some() {
                        "Contraseña protegida en Administrador de credenciales de Windows · Pool activo con verificación TLS/SSH."
                    } else {
                        "Nueva conexión sin guardar · Los límites de consulta, backups y timeouts siguen la política global."
                    };
                    self.draw_ui_text(
                        canvas,
                        creds_msg,
                        fx + 24,
                        sub_base,
                        fx + fw - 24,
                        theme.text_secondary(),
                        tokens::DETAIL,
                        false,
                    );

                    if let Some((bx, by, bw, bh)) = rects.conn_adv_duplicate_btn {
                        Self::draw_rect(canvas, bx, by, bw, bh, theme.mantle());
                        canvas.rounded_stroke(
                            bx,
                            by,
                            bw,
                            bh,
                            tokens::RADIUS_CONTROL,
                            theme.border_color(),
                        );
                        let b_base = self.centered_baseline(by + bh / 2);
                        self.draw_ui_text(
                            canvas,
                            "Duplicar conexión",
                            bx + 14,
                            b_base,
                            bx + bw - 8,
                            theme.foreground,
                            tokens::DETAIL,
                            false,
                        );
                    }

                    if let Some((sx, sy, sw, sh)) = rects.conn_adv_settings_btn {
                        Self::draw_rect(canvas, sx, sy, sw, sh, theme.mantle());
                        canvas.rounded_stroke(
                            sx,
                            sy,
                            sw,
                            sh,
                            tokens::RADIUS_CONTROL,
                            theme.accent_color(),
                        );
                        let s_base = self.centered_baseline(sy + sh / 2);
                        self.draw_ui_text(
                            canvas,
                            "Abrir ajustes de BD…",
                            sx + 14,
                            s_base,
                            sx + sw - 8,
                            theme.accent_color(),
                            tokens::DETAIL,
                            true,
                        );
                    }
                }
            }
        }

        // 4. Panel inferior de «Resultado de la prueba» (solo si hay pasos o advertencias)
        if !state.test_steps.is_empty() || state.test_warning.is_some() {
            let Some((test_x, test_y, test_w, test_panel_h)) = rects.conn_test_panel else {
                return;
            };
            Self::draw_rect(canvas, test_x, test_y, test_w, test_panel_h, theme.mantle());
            canvas.rounded_stroke(
                test_x,
                test_y,
                test_w,
                test_panel_h,
                tokens::RADIUS_CONTROL,
                theme.border_color(),
            );

            let tp_h_base = self.centered_baseline(test_y + 16);
            self.draw_ui_text(
                canvas,
                "Resultado de la prueba",
                test_x + 14,
                tp_h_base,
                test_x + test_w - 10,
                theme.text_secondary(),
                tokens::DETAIL,
                true,
            );

            let mut step_y = test_y + 34;
            for step in &state.test_steps {
                let s_base = self.centered_baseline(step_y + 11);
                let (mark_c, text_c) = if step.ok {
                    (theme.accent_color(), theme.foreground)
                } else {
                    (theme.red(), theme.red())
                };
                self.draw_mono_text(
                    canvas,
                    &step.mark,
                    test_x + 14,
                    s_base,
                    test_x + 30,
                    mark_c,
                    tokens::DETAIL,
                    true,
                );
                self.draw_ui_text(
                    canvas,
                    &step.text,
                    test_x + 34,
                    s_base,
                    test_x + test_w - 70,
                    text_c,
                    tokens::DETAIL,
                    false,
                );
                let ms_str = format!("{} ms", step.elapsed_ms);
                self.draw_mono_text(
                    canvas,
                    &ms_str,
                    test_x + test_w - 60,
                    s_base,
                    test_x + test_w - 10,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                step_y += 24;
            }

            // Aviso de permisos de clave insegura
            if let Some((warning_msg, _)) = &state.test_warning {
                let warn_rect = (test_x, test_y + test_panel_h.saturating_sub(38), test_w, 38);
                let yel = theme.yellow();
                Self::draw_rect(
                    canvas,
                    warn_rect.0,
                    warn_rect.1,
                    warn_rect.2,
                    warn_rect.3,
                    Color::from_rgba(yel.r, yel.g, yel.b, 25),
                );
                self.draw_lucide_icon(
                    canvas,
                    LucideIcon::Info,
                    warn_rect.0 + 12,
                    warn_rect.1 + 10,
                    16.0,
                    yel,
                );
                let w_base = self.centered_baseline(warn_rect.1 + 19);
                self.draw_ui_text(
                    canvas,
                    warning_msg,
                    warn_rect.0 + 36,
                    w_base,
                    warn_rect.0 + warn_rect.2 - 140,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );

                let fix_btn = (warn_rect.0 + warn_rect.2 - 130, warn_rect.1 + 7, 120, 24);
                Self::draw_rect(
                    canvas,
                    fix_btn.0,
                    fix_btn.1,
                    fix_btn.2,
                    fix_btn.3,
                    theme.background,
                );
                canvas.rounded_stroke(
                    fix_btn.0,
                    fix_btn.1,
                    fix_btn.2,
                    fix_btn.3,
                    tokens::RADIUS_CONTROL,
                    theme.border_color(),
                );
                let fix_base = self.centered_baseline(fix_btn.1 + 12);
                self.draw_ui_text(
                    canvas,
                    "Corregir permisos",
                    fix_btn.0 + 8,
                    fix_base,
                    fix_btn.0 + fix_btn.2 - 4,
                    theme.foreground,
                    tokens::MICRO,
                    false,
                );
            }
        }
    }

    /// Vista 06: Cuadrícula de datos, barra de filtro WHERE/ORDER BY, inspector y cambios pendientes.
    fn render_db_view_browse(
        &mut self,
        canvas: &mut Canvas,
        rects: &DbViewRects,
        state: &DbBrowseState,
        _is_prod_write: bool,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        // Empty state si no hay columnas ni tabla seleccionada
        if state.columns.is_empty() && state.table_name.is_empty() {
            let (cx, cy, cw, ch) = rects.main_area;
            let center_x = cx + cw / 2;
            let center_y = cy + ch / 2;
            self.draw_lucide_icon(
                canvas,
                LucideIcon::Database,
                center_x.saturating_sub(24),
                center_y.saturating_sub(48),
                48.0,
                theme.text_secondary(),
            );
            let t_base = self.centered_baseline(center_y + 16);
            self.draw_ui_text_tracked(
                canvas,
                "NINGUNA TABLA SELECCIONADA",
                center_x.saturating_sub(110),
                t_base,
                center_x + 150,
                theme.foreground,
                tokens::BODY,
                true,
                1.2,
            );
            let s_base = self.centered_baseline(center_y + 40);
            self.draw_ui_text(
                canvas,
                "Selecciona una tabla en el panel lateral o pulsa «Conectar» en Conexiones.",
                center_x.saturating_sub(220),
                s_base,
                center_x + 250,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            return;
        }

        // 1. Barra de filtros WHERE y ORDER BY
        if let Some((bx, by, bw, bh)) = rects.filter_bar {
            Self::draw_rect(canvas, bx, by, bw, bh, theme.bar_bg());
            Self::draw_rect(canvas, bx, by + bh - 1, bw, 1, theme.chrome_line());

            let where_base = self.centered_baseline(by + bh / 2);
            self.draw_mono_text(
                canvas,
                "WHERE",
                bx + 14,
                where_base,
                bx + 70,
                theme.mauve(),
                tokens::DETAIL,
                true,
            );

            if let Some((wx, wy, ww, wh)) = rects.where_input {
                Self::draw_rect(canvas, wx, wy, ww, wh, theme.background);
                let is_active = state.active_filter == Some(BrowseFilterField::Where);
                let border_c = if is_active {
                    theme.accent_color()
                } else {
                    theme.border_color()
                };
                canvas.rounded_stroke(wx, wy, ww, wh, tokens::RADIUS_CONTROL, border_c);
                let w_base = self.centered_baseline(wy + wh / 2);
                self.draw_mono_text(
                    canvas,
                    &state.where_input.text,
                    wx + 10,
                    w_base,
                    wx + ww - 8,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
                if is_active && cursor_visible {
                    let cur_x = wx + 10 + state.where_input.text.len() * 7;
                    if cur_x + 2 < wx + ww {
                        Self::draw_rect(
                            canvas,
                            cur_x,
                            wy + 6,
                            2,
                            wh.saturating_sub(12),
                            theme.accent_color(),
                        );
                    }
                }
            }

            let order_lbl_x = rects
                .where_input
                .map(|w| w.0 + w.2 + 14)
                .unwrap_or(bx + 300);
            self.draw_mono_text(
                canvas,
                "ORDER BY",
                order_lbl_x,
                where_base,
                order_lbl_x + 80,
                theme.mauve(),
                tokens::DETAIL,
                true,
            );

            if let Some((ox, oy, ow, oh)) = rects.order_input {
                Self::draw_rect(canvas, ox, oy, ow, oh, theme.background);
                let is_active = state.active_filter == Some(BrowseFilterField::OrderBy);
                let border_c = if is_active {
                    theme.accent_color()
                } else {
                    theme.border_color()
                };
                canvas.rounded_stroke(ox, oy, ow, oh, tokens::RADIUS_CONTROL, border_c);
                let o_base = self.centered_baseline(oy + oh / 2);
                self.draw_mono_text(
                    canvas,
                    &state.order_by_input.text,
                    ox + 10,
                    o_base,
                    ox + ow - 8,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
                if is_active && cursor_visible {
                    let cur_x = ox + 10 + state.order_by_input.text.len() * 7;
                    if cur_x + 2 < ox + ow {
                        Self::draw_rect(
                            canvas,
                            cur_x,
                            oy + 6,
                            2,
                            oh.saturating_sub(12),
                            theme.accent_color(),
                        );
                    }
                }
            }
        }

        // 2. Cuadrícula de datos
        if let Some((gx, gy, gw, gh)) = rects.grid_rect {
            Self::draw_rect(canvas, gx, gy, gw, gh, theme.background);

            // Cabecera de columnas
            let col_h = 36;
            Self::draw_rect(canvas, gx, gy, gw, col_h, theme.mantle());
            Self::draw_rect(canvas, gx, gy + col_h - 1, gw, 1, theme.chrome_line());

            let col_w = (gw.saturating_sub(50)) / state.columns.len().max(1);
            let mut cur_col_x = gx + 50;

            for col in &state.columns {
                let name_base = self.centered_baseline(gy + 14);
                self.draw_ui_text(
                    canvas,
                    &col.name,
                    cur_col_x + 8,
                    name_base,
                    cur_col_x + col_w - 30,
                    theme.foreground,
                    tokens::DETAIL,
                    true,
                );

                if col.is_pk {
                    self.draw_mono_text(
                        canvas,
                        "PK",
                        cur_col_x + col_w - 24,
                        name_base,
                        cur_col_x + col_w - 4,
                        theme.yellow(),
                        tokens::MICRO,
                        true,
                    );
                } else if col.is_fk {
                    self.draw_mono_text(
                        canvas,
                        "FK",
                        cur_col_x + col_w - 24,
                        name_base,
                        cur_col_x + col_w - 4,
                        theme.accent_color(),
                        tokens::MICRO,
                        true,
                    );
                }

                let type_base = self.centered_baseline(gy + 26);
                self.draw_mono_text(
                    canvas,
                    &col.data_type,
                    cur_col_x + 8,
                    type_base,
                    cur_col_x + col_w - 6,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                cur_col_x += col_w;
            }

            // Filas de datos
            let row_h = 32;
            let mut row_y = gy + col_h;
            for (r_idx, row) in state.rows.iter().enumerate().skip(state.scroll_y) {
                if row_y + row_h > gy + gh {
                    break;
                }
                let is_sel = state.selected_row == Some(r_idx);
                if is_sel {
                    Self::draw_rect(
                        canvas,
                        gx,
                        row_y,
                        gw,
                        row_h,
                        Color::from_rgba(
                            theme.accent_color().r,
                            theme.accent_color().g,
                            theme.accent_color().b,
                            20,
                        ),
                    );
                }

                // Número de fila
                let n_base = self.centered_baseline(row_y + row_h / 2);
                let num_str = format!("{}", r_idx + 1);
                self.draw_mono_text(
                    canvas,
                    &num_str,
                    gx + 12,
                    n_base,
                    gx + 42,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );

                let mut cell_x = gx + 50;
                for (c_idx, cell_val) in row.cells.iter().enumerate() {
                    let is_modified = row.modified_cols.contains(&c_idx);
                    let editing_inp = state
                        .editing_cell
                        .as_ref()
                        .filter(|(r, c, _)| *r == r_idx && *c == c_idx)
                        .map(|(_, _, inp)| inp);

                    if let Some(inp) = editing_inp {
                        Self::draw_rect(
                            canvas,
                            cell_x + 2,
                            row_y + 2,
                            col_w.saturating_sub(4),
                            row_h.saturating_sub(4),
                            theme.background,
                        );
                        canvas.rounded_stroke(
                            cell_x + 2,
                            row_y + 2,
                            col_w.saturating_sub(4),
                            row_h.saturating_sub(4),
                            tokens::RADIUS_CONTROL,
                            theme.accent_color(),
                        );
                        let val_base = self.centered_baseline(row_y + row_h / 2);
                        self.draw_mono_text(
                            canvas,
                            &inp.text,
                            cell_x + 8,
                            val_base,
                            cell_x + col_w - 8,
                            theme.foreground,
                            tokens::BODY,
                            true,
                        );
                        if cursor_visible {
                            let cur_x = cell_x + 8 + inp.text.len() * 7;
                            if cur_x + 2 < cell_x + col_w {
                                Self::draw_rect(
                                    canvas,
                                    cur_x,
                                    row_y + 6,
                                    2,
                                    row_h.saturating_sub(12),
                                    theme.accent_color(),
                                );
                            }
                        }
                    } else {
                        if is_modified {
                            let yel = theme.yellow();
                            Self::draw_rect(
                                canvas,
                                cell_x + 2,
                                row_y + 2,
                                col_w.saturating_sub(4),
                                row_h.saturating_sub(4),
                                Color::from_rgba(yel.r, yel.g, yel.b, 35),
                            );
                        }

                        let val_base = self.centered_baseline(row_y + row_h / 2);
                        self.draw_ui_text(
                            canvas,
                            cell_val,
                            cell_x + 8,
                            val_base,
                            cell_x + col_w - 8,
                            theme.foreground,
                            tokens::BODY,
                            false,
                        );
                    }
                    cell_x += col_w;
                }
                Self::draw_rect(canvas, gx, row_y + row_h - 1, gw, 1, theme.chrome_line());
                row_y += row_h;
            }
        }

        // 3. Barra de cambios pendientes (en la parte inferior)
        if let Some((px, py, pw, ph)) = rects.pending_bar {
            let yel = theme.yellow();
            Self::draw_rect(
                canvas,
                px,
                py,
                pw,
                ph,
                Color::from_rgba(yel.r, yel.g, yel.b, 25),
            );
            Self::draw_rect(canvas, px, py, pw, 1, yel);

            // Chip amarillo: N cambios sin aplicar
            let chip_w = 150;
            let chip_rect = (px + 14, py + 8, chip_w, 28);
            Self::draw_rect(
                canvas,
                chip_rect.0,
                chip_rect.1,
                chip_rect.2,
                chip_rect.3,
                Color::from_rgba(yel.r, yel.g, yel.b, 45),
            );
            let chip_base = self.centered_baseline(py + ph / 2);
            let chip_text = format!("{} cambios sin aplicar", state.pending_changes.len());
            self.draw_ui_text(
                canvas,
                &chip_text,
                chip_rect.0 + 8,
                chip_base,
                chip_rect.0 + chip_rect.2 - 4,
                yel,
                tokens::DETAIL,
                true,
            );

            // Preview del SQL generado
            let sql_preview = state.generated_sql();
            self.draw_mono_text(
                canvas,
                &sql_preview,
                px + chip_w + 30,
                chip_base,
                px + pw - 270,
                theme.foreground,
                tokens::DETAIL,
                false,
            );

            // Botones: Descartar y «Revisar y aplicar…»
            if let Some((dx, dy, dw, dh)) = rects.pending_discard_btn {
                Self::draw_rect(canvas, dx, dy, dw, dh, theme.background);
                canvas.rounded_stroke(dx, dy, dw, dh, tokens::RADIUS_CONTROL, theme.border_color());
                let d_base = self.centered_baseline(dy + dh / 2);
                self.draw_ui_text(
                    canvas,
                    "Descartar",
                    dx + 12,
                    d_base,
                    dx + dw - 6,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }
            if let Some((ax, ay, aw, ah)) = rects.pending_apply_btn {
                Self::draw_rect(canvas, ax, ay, aw, ah, theme.accent_color());
                let a_base = self.centered_baseline(ay + ah / 2);
                self.draw_ui_text(
                    canvas,
                    "Revisar y aplicar…",
                    ax + 12,
                    a_base,
                    ax + aw - 6,
                    theme.background,
                    tokens::DETAIL,
                    true,
                );
            }
        }

        // 4. Panel lateral inspector de fila (derecha)
        if let Some((ix, iy, iw, ih)) = rects.inspector_rect {
            Self::draw_rect(canvas, ix, iy, iw, ih, theme.mantle());
            Self::draw_rect(canvas, ix, iy, 1, ih, theme.chrome_line());

            let head_base = self.centered_baseline(iy + 20);
            self.draw_ui_text(
                canvas,
                "Fila",
                ix + 16,
                head_base,
                ix + 60,
                theme.foreground,
                tokens::BODY,
                true,
            );
            if let Some(r_idx) = state.selected_row {
                let id_str = format!(
                    "id {}",
                    state.rows.get(r_idx).map(|r| r.id.as_str()).unwrap_or("")
                );
                self.draw_mono_text(
                    canvas,
                    &id_str,
                    ix + 56,
                    head_base,
                    ix + iw - 16,
                    theme.accent_color(),
                    tokens::BODY,
                    true,
                );
            }

            let mut cur_y = iy + 44;
            if let Some(r_idx) = state.selected_row {
                if let Some(row) = state.rows.get(r_idx) {
                    for (c_idx, col) in state.columns.iter().enumerate() {
                        let c_val = row.cells.get(c_idx).map(|s| s.as_str()).unwrap_or("");
                        let lbl_base = self.centered_baseline(cur_y + 10);
                        self.draw_ui_text(
                            canvas,
                            &col.name,
                            ix + 16,
                            lbl_base,
                            ix + iw - 16,
                            theme.text_secondary(),
                            tokens::MICRO,
                            true,
                        );

                        let val_rect = (ix + 16, cur_y + 18, iw.saturating_sub(32), 26);
                        Self::draw_rect(
                            canvas,
                            val_rect.0,
                            val_rect.1,
                            val_rect.2,
                            val_rect.3,
                            theme.background,
                        );
                        canvas.rounded_stroke(
                            val_rect.0,
                            val_rect.1,
                            val_rect.2,
                            val_rect.3,
                            tokens::RADIUS_CONTROL,
                            theme.border_color(),
                        );
                        let v_base = self.centered_baseline(val_rect.1 + val_rect.3 / 2);
                        self.draw_ui_text(
                            canvas,
                            c_val,
                            val_rect.0 + 8,
                            v_base,
                            val_rect.0 + val_rect.2 - 6,
                            theme.foreground,
                            tokens::DETAIL,
                            false,
                        );
                        cur_y += 50;
                    }
                }
            }

            // Sección de relaciones navegables
            cur_y += 10;
            let rel_lbl_base = self.centered_baseline(cur_y + 10);
            self.draw_ui_text_tracked(
                canvas,
                "RELACIONES",
                ix + 16,
                rel_lbl_base,
                ix + iw - 16,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );
            cur_y += 24;

            for rel in &state.relations {
                let btn_rect = (ix + 16, cur_y, iw.saturating_sub(32), 28);
                Self::draw_rect(
                    canvas,
                    btn_rect.0,
                    btn_rect.1,
                    btn_rect.2,
                    btn_rect.3,
                    theme.background,
                );
                canvas.rounded_stroke(
                    btn_rect.0,
                    btn_rect.1,
                    btn_rect.2,
                    btn_rect.3,
                    tokens::RADIUS_CONTROL,
                    theme.border_color(),
                );

                let r_base = self.centered_baseline(btn_rect.1 + btn_rect.3 / 2);
                self.draw_mono_text(
                    canvas,
                    &rel.table,
                    btn_rect.0 + 10,
                    r_base,
                    btn_rect.0 + 100,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
                self.draw_ui_text(
                    canvas,
                    &rel.count_label,
                    btn_rect.0 + 110,
                    r_base,
                    btn_rect.0 + btn_rect.2 - 24,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                self.draw_mono_text(
                    canvas,
                    "→",
                    btn_rect.0 + btn_rect.2 - 18,
                    r_base,
                    btn_rect.0 + btn_rect.2 - 4,
                    theme.accent_color(),
                    tokens::DETAIL,
                    true,
                );
                cur_y += 34;
            }
        }
    }

    /// Vista 07: Editor SQL con resaltado de sintaxis, barra de herramientas y pestañas de resultados.
    fn render_db_view_query(
        &mut self,
        canvas: &mut Canvas,
        rects: &DbViewRects,
        db: &DbTabState,
        cursor_visible: bool,
        theme: &Theme,
    ) {
        let state = &db.query;

        // 1. Barra de herramientas del editor SQL
        if let Some((tx, ty, tw, th)) = rects.query_toolbar {
            Self::draw_rect(canvas, tx, ty, tw, th, theme.bar_bg());
            Self::draw_rect(canvas, tx, ty + th - 1, tw, 1, theme.chrome_line());

            if let Some((ex, ey, ew, eh)) = rects.query_exec_btn {
                Self::draw_rect(canvas, ex, ey, ew, eh, theme.accent_color());
                self.draw_lucide_icon(
                    canvas,
                    LucideIcon::ChevronRight,
                    ex + 8,
                    ey + 7,
                    14.0,
                    theme.background,
                );
                let b_base = self.centered_baseline(ey + eh / 2);
                self.draw_ui_text(
                    canvas,
                    "Ejecutar Ctrl+↵",
                    ex + 24,
                    b_base,
                    ex + ew - 6,
                    theme.background,
                    tokens::DETAIL,
                    true,
                );
            }

            if let Some((ex, ey, ew, eh)) = rects.query_explain_btn {
                Self::draw_rect(canvas, ex, ey, ew, eh, theme.background);
                canvas.rounded_stroke(ex, ey, ew, eh, tokens::RADIUS_CONTROL, theme.border_color());
                let b_base = self.centered_baseline(ey + eh / 2);
                self.draw_ui_text(
                    canvas,
                    "Explicar",
                    ex + 12,
                    b_base,
                    ex + ew - 8,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            if let Some((fx, fy, fw, fh)) = rects.query_format_btn {
                Self::draw_rect(canvas, fx, fy, fw, fh, theme.background);
                canvas.rounded_stroke(fx, fy, fw, fh, tokens::RADIUS_CONTROL, theme.border_color());
                let b_base = self.centered_baseline(fy + fh / 2);
                self.draw_ui_text(
                    canvas,
                    "Formatear",
                    fx + 12,
                    b_base,
                    fx + fw - 8,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            if let Some((sx, sy, sw, sh)) = rects.query_tx_toggle {
                let half_w = sw / 2;
                Self::draw_rect(canvas, sx, sy, sw, sh, theme.mantle());
                canvas.rounded_stroke(sx, sy, sw, sh, tokens::RADIUS_CONTROL, theme.border_color());

                let auto_bg = if state.auto_commit {
                    theme.background
                } else {
                    theme.mantle()
                };
                let tx_bg = if !state.auto_commit {
                    theme.background
                } else {
                    theme.mantle()
                };
                Self::draw_rect(canvas, sx, sy, half_w, sh, auto_bg);
                Self::draw_rect(canvas, sx + half_w, sy, sw - half_w, sh, tx_bg);

                let t_base = self.centered_baseline(sy + sh / 2);
                self.draw_ui_text(
                    canvas,
                    "Auto-commit",
                    sx + 6,
                    t_base,
                    sx + half_w - 4,
                    if state.auto_commit {
                        theme.accent_color()
                    } else {
                        theme.text_secondary()
                    },
                    tokens::MICRO,
                    state.auto_commit,
                );
                self.draw_ui_text(
                    canvas,
                    "Transacción",
                    sx + half_w + 6,
                    t_base,
                    sx + sw - 4,
                    if !state.auto_commit {
                        theme.accent_color()
                    } else {
                        theme.text_secondary()
                    },
                    tokens::MICRO,
                    !state.auto_commit,
                );
            }

            if let Some((bx, by, bw, bh)) = rects.query_save_btn {
                Self::draw_rect(canvas, bx, by, bw, bh, theme.background);
                canvas.rounded_stroke(bx, by, bw, bh, tokens::RADIUS_CONTROL, theme.border_color());
                let b_base = self.centered_baseline(by + bh / 2);
                self.draw_ui_text(
                    canvas,
                    "Guardar",
                    bx + 16,
                    b_base,
                    bx + bw - 6,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            // Chip de conexión activa a la derecha
            if tw > 750 {
                let r_pad = tx + tw.saturating_sub(240);
                let c_dot = if db.is_connected {
                    theme.green()
                } else {
                    theme.text_secondary()
                };
                Self::draw_rect(canvas, r_pad, ty + 15, 8, 8, c_dot);
                let conn_base = self.centered_baseline(ty + th / 2);
                self.draw_ui_text(
                    canvas,
                    &db.active_connection_name,
                    r_pad + 14,
                    conn_base,
                    r_pad + 100,
                    theme.foreground,
                    tokens::DETAIL,
                    true,
                );

                let env_txt = match db.active_connection_env {
                    Env::Prod => "PROD",
                    Env::Stg => "STG",
                    Env::Local => "LOCAL",
                };
                let (env_bg, env_fg) = match db.active_connection_env {
                    Env::Prod => (
                        Color::from_rgba(theme.red().r, theme.red().g, theme.red().b, 50),
                        theme.red(),
                    ),
                    Env::Stg => (
                        Color::from_rgba(theme.yellow().r, theme.yellow().g, theme.yellow().b, 50),
                        theme.yellow(),
                    ),
                    Env::Local => (
                        Color::from_rgba(
                            theme.accent_color().r,
                            theme.accent_color().g,
                            theme.accent_color().b,
                            50,
                        ),
                        theme.accent_color(),
                    ),
                };
                let env_chip_x = r_pad + 106;
                Self::draw_rect(canvas, env_chip_x, ty + 8, 44, 22, env_bg);
                self.draw_mono_text(
                    canvas,
                    env_txt,
                    env_chip_x + 6,
                    conn_base,
                    env_chip_x + 40,
                    env_fg,
                    tokens::MICRO,
                    true,
                );

                if db.connections.read_only {
                    let ro_x = env_chip_x + 50;
                    Self::draw_rect(canvas, ro_x, ty + 8, 64, 22, theme.mantle());
                    canvas.rounded_stroke(
                        ro_x,
                        ty + 8,
                        64,
                        22,
                        tokens::RADIUS_CONTROL,
                        theme.border_color(),
                    );
                    self.draw_ui_text(
                        canvas,
                        "Solo lect.",
                        ro_x + 6,
                        conn_base,
                        ro_x + 60,
                        theme.text_secondary(),
                        tokens::MICRO,
                        false,
                    );
                }
            }
        }

        // 2. Área del editor SQL
        let (cur_line, cur_col) = state.cursor_line_col();
        if let Some((ex, ey, ew, eh)) = rects.query_editor_rect {
            Self::draw_rect(canvas, ex, ey, ew, eh, theme.background);

            let gutter_w = 46;
            Self::draw_rect(canvas, ex, ey, gutter_w, eh, theme.mantle());
            Self::draw_rect(canvas, ex + gutter_w - 1, ey, 1, eh, theme.chrome_line());

            let line_h = 22;
            let mut l_y = ey + 8;
            for (idx, line) in state.editor_input.text.lines().enumerate() {
                if l_y + line_h > ey + eh {
                    break;
                }
                let line_num = idx + 1;
                let base = self.centered_baseline(l_y + line_h / 2);
                let num_str = format!("{}", line_num);
                self.draw_mono_text(
                    canvas,
                    &num_str,
                    ex + 10,
                    base,
                    ex + gutter_w - 8,
                    if line_num == cur_line {
                        theme.foreground
                    } else {
                        theme.text_secondary()
                    },
                    tokens::DETAIL,
                    line_num == cur_line,
                );

                // Dibujar tokens con resaltado sintáctico
                let mut token_x = ex + gutter_w + 14;
                let tokens = Self::tokenize_sql(line);
                for (tok_str, tok_kind) in tokens {
                    let color = match tok_kind {
                        SqlTokenKind::Keyword => theme.mauve(),
                        SqlTokenKind::StringLit => theme.green(),
                        SqlTokenKind::NumberLit => theme.peach(),
                        SqlTokenKind::Comment => theme.text_secondary(),
                        SqlTokenKind::Other => theme.foreground,
                    };
                    let w = self.draw_mono_text(
                        canvas,
                        tok_str,
                        token_x,
                        base,
                        ex + ew - 14,
                        color,
                        tokens::BODY,
                        false,
                    );
                    token_x += w;
                }

                // Cursor parpadeante si estamos en esta línea
                if line_num == cur_line && cursor_visible {
                    let prefix = if cur_col > 1 {
                        line.chars().take(cur_col - 1).collect::<String>()
                    } else {
                        String::new()
                    };
                    let approx_char_w = 8;
                    let cursor_x = (ex + gutter_w + 14 + prefix.chars().count() * approx_char_w)
                        .min(ex + ew - 6);
                    Self::draw_rect(
                        canvas,
                        cursor_x,
                        l_y + 2,
                        2,
                        line_h - 4,
                        theme.accent_color(),
                    );
                }

                l_y += line_h;
            }
        }

        // 3. Barra de estado inferior del Editor SQL
        if let Some((sx, sy, sw, sh)) = rects.query_status_bar {
            Self::draw_rect(canvas, sx, sy, sw, sh, theme.mantle());
            Self::draw_rect(canvas, sx, sy, sw, 1, theme.chrome_line());

            let base = self.centered_baseline(sy + sh / 2);
            let engine_str = match db.active_connection_engine {
                Engine::Postgres => "PostgreSQL",
                Engine::Mysql => "MySQL",
                Engine::Mariadb => "MariaDB",
                Engine::SqlServer => "SQL Server",
            };
            self.draw_ui_text(
                canvas,
                engine_str,
                sx + 14,
                base,
                sx + 120,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            let pos_str = format!("Ln {}, Col {}", cur_line, cur_col);
            self.draw_mono_text(
                canvas,
                &pos_str,
                sx + 130,
                base,
                sx + 240,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            let stmt_str = format!("1 sentencia · {}", state.statement_type());
            self.draw_ui_text(
                canvas,
                &stmt_str,
                sx + 250,
                base,
                sx + sw - 210,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            if sw > 600 {
                self.draw_ui_text(
                    canvas,
                    "Ctrl+↵ ejecuta · Esc limpia",
                    sx + sw.saturating_sub(190),
                    base,
                    sx + sw - 14,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
            }
        }

        // 4. Barra de pestañas de resultados (Grid, Plan, Messages)
        if let Some((rx, ry, rw, rh)) = rects.query_results_nav {
            Self::draw_rect(canvas, rx, ry, rw, rh, theme.mantle());
            Self::draw_rect(canvas, rx, ry + rh - 1, rw, 1, theme.chrome_line());

            let res_tabs = ["Resultados", "Plan de ejecución", "Mensajes"];
            for (i, &t_rect) in rects.query_results_tabs.iter().enumerate() {
                if i >= res_tabs.len() {
                    break;
                }
                let is_sel = matches!(
                    (i, state.results_tab),
                    (0, DbQueryResultsTab::Grid)
                        | (1, DbQueryResultsTab::Plan)
                        | (2, DbQueryResultsTab::Messages)
                );
                let (tx, ty, tw, th) = t_rect;
                if is_sel {
                    Self::draw_rect(canvas, tx, ty + th - 2, tw, 2, theme.accent_color());
                }
                let b = self.centered_baseline(ty + th / 2);
                self.draw_ui_text(
                    canvas,
                    res_tabs[i],
                    tx + 12,
                    b,
                    tx + tw - 6,
                    if is_sel {
                        theme.foreground
                    } else {
                        theme.text_secondary()
                    },
                    tokens::DETAIL,
                    is_sel,
                );
            }

            // Métrica de tiempo y filas o error
            if rw > 650 {
                let s_base = self.centered_baseline(ry + rh / 2);
                let stat_x = rx + rw.saturating_sub(320);
                if state.error_message.is_some() {
                    Self::draw_rect(canvas, stat_x, ry + 15, 6, 6, theme.red());
                    self.draw_ui_text(
                        canvas,
                        "Error en consulta",
                        stat_x + 12,
                        s_base,
                        stat_x + 130,
                        theme.red(),
                        tokens::DETAIL,
                        true,
                    );
                } else {
                    Self::draw_rect(canvas, stat_x, ry + 15, 6, 6, theme.green());
                    let stat_str = format!("{} filas · {} ms", state.rows_count, state.elapsed_ms);
                    self.draw_mono_text(
                        canvas,
                        &stat_str,
                        stat_x + 12,
                        s_base,
                        stat_x + 130,
                        theme.text_secondary(),
                        tokens::DETAIL,
                        false,
                    );
                }
            }

            // Botones de exportación: CSV, JSON, Copiar
            if let Some((cx, cy, cw, ch)) = rects.query_export_csv_btn {
                Self::draw_rect(canvas, cx, cy, cw, ch, theme.background);
                canvas.rounded_stroke(cx, cy, cw, ch, tokens::RADIUS_CONTROL, theme.border_color());
                let b = self.centered_baseline(cy + ch / 2);
                self.draw_ui_text(
                    canvas,
                    "CSV",
                    cx + 8,
                    b,
                    cx + cw - 4,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
            }

            if let Some((jx, jy, jw, jh)) = rects.query_export_json_btn {
                Self::draw_rect(canvas, jx, jy, jw, jh, theme.background);
                canvas.rounded_stroke(jx, jy, jw, jh, tokens::RADIUS_CONTROL, theme.border_color());
                let b = self.centered_baseline(jy + jh / 2);
                self.draw_ui_text(
                    canvas,
                    "JSON",
                    jx + 8,
                    b,
                    jx + jw - 4,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
            }

            if let Some((cpx, cpy, cpw, cph)) = rects.query_copy_btn {
                Self::draw_rect(canvas, cpx, cpy, cpw, cph, theme.background);
                canvas.rounded_stroke(
                    cpx,
                    cpy,
                    cpw,
                    cph,
                    tokens::RADIUS_CONTROL,
                    theme.border_color(),
                );
                let b = self.centered_baseline(cpy + cph / 2);
                self.draw_ui_text(
                    canvas,
                    "Copiar",
                    cpx + 8,
                    b,
                    cpx + cpw - 4,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
            }
        }

        // 5. Contenido de la pestaña activa de resultados
        if let Some((gx, gy, gw, gh)) = rects.query_results_grid {
            match state.results_tab {
                DbQueryResultsTab::Grid => {
                    Self::draw_rect(canvas, gx, gy, gw, gh, theme.background);
                    if state.is_executing {
                        self.render_loading_state(
                            canvas,
                            (gx, gy, gw, gh),
                            "Ejecutando consulta SQL…",
                            false,
                            theme,
                        );
                    } else if let Some(ref err_msg) = state.error_message {
                        self.render_error_state(
                            canvas,
                            (gx, gy, gw, gh),
                            "ERROR DE CONSULTA SQL",
                            "La ejecución no se pudo completar",
                            err_msg,
                            &[],
                            &[],
                            None,
                            theme,
                        );
                    } else if state.result_columns.is_empty() || state.result_rows.is_empty() {
                        self.render_empty_state(
                            canvas,
                            (gx, gy, gw, gh),
                            None,
                            &[],
                            "SIN RESULTADOS",
                            "Ejecuta una consulta SQL con Ctrl+↵ para visualizar los datos",
                            &[],
                            None,
                            theme,
                        );
                    } else {
                        let col_h = 32;
                        Self::draw_rect(canvas, gx, gy, gw, col_h, theme.mantle());
                        Self::draw_rect(canvas, gx, gy + col_h - 1, gw, 1, theme.chrome_line());

                        let col_w = gw / state.result_columns.len().max(1);
                        let mut cx = gx;
                        for col in &state.result_columns {
                            let b = self.centered_baseline(gy + col_h / 2);
                            self.draw_mono_text(
                                canvas,
                                col,
                                cx + 12,
                                b,
                                cx + col_w - 10,
                                theme.text_secondary(),
                                tokens::DETAIL,
                                true,
                            );
                            cx += col_w;
                        }

                        let row_h = 30;
                        let mut ry = gy + col_h;
                        for row in state.result_rows.iter().skip(state.results_scroll_y) {
                            if ry + row_h > gy + gh {
                                break;
                            }
                            let mut rcx = gx;
                            for cell in row {
                                let b = self.centered_baseline(ry + row_h / 2);
                                self.draw_ui_text(
                                    canvas,
                                    cell,
                                    rcx + 12,
                                    b,
                                    rcx + col_w - 10,
                                    theme.foreground,
                                    tokens::BODY,
                                    false,
                                );
                                rcx += col_w;
                            }
                            Self::draw_rect(canvas, gx, ry + row_h - 1, gw, 1, theme.chrome_line());
                            ry += row_h;
                        }
                    }
                }
                DbQueryResultsTab::Plan => {
                    Self::draw_rect(canvas, gx, gy, gw, gh, theme.background);
                    if state.execution_plan.is_empty() {
                        self.render_empty_state(
                            canvas,
                            (gx, gy, gw, gh),
                            None,
                            &[],
                            "PLAN DE EJECUCIÓN NO DISPONIBLE",
                            "Pulsa el botón «Explicar» o añade EXPLAIN a tu sentencia SQL",
                            &[],
                            None,
                            theme,
                        );
                    } else {
                        let mut cur_y = gy + 14;
                        let line_h = 22;
                        for line in state.execution_plan.lines() {
                            if cur_y + line_h > gy + gh {
                                break;
                            }
                            let b = self.centered_baseline(cur_y + line_h / 2);
                            self.draw_mono_text(
                                canvas,
                                line,
                                gx + 18,
                                b,
                                gx + gw - 18,
                                theme.foreground,
                                tokens::BODY,
                                false,
                            );
                            cur_y += line_h;
                        }
                    }
                }
                DbQueryResultsTab::Messages => {
                    Self::draw_rect(canvas, gx, gy, gw, gh, theme.background);
                    let mut cur_y = gy + 14;
                    if let Some(ref err) = state.error_message {
                        let card_h = 60;
                        let err_bg =
                            Color::from_rgba(theme.red().r, theme.red().g, theme.red().b, 25);
                        Self::draw_rect(
                            canvas,
                            gx + 16,
                            cur_y,
                            gw.saturating_sub(32),
                            card_h,
                            err_bg,
                        );
                        canvas.rounded_stroke(
                            gx + 16,
                            cur_y,
                            gw.saturating_sub(32),
                            card_h,
                            tokens::RADIUS_CONTROL,
                            theme.red(),
                        );
                        self.draw_lucide_icon(
                            canvas,
                            LucideIcon::X,
                            gx + 26,
                            cur_y + 12,
                            18.0,
                            theme.red(),
                        );
                        let b = self.centered_baseline(cur_y + 22);
                        self.draw_ui_text(
                            canvas,
                            "Error de ejecución SQL",
                            gx + 54,
                            b,
                            gx + gw - 40,
                            theme.red(),
                            tokens::BODY,
                            true,
                        );
                        let b2 = self.centered_baseline(cur_y + 44);
                        self.draw_mono_text(
                            canvas,
                            err,
                            gx + 54,
                            b2,
                            gx + gw - 40,
                            theme.foreground,
                            tokens::DETAIL,
                            false,
                        );
                        cur_y += card_h + 14;
                    }

                    if state.messages.is_empty() && state.error_message.is_none() {
                        self.render_empty_state(
                            canvas,
                            (gx, gy, gw, gh),
                            None,
                            &[],
                            "SIN MENSAJES",
                            "Los avisos, recuentos de filas afectadas y errores aparecerán aquí",
                            &[],
                            None,
                            theme,
                        );
                    } else {
                        let line_h = 24;
                        for msg in &state.messages {
                            if cur_y + line_h > gy + gh {
                                break;
                            }
                            Self::draw_rect(canvas, gx + 20, cur_y + 9, 6, 6, theme.accent_color());
                            let b = self.centered_baseline(cur_y + line_h / 2);
                            self.draw_ui_text(
                                canvas,
                                msg,
                                gx + 34,
                                b,
                                gx + gw - 20,
                                theme.foreground,
                                tokens::BODY,
                                false,
                            );
                            cur_y += line_h;
                        }
                    }
                }
            }
        }
    }

    /// Vista 08: Esquema y tarjetas de tablas con PK/FK y detalle.
    fn render_db_view_schema(
        &mut self,
        canvas: &mut Canvas,
        rects: &DbViewRects,
        state: &DbSchemaState,
        theme: &Theme,
    ) {
        if state.tables_list.is_empty() {
            let (cx, cy, cw, ch) = rects.main_area;
            let center_x = cx + cw / 2;
            let center_y = cy + ch / 2;
            self.draw_lucide_icon(
                canvas,
                LucideIcon::Database,
                center_x.saturating_sub(24),
                center_y.saturating_sub(48),
                48.0,
                theme.text_secondary(),
            );
            let t_base = self.centered_baseline(center_y + 16);
            self.draw_ui_text_tracked(
                canvas,
                "ESQUEMA NO DISPONIBLE",
                center_x.saturating_sub(90),
                t_base,
                center_x + 150,
                theme.foreground,
                tokens::BODY,
                true,
                1.2,
            );
            let s_base = self.centered_baseline(center_y + 40);
            self.draw_ui_text(
                canvas,
                "Conecta a una base de datos para inspeccionar tablas y relaciones.",
                center_x.saturating_sub(210),
                s_base,
                center_x + 250,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
            return;
        }

        // 1. Panel izquierdo: Lista de tablas
        if let Some((lx, ly, lw, lh)) = rects.schema_tables_list {
            Self::draw_rect(canvas, lx, ly, lw, lh, theme.mantle());
            Self::draw_rect(canvas, lx + lw - 1, ly, 1, lh, theme.chrome_line());

            let head_base = self.centered_baseline(ly + 18);
            self.draw_ui_text_tracked(
                canvas,
                "TABLAS DEL ESQUEMA",
                lx + 14,
                head_base,
                lx + lw - 10,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );

            let mut cur_y = ly + 36;
            let row_h = 28;
            for (tbl_name, count_lbl) in &state.tables_list {
                if cur_y + row_h > ly + lh {
                    break;
                }
                let is_sel = tbl_name == &state.selected_table;
                if is_sel {
                    Self::draw_rect(
                        canvas,
                        lx + 8,
                        cur_y,
                        lw.saturating_sub(16),
                        row_h,
                        theme.background,
                    );
                }
                let r_base = self.centered_baseline(cur_y + row_h / 2);
                self.draw_ui_text(
                    canvas,
                    tbl_name,
                    lx + 16,
                    r_base,
                    lx + lw - 60,
                    if is_sel {
                        theme.foreground
                    } else {
                        theme.text_secondary()
                    },
                    tokens::BODY,
                    is_sel,
                );
                self.draw_mono_text(
                    canvas,
                    count_lbl,
                    lx + lw - 54,
                    r_base,
                    lx + lw - 10,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                cur_y += row_h;
            }
        }

        // 2. Barra superior de herramientas del lienzo de esquema
        if let Some((tx, ty, tw, th)) = rects.schema_toolbar {
            Self::draw_rect(canvas, tx, ty, tw, th, theme.bar_bg());
            Self::draw_rect(canvas, tx, ty + th - 1, tw, 1, theme.chrome_line());

            let head_base = self.centered_baseline(ty + th / 2);
            self.draw_mono_text(
                canvas,
                &state.schema_name,
                tx + 14,
                head_base,
                tx + 120,
                theme.foreground,
                tokens::BODY,
                true,
            );

            let meta_str = format!(
                "{} tablas · {} relaciones",
                state.cards.len(),
                state.relations.len()
            );
            self.draw_ui_text(
                canvas,
                &meta_str,
                tx + 90,
                head_base,
                tx + tw - 340,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // Botón Ordenar
            if let Some((ax, ay, aw, ah)) = rects.schema_arrange_btn {
                Self::draw_rect(canvas, ax, ay, aw, ah, theme.background);
                canvas.rounded_stroke(ax, ay, aw, ah, tokens::RADIUS_CONTROL, theme.border_color());
                let b = self.centered_baseline(ay + ah / 2);
                self.draw_ui_text(
                    canvas,
                    "Ordenar",
                    ax + 10,
                    b,
                    ax + aw - 6,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            // Segmentado Columnas / Claves
            if let Some((kx, ky, kw, kh)) = rects.schema_keys_toggle {
                let half_w = kw / 2;
                Self::draw_rect(canvas, kx, ky, kw, kh, theme.mantle());
                canvas.rounded_stroke(kx, ky, kw, kh, tokens::RADIUS_CONTROL, theme.border_color());

                let col_bg = if !state.only_keys {
                    theme.background
                } else {
                    theme.mantle()
                };
                let key_bg = if state.only_keys {
                    theme.background
                } else {
                    theme.mantle()
                };
                Self::draw_rect(canvas, kx, ky, half_w, kh, col_bg);
                Self::draw_rect(canvas, kx + half_w, ky, kw - half_w, kh, key_bg);

                let b = self.centered_baseline(ky + kh / 2);
                self.draw_ui_text(
                    canvas,
                    "Columnas",
                    kx + 10,
                    b,
                    kx + half_w - 4,
                    if !state.only_keys {
                        theme.accent_color()
                    } else {
                        theme.text_secondary()
                    },
                    tokens::MICRO,
                    !state.only_keys,
                );
                self.draw_ui_text(
                    canvas,
                    "Claves",
                    kx + half_w + 14,
                    b,
                    kx + kw - 4,
                    if state.only_keys {
                        theme.accent_color()
                    } else {
                        theme.text_secondary()
                    },
                    tokens::MICRO,
                    state.only_keys,
                );
            }

            // Control de Zoom: [ - | 100% | + ]
            if let Some((zx, zy, zw, zh)) = rects.schema_zoom_minus_btn {
                let full_w = zw + 36 + zw;
                Self::draw_rect(canvas, zx, zy, full_w, zh, theme.mantle());
                canvas.rounded_stroke(
                    zx,
                    zy,
                    full_w,
                    zh,
                    tokens::RADIUS_CONTROL,
                    theme.border_color(),
                );

                let b = self.centered_baseline(zy + zh / 2);
                self.draw_ui_text(
                    canvas,
                    "−",
                    zx + 8,
                    b,
                    zx + zw,
                    theme.foreground,
                    tokens::BODY,
                    false,
                );
                let zoom_lbl = format!("{}%", state.zoom_pct);
                self.draw_mono_text(
                    canvas,
                    &zoom_lbl,
                    zx + zw + 4,
                    b,
                    zx + zw + 34,
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );
                self.draw_ui_text(
                    canvas,
                    "+",
                    zx + zw + 36 + 8,
                    b,
                    zx + full_w,
                    theme.foreground,
                    tokens::BODY,
                    false,
                );
            }

            // Botón Exportar
            if let Some((ex, ey, ew, eh)) = rects.schema_export_btn {
                Self::draw_rect(canvas, ex, ey, ew, eh, theme.background);
                canvas.rounded_stroke(ex, ey, ew, eh, tokens::RADIUS_CONTROL, theme.border_color());
                let b = self.centered_baseline(ey + eh / 2);
                self.draw_ui_text(
                    canvas,
                    "Exportar",
                    ex + 10,
                    b,
                    ex + ew - 6,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }
        }

        // 3. Lienzo central: Cuadrícula blueprint, líneas de relación y tarjetas
        if let Some((cx, cy, cw, ch)) = rects.schema_cards_area {
            Self::draw_rect(canvas, cx, cy, cw, ch, theme.crust());

            // Trama punteada sutil
            let dot_color = Color::from_rgba(
                theme.text_secondary().r,
                theme.text_secondary().g,
                theme.text_secondary().b,
                22,
            );
            let mut py = cy + 12;
            while py < cy + ch {
                let mut px = cx + 12;
                while px < cx + cw {
                    Self::draw_rect(canvas, px, py, 1, 1, dot_color);
                    px += 24;
                }
                py += 24;
            }

            let area_clip: Clip = (cx, cy, cx + cw, cy + ch);
            let hdr_h = state.scale_px(34).max(18);
            let row_h = state.scale_px(26).max(16);

            // Dibujar líneas de relaciones ortogonales (elbow) escaladas y con paneo
            for rel in &state.relations {
                let from_card = state.cards.iter().find(|c| c.name == rel.from_table);
                let to_card = state.cards.iter().find(|c| c.name == rel.to_table);
                if let (Some(fc), Some(tc)) = (from_card, to_card) {
                    let from_idx = fc
                        .visible_columns(state.only_keys)
                        .position(|c| c.name == rel.from_col)
                        .unwrap_or(0);
                    let to_idx = tc
                        .visible_columns(state.only_keys)
                        .position(|c| c.name == rel.to_col)
                        .unwrap_or(0);

                    let (fx_i, fy_i, fw, _) = state.card_screen_rect(fc, cx, cy);
                    let (tx_i, ty_i, tw, _) = state.card_screen_rect(tc, cx, cy);
                    let y1 = (fy_i + (hdr_h + from_idx * row_h + row_h / 2) as isize)
                        .clamp(cy as isize, (cy + ch) as isize)
                        as usize;
                    let y2 = (ty_i + (hdr_h + to_idx * row_h + row_h / 2) as isize)
                        .clamp(cy as isize, (cy + ch) as isize)
                        as usize;

                    let (x1_i, x2_i) = if fc.x < tc.x {
                        (fx_i + fw as isize, tx_i)
                    } else {
                        (fx_i, tx_i + tw as isize)
                    };
                    let x1 = x1_i.clamp(cx as isize, (cx + cw) as isize) as usize;
                    let x2 = x2_i.clamp(cx as isize, (cx + cw) as isize) as usize;

                    let is_highlighted =
                        fc.name == state.selected_table || tc.name == state.selected_table;
                    let line_c = if is_highlighted {
                        theme.accent_color()
                    } else {
                        Color::from_rgba(
                            theme.text_secondary().r,
                            theme.text_secondary().g,
                            theme.text_secondary().b,
                            80,
                        )
                    };
                    let thick = if is_highlighted { 2 } else { 1 };

                    // Segmentos en codo
                    let mx = (x1 + x2) / 2;
                    let h1_x = x1.min(mx);
                    let h1_w = x1.abs_diff(mx);
                    canvas.fill_rect(
                        area_clip,
                        h1_x,
                        y1.saturating_sub(thick / 2),
                        h1_w,
                        thick,
                        line_c,
                    );

                    let v_y = y1.min(y2);
                    let v_h = y1.abs_diff(y2);
                    canvas.fill_rect(
                        area_clip,
                        mx.saturating_sub(thick / 2),
                        v_y,
                        thick,
                        v_h,
                        line_c,
                    );

                    let h2_x = mx.min(x2);
                    let h2_w = mx.abs_diff(x2);
                    canvas.fill_rect(
                        area_clip,
                        h2_x,
                        y2.saturating_sub(thick / 2),
                        h2_w,
                        thick,
                        line_c,
                    );

                    // Marcas de cardinalidad (N en dependiente / foránea, 1 en referenciada / primaria)
                    let mark_y1 = self.centered_baseline(y1);
                    let mark_y2 = self.centered_baseline(y2);
                    let n_x = if fc.x < tc.x {
                        x1 + 6
                    } else {
                        x1.saturating_sub(16)
                    };
                    let one_x = if fc.x < tc.x {
                        x2.saturating_sub(14)
                    } else {
                        x2 + 6
                    };

                    if n_x >= cx && n_x + 16 <= cx + cw {
                        self.draw_mono_text(
                            canvas,
                            "N",
                            n_x,
                            mark_y1,
                            n_x + 16,
                            line_c,
                            tokens::MICRO,
                            true,
                        );
                    }
                    if one_x >= cx && one_x + 16 <= cx + cw {
                        self.draw_mono_text(
                            canvas,
                            "1",
                            one_x,
                            mark_y2,
                            one_x + 16,
                            line_c,
                            tokens::MICRO,
                            true,
                        );
                    }
                }
            }

            // Dibujar tarjetas de tablas escaladas y con paneo
            for card in &state.cards {
                let (c_xi, c_yi, card_w, c_h) = state.card_screen_rect(card, cx, cy);
                if c_xi + card_w as isize <= cx as isize
                    || c_yi + c_h as isize <= cy as isize
                    || c_xi >= (cx + cw) as isize
                    || c_yi >= (cy + ch) as isize
                {
                    continue;
                }
                let c_x = c_xi.max(cx as isize) as usize;
                let c_y = c_yi.max(cy as isize) as usize;
                if c_xi < cx as isize || c_yi < cy as isize {
                    continue;
                }
                let is_sel = card.name == state.selected_table;

                // Tarjeta
                canvas.fill_rect(area_clip, c_x, c_y, card_w, c_h, theme.mantle());
                let border_c = if is_sel {
                    theme.accent_color()
                } else {
                    theme.border_color()
                };
                canvas.rounded_stroke(c_x, c_y, card_w, c_h, tokens::RADIUS_CONTROL, border_c);
                if is_sel {
                    canvas.rounded_stroke(
                        c_x + 1,
                        c_y + 1,
                        card_w.saturating_sub(2),
                        c_h.saturating_sub(2),
                        tokens::RADIUS_CONTROL,
                        border_c,
                    );
                }

                // Cabecera de la tabla
                let head_bg = if is_sel {
                    Color::from_rgba(
                        theme.accent_color().r,
                        theme.accent_color().g,
                        theme.accent_color().b,
                        35,
                    )
                } else {
                    theme.background
                };
                canvas.fill_rect(area_clip, c_x, c_y, card_w, hdr_h, head_bg);
                canvas.fill_rect(
                    area_clip,
                    c_x,
                    c_y + hdr_h.saturating_sub(1),
                    card_w,
                    1,
                    theme.chrome_line(),
                );

                let h_dot = (c_x + 10, c_y + hdr_h / 2 - 4, 8, 8);
                canvas.fill_rect(
                    area_clip,
                    h_dot.0,
                    h_dot.1,
                    h_dot.2,
                    h_dot.3,
                    theme.accent_color(),
                );

                let h_base = self.centered_baseline(c_y + hdr_h / 2);
                self.draw_mono_text(
                    canvas,
                    &card.name,
                    c_x + 24,
                    h_base,
                    c_x + card_w.saturating_sub(60),
                    theme.foreground,
                    tokens::BODY,
                    true,
                );
                self.draw_mono_text(
                    canvas,
                    &card.row_count_label,
                    c_x + card_w.saturating_sub(55),
                    h_base,
                    c_x + card_w.saturating_sub(8),
                    theme.text_secondary(),
                    tokens::MICRO,
                    false,
                );

                // Filas de columnas
                let mut col_y = c_y + hdr_h;
                for col in card.visible_columns(state.only_keys) {
                    if col_y + row_h > cy + ch {
                        break;
                    }
                    let c_base = self.centered_baseline(col_y + row_h / 2);
                    if col.is_pk {
                        self.draw_mono_text(
                            canvas,
                            "PK",
                            c_x + 8,
                            c_base,
                            c_x + 26,
                            theme.yellow(),
                            tokens::MICRO,
                            true,
                        );
                    } else if col.is_fk {
                        self.draw_mono_text(
                            canvas,
                            "FK",
                            c_x + 8,
                            c_base,
                            c_x + 26,
                            theme.accent_color(),
                            tokens::MICRO,
                            true,
                        );
                    }
                    self.draw_ui_text(
                        canvas,
                        &col.name,
                        c_x + 28,
                        c_base,
                        c_x + card_w.saturating_sub(70),
                        theme.foreground,
                        tokens::DETAIL,
                        false,
                    );
                    self.draw_mono_text(
                        canvas,
                        &col.data_type,
                        c_x + card_w.saturating_sub(65),
                        c_base,
                        c_x + card_w.saturating_sub(6),
                        theme.text_secondary(),
                        tokens::MICRO,
                        false,
                    );
                    canvas.fill_rect(
                        area_clip,
                        c_x + 6,
                        col_y + row_h.saturating_sub(1),
                        card_w.saturating_sub(12),
                        1,
                        Color::from_rgba(
                            theme.chrome_line().r,
                            theme.chrome_line().g,
                            theme.chrome_line().b,
                            40,
                        ),
                    );
                    col_y += row_h;
                }
            }

            // Leyenda flotante en la esquina inferior izquierda del lienzo
            let leg_w = 340;
            let leg_h = 30;
            let leg_x = cx + 16;
            let leg_y = cy + ch.saturating_sub(leg_h + 16);
            Self::draw_rect(canvas, leg_x, leg_y, leg_w, leg_h, theme.mantle());
            canvas.rounded_stroke(
                leg_x,
                leg_y,
                leg_w,
                leg_h,
                tokens::RADIUS_CONTROL,
                theme.border_color(),
            );

            let l_base = self.centered_baseline(leg_y + leg_h / 2);
            self.draw_mono_text(
                canvas,
                "PK",
                leg_x + 12,
                l_base,
                leg_x + 32,
                theme.yellow(),
                tokens::MICRO,
                true,
            );
            self.draw_ui_text(
                canvas,
                "primaria",
                leg_x + 34,
                l_base,
                leg_x + 90,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            self.draw_mono_text(
                canvas,
                "FK",
                leg_x + 104,
                l_base,
                leg_x + 124,
                theme.accent_color(),
                tokens::MICRO,
                true,
            );
            self.draw_ui_text(
                canvas,
                "foránea",
                leg_x + 126,
                l_base,
                leg_x + 180,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            self.draw_mono_text(
                canvas,
                "1 — N",
                leg_x + 194,
                l_base,
                leg_x + 234,
                theme.accent_color(),
                tokens::MICRO,
                true,
            );
            self.draw_ui_text(
                canvas,
                "relación elegida",
                leg_x + 238,
                l_base,
                leg_x + leg_w - 6,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );
        }

        // 4. Panel derecho: Detalle de la tabla seleccionada
        if let Some((dx, dy, dw, dh)) = rects.schema_detail_panel {
            Self::draw_rect(canvas, dx, dy, dw, dh, theme.mantle());
            Self::draw_rect(canvas, dx, dy, 1, dh, theme.chrome_line());

            let head_base = self.centered_baseline(dy + 20);
            let full_tbl = format!("{}.{}", state.schema_name, state.selected_table);
            self.draw_mono_text(
                canvas,
                &full_tbl,
                dx + 16,
                head_base,
                dx + dw - 95,
                theme.foreground,
                tokens::BODY,
                true,
            );

            let row_count = state
                .tables_list
                .iter()
                .find(|(tbl, _)| tbl == &state.selected_table)
                .map(|(_, count)| count.as_str())
                .unwrap_or("0");
            let row_count_str = format!("{} filas", row_count);
            self.draw_mono_text(
                canvas,
                &row_count_str,
                dx + dw - 90,
                head_base,
                dx + dw - 10,
                theme.text_secondary(),
                tokens::MICRO,
                false,
            );

            let mut cur_y = dy + 44;

            // Claves foráneas
            let fk_base = self.centered_baseline(cur_y + 10);
            self.draw_ui_text_tracked(
                canvas,
                "CLAVES FORÁNEAS",
                dx + 16,
                fk_base,
                dx + dw - 16,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );
            cur_y += 20;

            let fks = state.foreign_keys_for_table(&state.selected_table);
            if fks.is_empty() {
                let none_base = self.centered_baseline(cur_y + 10);
                self.draw_ui_text(
                    canvas,
                    "Ninguna clave foránea saliente",
                    dx + 16,
                    none_base,
                    dx + dw - 16,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
                cur_y += 24;
            } else {
                for (from_col, to_tbl, to_col) in fks {
                    let fk_box = (dx + 16, cur_y, dw.saturating_sub(32), 40);
                    Self::draw_rect(
                        canvas,
                        fk_box.0,
                        fk_box.1,
                        fk_box.2,
                        fk_box.3,
                        theme.background,
                    );
                    canvas.rounded_stroke(
                        fk_box.0,
                        fk_box.1,
                        fk_box.2,
                        fk_box.3,
                        tokens::RADIUS_CONTROL,
                        theme.border_color(),
                    );
                    let f1 = self.centered_baseline(fk_box.1 + 12);
                    let rel_lbl = format!("{} → {}({})", from_col, to_tbl, to_col);
                    self.draw_mono_text(
                        canvas,
                        &rel_lbl,
                        fk_box.0 + 10,
                        f1,
                        fk_box.0 + fk_box.2 - 6,
                        theme.foreground,
                        tokens::DETAIL,
                        false,
                    );
                    let f2 = self.centered_baseline(fk_box.1 + 27);
                    self.draw_mono_text(
                        canvas,
                        "ON DELETE RESTRICT",
                        fk_box.0 + 10,
                        f2,
                        fk_box.0 + fk_box.2 - 6,
                        theme.text_secondary(),
                        tokens::MICRO,
                        false,
                    );
                    cur_y += 48;
                }
            }

            // Referenciada por
            let ref_base = self.centered_baseline(cur_y + 10);
            self.draw_ui_text_tracked(
                canvas,
                "REFERENCIADA POR",
                dx + 16,
                ref_base,
                dx + dw - 16,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );
            cur_y += 20;

            let refs = state.referenced_by_for_table(&state.selected_table);
            if refs.is_empty() {
                let none_base = self.centered_baseline(cur_y + 10);
                self.draw_ui_text(
                    canvas,
                    "Ninguna tabla la referencia",
                    dx + 16,
                    none_base,
                    dx + dw - 16,
                    theme.text_secondary(),
                    tokens::DETAIL,
                    false,
                );
                cur_y += 24;
            } else {
                for (from_tbl, from_col) in refs {
                    let r_base = self.centered_baseline(cur_y + 10);
                    let ref_txt = format!("{}.{}", from_tbl, from_col);
                    self.draw_mono_text(
                        canvas,
                        &ref_txt,
                        dx + 16,
                        r_base,
                        dx + dw - 16,
                        theme.foreground,
                        tokens::DETAIL,
                        false,
                    );
                    cur_y += 22;
                }
            }

            // Índices
            cur_y += 6;
            let idx_base = self.centered_baseline(cur_y + 10);
            self.draw_ui_text_tracked(
                canvas,
                "ÍNDICES",
                dx + 16,
                idx_base,
                dx + dw - 16,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );
            cur_y += 20;
            let pk_idx = format!("{}_pkey", state.selected_table);
            let i1 = self.centered_baseline(cur_y + 10);
            self.draw_mono_text(
                canvas,
                &pk_idx,
                dx + 16,
                i1,
                dx + 140,
                theme.foreground,
                tokens::DETAIL,
                false,
            );
            self.draw_mono_text(
                canvas,
                "id · único",
                dx + 146,
                i1,
                dx + dw - 10,
                theme.text_secondary(),
                tokens::MICRO,
                false,
            );
            cur_y += 22;

            // Restricciones
            cur_y += 6;
            let check_base = self.centered_baseline(cur_y + 10);
            self.draw_ui_text_tracked(
                canvas,
                "RESTRICCIONES",
                dx + 16,
                check_base,
                dx + dw - 16,
                theme.text_secondary(),
                tokens::MICRO,
                true,
                1.2,
            );
            cur_y += 20;
            let ch1 = self.centered_baseline(cur_y + 10);
            self.draw_mono_text(
                canvas,
                "CHECK (id > 0)",
                dx + 16,
                ch1,
                dx + dw - 16,
                theme.text_secondary(),
                tokens::DETAIL,
                false,
            );

            // Botones inferiores: Ver DDL y Abrir datos
            if let Some((b1_x, b1_y, b1_w, b1_h)) = rects.schema_detail_ddl_btn {
                Self::draw_rect(canvas, b1_x, b1_y, b1_w, b1_h, theme.background);
                canvas.rounded_stroke(
                    b1_x,
                    b1_y,
                    b1_w,
                    b1_h,
                    tokens::RADIUS_CONTROL,
                    theme.border_color(),
                );
                let b1_base = self.centered_baseline(b1_y + b1_h / 2);
                self.draw_ui_text(
                    canvas,
                    "Ver DDL",
                    b1_x + 10,
                    b1_base,
                    b1_x + b1_w - 10,
                    theme.foreground,
                    tokens::DETAIL,
                    false,
                );
            }

            if let Some((b2_x, b2_y, b2_w, b2_h)) = rects.schema_detail_open_browse_btn {
                Self::draw_rect(canvas, b2_x, b2_y, b2_w, b2_h, theme.accent_color());
                let b2_base = self.centered_baseline(b2_y + b2_h / 2);
                let open_txt = format!("Abrir datos de {}", state.selected_table);
                self.draw_ui_text(
                    canvas,
                    &open_txt,
                    b2_x + 10,
                    b2_base,
                    b2_x + b2_w - 10,
                    theme.background,
                    tokens::DETAIL,
                    true,
                );
            }
        }
    }

    /// Tokeniza una línea de SQL en elementos léxicos para resaltado de sintaxis.
    fn tokenize_sql(line: &str) -> Vec<(&str, SqlTokenKind)> {
        let mut tokens = Vec::new();
        let trimmed_lead = line.trim_start();
        if trimmed_lead.starts_with("--") {
            tokens.push((line, SqlTokenKind::Comment));
            return tokens;
        }

        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            // Comentario de línea
            if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
                tokens.push((&line[i..], SqlTokenKind::Comment));
                break;
            }

            // String literal '...'
            if bytes[i] == b'\'' {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    i += 1;
                }
                tokens.push((&line[start..i], SqlTokenKind::StringLit));
                continue;
            }

            // Número
            if bytes[i].is_ascii_digit() {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_')
                {
                    i += 1;
                }
                tokens.push((&line[start..i], SqlTokenKind::NumberLit));
                continue;
            }

            // Identificador o Keyword
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                let is_kw = matches!(
                    word.to_ascii_uppercase().as_str(),
                    "SELECT"
                        | "FROM"
                        | "WHERE"
                        | "JOIN"
                        | "LEFT"
                        | "RIGHT"
                        | "INNER"
                        | "OUTER"
                        | "CROSS"
                        | "ON"
                        | "GROUP"
                        | "BY"
                        | "ORDER"
                        | "ASC"
                        | "DESC"
                        | "AND"
                        | "OR"
                        | "NOT"
                        | "IN"
                        | "IS"
                        | "NULL"
                        | "INSERT"
                        | "INTO"
                        | "VALUES"
                        | "UPDATE"
                        | "SET"
                        | "DELETE"
                        | "LIMIT"
                        | "OFFSET"
                        | "HAVING"
                        | "UNION"
                        | "ALL"
                        | "AS"
                        | "DISTINCT"
                        | "COUNT"
                        | "SUM"
                        | "AVG"
                        | "MIN"
                        | "MAX"
                        | "ROUND"
                        | "EXPLAIN"
                        | "ANALYZE"
                        | "BEGIN"
                        | "COMMIT"
                        | "ROLLBACK"
                        | "CREATE"
                        | "TABLE"
                        | "DROP"
                        | "ALTER"
                        | "WITH"
                        | "CASE"
                        | "WHEN"
                        | "THEN"
                        | "ELSE"
                        | "END"
                        | "TRUE"
                        | "FALSE"
                );
                tokens.push((
                    word,
                    if is_kw {
                        SqlTokenKind::Keyword
                    } else {
                        SqlTokenKind::Other
                    },
                ));
                continue;
            }

            // Símbolos y espacios
            let start = i;
            while i < bytes.len()
                && !bytes[i].is_ascii_alphanumeric()
                && bytes[i] != b'_'
                && bytes[i] != b'\''
                && !(i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-')
            {
                i += 1;
            }
            tokens.push((&line[start..i], SqlTokenKind::Other));
        }
        tokens
    }
}

/// Clasificación léxica de un token SQL para renderizado con colores del tema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlTokenKind {
    Keyword,
    StringLit,
    NumberLit,
    Comment,
    Other,
}

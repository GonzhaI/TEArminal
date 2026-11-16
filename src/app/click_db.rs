//! Despacho de clics y eventos de ratón para el módulo de bases de datos (F6).
//!
//! Controla la selección de conexiones, sub-pestañas superiores, filtros, edición de celdas,
//! descarte o aplicación de cambios pendientes y ejecución de consultas SQL.

use super::*;
use crate::app::db_tab::{DbConnFormTab, DbQueryResultsTab};
use crate::app::dialog::{DialogSpec, ToastSeverity};
use crate::db::store::{Connection, Env};
use crate::renderer::{rect_contains, DbSidebarRects, DbViewMode, DbViewRects};
use winit::event::{ElementState, MouseButton};

impl TearminalApp {
    /// Procesa clics dentro del panel lateral contextual de Datos (`AppModule::Data`).
    pub(super) fn click_db_sidebar_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_h: usize,
    ) -> bool {
        if state != ElementState::Pressed || button != MouseButton::Left {
            return false;
        }
        if self.sidebar_collapsed || self.active_module != crate::config::AppModule::Data {
            return false;
        }
        let sb_rects = crate::renderer::ContextualSidebarRects::compute(
            self.sidebar_collapsed,
            self.active_module,
            win_h,
            self.status_bar_height(),
        );
        let side_geom = DbSidebarRects::compute(sb_rects.content);
        if !rect_contains(side_geom.container, mx, my) {
            return false;
        }

        // 1. Selector de conexión activa: abre la vista de Conexiones
        if rect_contains(side_geom.conn_selector, mx, my) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = tab.db_tab {
                    db.active_view = DbViewMode::Connections;
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // 2. Conmutador Esquema / Consultas
        if rect_contains(side_geom.schema_tab_btn, mx, my) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = tab.db_tab {
                    db.sidebar_subtab_queries = false;
                    db.sidebar_search_focused = false;
                    db.sidebar_scroll = 0;
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }
        if rect_contains(side_geom.query_tab_btn, mx, my) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = tab.db_tab {
                    db.sidebar_subtab_queries = true;
                    db.sidebar_search_focused = false;
                    db.sidebar_scroll = 0;
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // 2b. Campo de búsqueda lateral
        if rect_contains(side_geom.search_input, mx, my) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = tab.db_tab {
                    db.sidebar_search_focused = true;
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // 3. Botón inferior: Nueva conexión o Nueva consulta
        if rect_contains(side_geom.new_conn_btn, mx, my) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = tab.db_tab {
                    db.sidebar_search_focused = false;
                    if db.sidebar_subtab_queries {
                        db.query.editor_input =
                            crate::ui::TextInput::new("SELECT * FROM \nWHERE \nLIMIT 50;");
                        db.active_view = DbViewMode::Query;
                    } else {
                        db.connections.reset_to_new_connection();
                        db.active_view = DbViewMode::Connections;
                    }
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        // 4. Lista de tablas o consultas (filtradas y con scroll)
        if rect_contains(side_geom.items_area, mx, my) {
            let rel_y = my.saturating_sub(side_geom.items_area.1);
            let is_queries = self
                .tabs
                .get(self.active_tab_index)
                .and_then(|t| t.db_tab.as_ref())
                .is_some_and(|db| db.sidebar_subtab_queries);

            if !is_queries {
                let row_idx = rel_y / 30;
                let mut table_to_load = None;
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = tab.db_tab {
                        db.sidebar_search_focused = false;
                        let filtered = db.filtered_tables();
                        if let Some((tbl_name, _)) = filtered.get(db.sidebar_scroll + row_idx) {
                            db.browse.table_name = tbl_name.clone();
                            db.browse.current_page = 0;
                            db.browse.scroll_y = 0;
                            db.active_view = DbViewMode::Browse;
                            table_to_load = Some(tbl_name.clone());
                        }
                    }
                }
                if table_to_load.is_some() {
                    self.reload_db_browse_table();
                }
            } else {
                let mut should_exec = false;
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = tab.db_tab {
                        db.sidebar_search_focused = false;
                        let header_h = 24;
                        let row_h = 38;
                        let saved = db.filtered_saved_queries();
                        let history = db.filtered_history();
                        let saved_count = saved.len();
                        let saved_total_h = if saved_count > 0 {
                            header_h + saved_count * row_h
                        } else {
                            0
                        };

                        if rel_y < saved_total_h && rel_y >= header_h {
                            let idx = (rel_y - header_h) / row_h;
                            if let Some(item) = saved.get(idx) {
                                db.query.editor_input = crate::ui::TextInput::new(&item.sql);
                                db.active_view = DbViewMode::Query;
                                should_exec = true;
                            }
                        } else if rel_y >= saved_total_h + header_h {
                            let hist_rel_y = rel_y - saved_total_h - header_h;
                            let idx = db.sidebar_scroll + hist_rel_y / row_h;
                            if let Some(item) = history.get(idx) {
                                db.query.editor_input = crate::ui::TextInput::new(&item.sql);
                                db.active_view = DbViewMode::Query;
                                should_exec = true;
                            }
                        }
                    }
                }
                if should_exec {
                    self.execute_db_query_from_editor();
                }
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }

        false
    }

    /// Procesa clics dentro del área principal de trabajo de la pestaña de datos.
    pub(super) fn click_db_view_area(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mx: usize,
        my: usize,
        win_w: usize,
        win_h: usize,
    ) -> bool {
        if state != ElementState::Pressed || button != MouseButton::Left {
            return false;
        }
        let (offset, right_offset, top_offset, status_bar_h) = self.terminal_area_offsets();
        let avail_w = win_w.saturating_sub(offset + right_offset);
        let avail_h = win_h.saturating_sub(top_offset + status_bar_h);
        let container = (offset, top_offset, avail_w, avail_h);

        enum DbViewAction {
            None,
            Redraw,
            Connect(Connection),
            SpawnTestConnection(Connection),
            ReloadBrowseTable,
            ExecuteQuery,
            OpenDialog(DialogSpec),
            ShowToast(ToastSeverity, String),
            CopyText(String, &'static str),
            SpawnApplyChanges(Connection, String),
            OpenDbSettings,
        }

        let prod_read_only_policy = self.config.database.prod_read_only;
        let mut action = DbViewAction::None;

        {
            let active_tab = match self.tabs.get_mut(self.active_tab_index) {
                Some(t) if t.is_database => t,
                _ => return false,
            };
            let db = match active_tab.db_tab.as_mut() {
                Some(db) => db,
                None => return false,
            };
            db.sidebar_search_focused = false;

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

            // 1. Clic en las sub-pestañas de navegación superior
            if let Some(nav_idx) = rects.hit_nav_tab(mx, my) {
                match nav_idx {
                    0 => db.active_view = DbViewMode::Connections,
                    1 => db.active_view = DbViewMode::Browse,
                    2 => db.active_view = DbViewMode::Query,
                    3 => db.active_view = DbViewMode::Schema,
                    _ => {}
                }
                action = DbViewAction::Redraw;
            } else if let Some(b_rect) = rects.prod_banner {
                // 2. Banner de producción: botón «Volver a solo lectura»
                if rect_contains(b_rect, mx, my) {
                    let btn_w = 140;
                    let btn_h = 24;
                    let btn_x = b_rect.0 + b_rect.2.saturating_sub(btn_w + 14);
                    let btn_y = b_rect.1 + (b_rect.3 - btn_h) / 2;
                    if rect_contains((btn_x, btn_y, btn_w, btn_h), mx, my) {
                        db.toggle_prod_write();
                        action = DbViewAction::ShowToast(
                            ToastSeverity::Info,
                            "Modo de solo lectura activado".into(),
                        );
                    }
                }
            }

            if matches!(action, DbViewAction::None) {
                // 3. Manejo por vista
                match db.active_view {
                    DbViewMode::Connections => {
                        // 1. Clic en la lista de conexiones (teniendo en cuenta el scroll)
                        let scroll_px = db.connections.list_scroll * 38;
                        for (cid, item_r) in &rects.conn_items {
                            if rect_contains(*item_r, mx, my + scroll_px) {
                                let conn_opt = db.connections.store.find(cid).cloned();
                                if let Some(conn) = conn_opt {
                                    let id = conn.id.clone();
                                    db.connections.select_connection(&id);
                                    db.active_connection_id = Some(id);
                                    db.active_connection_name = conn.name.clone();
                                    db.active_connection_engine = conn.engine;
                                    db.active_connection_env = conn.env;
                                    db.active_connection_db = conn.database.clone();
                                    action = DbViewAction::Redraw;
                                }
                                break;
                            }
                        }

                        // 2. Pestañas del formulario
                        for (i, &t_rect) in rects.conn_form_tabs.iter().enumerate() {
                            if rect_contains(t_rect, mx, my) {
                                db.connections.form_tab = match i {
                                    0 => DbConnFormTab::General,
                                    1 => DbConnFormTab::Tls,
                                    2 => DbConnFormTab::Tunnel,
                                    3 => DbConnFormTab::Advanced,
                                    _ => DbConnFormTab::General,
                                };
                                action = DbViewAction::Redraw;
                                break;
                            }
                        }

                        // 3. Botón Conectar (acción directa que conecta, carga esquema y cambia a Browse)
                        if let Some(c_btn) = rects.conn_connect_btn {
                            if rect_contains(c_btn, mx, my) {
                                let conn = db.connections.build_connection_from_form();
                                if conn.name.is_empty() {
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Warning,
                                        "El nombre de la conexión no puede estar vacío".into(),
                                    );
                                } else {
                                    action = DbViewAction::Connect(conn);
                                }
                            }
                        }

                        // 4. Botón Guardar
                        if let Some(s_btn) = rects.conn_save_btn {
                            if rect_contains(s_btn, mx, my) {
                                let conn = db.connections.build_connection_from_form();
                                if conn.name.is_empty() {
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Warning,
                                        "El nombre de la conexión no puede estar vacío".into(),
                                    );
                                } else {
                                    let _ = db.connections.store.upsert(conn.clone());
                                    let _ =
                                        db.connections.store.save_to(
                                            &crate::db::store::ConnectionStore::default_path(),
                                        );
                                    if !db.connections.password_input.text.is_empty() {
                                        let secret = crate::db::credentials::Secret::new(
                                            &db.connections.password_input.text,
                                        );
                                        let _ = crate::db::credentials::store(
                                            crate::db::credentials::SecretKind::DbPassword,
                                            &conn.id,
                                            &secret,
                                        );
                                    }
                                    db.connections.selected_id = Some(conn.id.clone());
                                    db.active_connection_id = Some(conn.id.clone());
                                    db.active_connection_name = conn.name.clone();
                                    db.active_connection_engine = conn.engine;
                                    db.active_connection_env = conn.env;
                                    db.active_connection_db = conn.database.clone();
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Info,
                                        format!("Conexión «{}» guardada", conn.name),
                                    );
                                }
                            }
                        }

                        // 5b. Botón Eliminar conexión
                        if let Some(d_btn) = rects.conn_delete_btn {
                            if rect_contains(d_btn, mx, my) {
                                if let Some(sel_id) = db.connections.selected_id.clone() {
                                    let sel_name = db
                                        .connections
                                        .store
                                        .find(&sel_id)
                                        .map(|c| c.name.clone())
                                        .unwrap_or_else(|| sel_id.clone());
                                    action = DbViewAction::OpenDialog(
                                        DialogSpec::delete_connection(&sel_id, &sel_name),
                                    );
                                } else {
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Info,
                                        "Selecciona una conexión guardada para eliminar".into(),
                                    );
                                }
                            }
                        }

                        // 5. Botón Probar conexión
                        if let Some(t_btn) = rects.conn_test_btn {
                            if rect_contains(t_btn, mx, my) {
                                let conn = db.connections.build_connection_from_form();
                                db.connections.test_steps = vec![crate::app::db_tab::DbTestStep {
                                    mark: "·".into(),
                                    text: "Iniciando prueba de conexión...".into(),
                                    elapsed_ms: 0,
                                    ok: true,
                                }];
                                action = DbViewAction::SpawnTestConnection(conn);
                            }
                        }

                        // 5c. Botón «Corregir permisos» en aviso de clave .pem insegura
                        if let Some((_, ref pem_path)) = db.connections.test_warning {
                            if let Some((tx, ty, tw, th)) = rects.conn_test_panel {
                                let warn_y = ty + th.saturating_sub(38);
                                let fix_btn = (tx + tw.saturating_sub(130), warn_y + 7, 120, 24);
                                if rect_contains(fix_btn, mx, my) {
                                    let key_name = pem_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "clave.pem".into());
                                    action = DbViewAction::OpenDialog(DialogSpec::pem_permissions(
                                        &key_name,
                                        pem_path.clone(),
                                    ));
                                }
                            }
                        }

                        // 6. Controles y campos del formulario
                        if let Some(ref fields) = rects.conn_fields {
                            if rect_contains(fields.name_input, mx, my) {
                                db.connections.active_field =
                                    Some(crate::app::db_tab::DbConnField::Name);
                                action = DbViewAction::Redraw;
                            } else if let Some(field) =
                                fields.field_at(db.connections.form_tab, mx, my)
                            {
                                db.connections.active_field = Some(field);
                                action = DbViewAction::Redraw;
                            } else {
                                match db.connections.form_tab {
                                    DbConnFormTab::General => {
                                        for &(eng, btn_r) in &fields.engine_btns {
                                            if rect_contains(btn_r, mx, my) {
                                                db.connections.engine = eng;
                                                action = DbViewAction::Redraw;
                                                break;
                                            }
                                        }
                                        for &(env, btn_r) in &fields.env_btns {
                                            if rect_contains(btn_r, mx, my) {
                                                db.connections.env = env;
                                                action = DbViewAction::Redraw;
                                                break;
                                            }
                                        }
                                        if rect_contains(fields.read_only_toggle, mx, my) {
                                            db.connections.read_only = !db.connections.read_only;
                                            action = DbViewAction::Redraw;
                                        }
                                    }
                                    DbConnFormTab::Tls => {
                                        for &(mode, btn_r) in &fields.ssl_mode_btns {
                                            if rect_contains(btn_r, mx, my) {
                                                db.connections.ssl_mode = mode;
                                                action = DbViewAction::Redraw;
                                                break;
                                            }
                                        }
                                    }
                                    DbConnFormTab::Tunnel => {
                                        if rect_contains(fields.tunnel_toggle, mx, my) {
                                            db.connections.tunnel_enabled =
                                                !db.connections.tunnel_enabled;
                                            action = DbViewAction::Redraw;
                                        }
                                    }
                                    DbConnFormTab::Advanced => {
                                        if let Some(dup_btn) = rects.conn_adv_duplicate_btn {
                                            if rect_contains(dup_btn, mx, my) {
                                                let cur_name = db
                                                    .connections
                                                    .name_input
                                                    .text
                                                    .trim()
                                                    .to_string();
                                                let base = if cur_name.is_empty() {
                                                    "Conexión".to_string()
                                                } else {
                                                    cur_name
                                                };
                                                db.connections.selected_id = None;
                                                db.connections.name_input =
                                                    crate::ui::TextInput::new(format!(
                                                        "{base} (copia)"
                                                    ));
                                                action = DbViewAction::ShowToast(
                                                    ToastSeverity::Info,
                                                    "Formulario duplicado como nueva conexión"
                                                        .into(),
                                                );
                                            }
                                        }
                                        if let Some(set_btn) = rects.conn_adv_settings_btn {
                                            if rect_contains(set_btn, mx, my) {
                                                action = DbViewAction::OpenDbSettings;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    DbViewMode::Browse => {
                        // Clic en filtro WHERE
                        if let Some(w_input) = rects.where_input {
                            if rect_contains(w_input, mx, my) {
                                db.browse.active_filter =
                                    Some(crate::app::db_tab::BrowseFilterField::Where);
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Clic en filtro ORDER BY
                        if let Some(o_input) = rects.order_input {
                            if rect_contains(o_input, mx, my) {
                                db.browse.active_filter =
                                    Some(crate::app::db_tab::BrowseFilterField::OrderBy);
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Botón Anterior página
                        if let Some(prev) = rects.page_prev_btn {
                            if rect_contains(prev, mx, my) && db.browse.current_page > 0 {
                                db.browse.current_page -= 1;
                                action = DbViewAction::ReloadBrowseTable;
                            }
                        }

                        // Botón Siguiente página
                        if let Some(next) = rects.page_next_btn {
                            if rect_contains(next, mx, my)
                                && (db.browse.current_page + 1) * db.browse.page_size
                                    < db.browse.total_rows
                            {
                                db.browse.current_page += 1;
                                action = DbViewAction::ReloadBrowseTable;
                            }
                        }

                        // Clic en la cuadrícula de datos (selección y clic/doble clic para editar celda)
                        if let Some(grid) = rects.grid_rect {
                            if rect_contains(grid, mx, my) {
                                let rel_y = my.saturating_sub(grid.1 + 36);
                                let row_idx = db.browse.scroll_y + rel_y / 32;
                                if row_idx < db.browse.rows.len() {
                                    let col_w = (grid.2.saturating_sub(50))
                                        / db.browse.columns.len().max(1);
                                    let rel_x = mx.saturating_sub(grid.0 + 50);
                                    let col_idx = rel_x.checked_div(col_w).unwrap_or(0);

                                    if col_idx < db.browse.columns.len() {
                                        let is_same = db.browse.selected_row == Some(row_idx)
                                            && db.browse.selected_col == Some(col_idx);
                                        db.browse.selected_row = Some(row_idx);
                                        db.browse.selected_col = Some(col_idx);

                                        if is_same {
                                            if db.is_effective_read_only(prod_read_only_policy) {
                                                action = DbViewAction::ShowToast(
                                                    ToastSeverity::Warning,
                                                    "Edición bloqueada en modo solo lectura".into(),
                                                );
                                            } else {
                                                let current_val = db.browse.rows[row_idx]
                                                    .cells
                                                    .get(col_idx)
                                                    .cloned()
                                                    .unwrap_or_default();
                                                db.browse.editing_cell = Some((
                                                    row_idx,
                                                    col_idx,
                                                    crate::ui::TextInput::new(&current_val),
                                                ));
                                                action = DbViewAction::Redraw;
                                            }
                                        } else {
                                            action = DbViewAction::Redraw;
                                        }
                                    }
                                }
                            }
                        }

                        // Botón Descartar cambios
                        if let Some(d_btn) = rects.pending_discard_btn {
                            if rect_contains(d_btn, mx, my) {
                                db.browse.discard_pending();
                                action = DbViewAction::ShowToast(
                                    ToastSeverity::Info,
                                    "Cambios descartados".into(),
                                );
                            }
                        }

                        // Botón «Revisar y aplicar…»
                        if let Some(a_btn) = rects.pending_apply_btn {
                            if rect_contains(a_btn, mx, my) {
                                let sql = db.browse.generated_sql();
                                let count = db.browse.pending_changes.len();
                                let tbl = db.browse.table_name.clone();
                                let conn_name = db.active_connection_name.clone();

                                if db.active_connection_env == Env::Prod {
                                    let db_name = db.active_connection_db.clone();
                                    let impact: Vec<crate::app::DbImpactRow> = db
                                        .browse
                                        .pending_changes
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, ch)| crate::app::DbImpactRow {
                                            n: (idx + 1).to_string(),
                                            op: format!("UPDATE · {}", ch.col_name),
                                            table: tbl.clone(),
                                            rows: "1".into(),
                                        })
                                        .collect();
                                    action =
                                        DbViewAction::OpenDialog(DialogSpec::prod_write_confirm(
                                            &conn_name, &db_name, count, &tbl, &sql, impact,
                                        ));
                                } else {
                                    let conn = db
                                        .active_connection_id
                                        .as_ref()
                                        .and_then(|id| db.connections.store.find(id))
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            db.connections.build_connection_from_form()
                                        });
                                    action = DbViewAction::SpawnApplyChanges(conn, sql);
                                }
                            }
                        }
                    }

                    DbViewMode::Query => {
                        // Botón Ejecutar
                        if let Some(e_btn) = rects.query_exec_btn {
                            if rect_contains(e_btn, mx, my) {
                                action = DbViewAction::ExecuteQuery;
                            }
                        }
                        // Botón Explicar
                        if let Some(exp_btn) = rects.query_explain_btn {
                            if rect_contains(exp_btn, mx, my) {
                                let cur_sql = db.query.editor_input.text.trim();
                                if !cur_sql.to_uppercase().starts_with("EXPLAIN") {
                                    db.query.editor_input = crate::ui::TextInput::new(format!(
                                        "EXPLAIN ANALYZE\n{}",
                                        cur_sql
                                    ));
                                }
                                db.query.results_tab = DbQueryResultsTab::Plan;
                                action = DbViewAction::ExecuteQuery;
                            }
                        }
                        // Botón Formatear
                        if let Some(f_btn) = rects.query_format_btn {
                            if rect_contains(f_btn, mx, my) {
                                db.query.format_current_sql();
                                action = DbViewAction::Redraw;
                            }
                        }
                        // Toggle Auto-commit / Transacción
                        if let Some(t_btn) = rects.query_tx_toggle {
                            if rect_contains(t_btn, mx, my) {
                                db.query.auto_commit = !db.query.auto_commit;
                                action = DbViewAction::Redraw;
                            }
                        }
                        // Botón Guardar consulta
                        if let Some(s_btn) = rects.query_save_btn {
                            if rect_contains(s_btn, mx, my) {
                                if db.query.save_current_query() {
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Success,
                                        "Consulta guardada en la biblioteca lateral".into(),
                                    );
                                } else {
                                    action = DbViewAction::ShowToast(
                                        ToastSeverity::Info,
                                        "La consulta está vacía o ya estaba guardada".into(),
                                    );
                                }
                            }
                        }
                        // Subtabs de resultados
                        for (i, &r_rect) in rects.query_results_tabs.iter().enumerate() {
                            if rect_contains(r_rect, mx, my) {
                                db.query.results_tab = match i {
                                    0 => DbQueryResultsTab::Grid,
                                    1 => DbQueryResultsTab::Plan,
                                    2 => DbQueryResultsTab::Messages,
                                    _ => DbQueryResultsTab::Grid,
                                };
                                action = DbViewAction::Redraw;
                                break;
                            }
                        }
                        // Botones de exportación: CSV, JSON, Copiar
                        if let Some(csv_btn) = rects.query_export_csv_btn {
                            if rect_contains(csv_btn, mx, my) {
                                let csv = db.query.to_csv();
                                action = DbViewAction::CopyText(
                                    csv,
                                    "Resultados copiados en formato CSV",
                                );
                            }
                        }
                        if let Some(json_btn) = rects.query_export_json_btn {
                            if rect_contains(json_btn, mx, my) {
                                let json = db.query.to_json();
                                action = DbViewAction::CopyText(
                                    json,
                                    "Resultados copiados en formato JSON",
                                );
                            }
                        }
                        if let Some(cp_btn) = rects.query_copy_btn {
                            if rect_contains(cp_btn, mx, my) {
                                let tsv = db.query.to_tsv();
                                action = DbViewAction::CopyText(
                                    tsv,
                                    "Resultados copiados al portapapeles",
                                );
                            }
                        }
                        // Clic dentro del editor SQL: posicionar cursor
                        if let Some(ed_r) = rects.query_editor_rect {
                            if rect_contains(ed_r, mx, my) {
                                let gutter_w = 46;
                                let text_start_x = ed_r.0 + gutter_w + 14;
                                let line_h = 22;
                                let target_line = ((my.saturating_sub(ed_r.1 + 8)) / line_h) + 1;
                                let approx_char_w = 8;
                                let target_col =
                                    (mx.saturating_sub(text_start_x) / approx_char_w) + 1;
                                db.query
                                    .editor_input
                                    .set_cursor_from_line_col(target_line, target_col);
                                action = DbViewAction::Redraw;
                            }
                        }
                    }

                    DbViewMode::Schema => {
                        // Selección de tabla en el panel izquierdo del esquema
                        if let Some(t_list) = rects.schema_tables_list {
                            if rect_contains(t_list, mx, my) {
                                let rel_y = my.saturating_sub(t_list.1 + 36);
                                let row_idx = rel_y / 28;
                                if let Some((tbl, _)) = db.schema.tables_list.get(row_idx) {
                                    db.schema.selected_table = tbl.clone();
                                    action = DbViewAction::Redraw;
                                }
                            }
                        }

                        // Toolbar de esquema: Toggle Columnas / Claves
                        if let Some(k_toggle) = rects.schema_keys_toggle {
                            if rect_contains(k_toggle, mx, my) {
                                db.schema.only_keys = !db.schema.only_keys;
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Toolbar de esquema: Botón Ordenar
                        if let Some(arr_btn) = rects.schema_arrange_btn {
                            if rect_contains(arr_btn, mx, my) {
                                let cw =
                                    rects.schema_cards_area.map(|(_, _, w, _)| w).unwrap_or(800);
                                db.schema.arrange_cards(cw);
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Toolbar de esquema: Zoom − y +
                        if let Some(zm_btn) = rects.schema_zoom_minus_btn {
                            if rect_contains(zm_btn, mx, my) {
                                db.schema.zoom_pct = db.schema.zoom_pct.saturating_sub(25).max(50);
                                action = DbViewAction::Redraw;
                            }
                        }
                        if let Some(zp_btn) = rects.schema_zoom_plus_btn {
                            if rect_contains(zp_btn, mx, my) {
                                db.schema.zoom_pct = (db.schema.zoom_pct + 25).min(200);
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Toolbar de esquema: Exportar DDL completo
                        if let Some(exp_btn) = rects.schema_export_btn {
                            if rect_contains(exp_btn, mx, my) {
                                let ddl = db.schema.generate_all_ddl();
                                action = DbViewAction::CopyText(
                                    ddl,
                                    "DDL completo del esquema copiado al portapapeles",
                                );
                            }
                        }

                        // Clic en tarjetas dentro del lienzo (soporta zoom y paneo)
                        if let Some((cx, cy, _, _)) = rects.schema_cards_area {
                            if let Some(name) = db.schema.hit_test_card(cx, cy, mx, my) {
                                db.schema.selected_table = name;
                                action = DbViewAction::Redraw;
                            }
                        }

                        // Botón «Ver DDL» en detalle
                        if let Some(b1) = rects.schema_detail_ddl_btn {
                            if rect_contains(b1, mx, my) {
                                let ddl = db.schema.generate_table_ddl(&db.schema.selected_table);
                                action = DbViewAction::CopyText(
                                    ddl,
                                    "DDL de la tabla copiado al portapapeles",
                                );
                            }
                        }

                        // Botón «Abrir datos de <tabla>»
                        if let Some(b2) = rects.schema_detail_open_browse_btn {
                            if rect_contains(b2, mx, my) {
                                db.browse.table_name = db.schema.selected_table.clone();
                                db.browse.current_page = 0;
                                db.active_view = DbViewMode::Browse;
                                action = DbViewAction::ReloadBrowseTable;
                            }
                        }
                    }
                }
            }
        }

        match action {
            DbViewAction::None => false,
            DbViewAction::Redraw => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::Connect(conn) => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = tab.db_tab {
                        let _ = db.connections.store.upsert(conn.clone());
                        let _ = db
                            .connections
                            .store
                            .save_to(&crate::db::store::ConnectionStore::default_path());
                        if !db.connections.password_input.text.is_empty() {
                            let secret = crate::db::credentials::Secret::new(
                                &db.connections.password_input.text,
                            );
                            let _ = crate::db::credentials::store(
                                crate::db::credentials::SecretKind::DbPassword,
                                &conn.id,
                                &secret,
                            );
                        }
                        db.connections.selected_id = Some(conn.id.clone());
                        db.active_connection_id = Some(conn.id.clone());
                        db.active_connection_name = conn.name.clone();
                        db.active_connection_engine = conn.engine;
                        db.active_connection_env = conn.env;
                        db.active_connection_db = conn.database.clone();
                        db.is_connected = false;
                    }
                }
                let c_name = conn.name.clone();
                self.spawn_db_load_schema(conn);
                self.show_toast(ToastSeverity::Info, format!("Conectando a «{c_name}»..."));
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::SpawnTestConnection(conn) => {
                self.spawn_db_test_connection(conn);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::ReloadBrowseTable => {
                self.reload_db_browse_table();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::ExecuteQuery => {
                self.execute_db_query_from_editor();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::OpenDialog(spec) => {
                self.open_dialog(spec);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::ShowToast(sev, msg) => {
                self.show_toast(sev, msg);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::CopyText(content, toast_msg) => {
                self.set_clipboard_text(&content);
                self.show_toast(ToastSeverity::Success, toast_msg);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::SpawnApplyChanges(conn, sql) => {
                self.spawn_db_apply_changes(
                    conn,
                    sql,
                    crate::db::session::WriteConfirmation::default(),
                );
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
            DbViewAction::OpenDbSettings => {
                self.settings_page_state.section = crate::renderer::SettingsSection::Database;
                self.open_or_switch_settings_tab();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                true
            }
        }
    }
}

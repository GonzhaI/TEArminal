//! Geometría para el módulo de bases de datos: conexiones, cuadrícula de datos, editor SQL y esquema.
//!
//! Única fuente de verdad de rectángulos para dibujo y eventos de clic (DISENO §5, pantallas 05 a 08 y 10).

use super::{rect_contains, Rect};
use crate::app::db_tab::{DbConnField, DbConnFormTab};
use crate::db::store::{Connection, Engine, Env, SslMode};

/// Pestaña o modo activo dentro de la vista de base de datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DbViewMode {
    #[default]
    Connections,
    Browse,
    Query,
    Schema,
}

/// Geometría de los campos y controles interactivos del formulario de conexión.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DbConnFormRects {
    pub name_input: Rect,
    // General
    pub engine_btns: Vec<(Engine, Rect)>,
    pub env_btns: Vec<(Env, Rect)>,
    pub host_input: Rect,
    pub port_input: Rect,
    pub database_input: Rect,
    pub user_input: Rect,
    pub password_input: Rect,
    pub read_only_toggle: Rect,
    // SSL / TLS
    pub ssl_mode_btns: Vec<(SslMode, Rect)>,
    pub tls_server_name: Rect,
    pub tls_ca_path: Rect,
    pub tls_cert_path: Rect,
    pub tls_key_path: Rect,
    // Túnel SSH
    pub tunnel_toggle: Rect,
    pub tunnel_host: Rect,
    pub tunnel_port: Rect,
    pub tunnel_user: Rect,
    pub tunnel_key_path: Rect,
    pub tunnel_fingerprint: Rect,
}

impl DbConnFormRects {
    pub(crate) fn field_at(
        &self,
        form_tab: DbConnFormTab,
        mx: usize,
        my: usize,
    ) -> Option<DbConnField> {
        if rect_contains(self.name_input, mx, my) {
            return Some(DbConnField::Name);
        }
        match form_tab {
            DbConnFormTab::General => {
                if rect_contains(self.host_input, mx, my) {
                    Some(DbConnField::Host)
                } else if rect_contains(self.port_input, mx, my) {
                    Some(DbConnField::Port)
                } else if rect_contains(self.database_input, mx, my) {
                    Some(DbConnField::Database)
                } else if rect_contains(self.user_input, mx, my) {
                    Some(DbConnField::User)
                } else if rect_contains(self.password_input, mx, my) {
                    Some(DbConnField::Password)
                } else {
                    None
                }
            }
            DbConnFormTab::Tls => {
                if rect_contains(self.tls_server_name, mx, my) {
                    Some(DbConnField::TlsServerName)
                } else if rect_contains(self.tls_ca_path, mx, my) {
                    Some(DbConnField::TlsCaPath)
                } else if rect_contains(self.tls_cert_path, mx, my) {
                    Some(DbConnField::TlsCertPath)
                } else if rect_contains(self.tls_key_path, mx, my) {
                    Some(DbConnField::TlsKeyPath)
                } else {
                    None
                }
            }
            DbConnFormTab::Tunnel => {
                if rect_contains(self.tunnel_host, mx, my) {
                    Some(DbConnField::TunnelHost)
                } else if rect_contains(self.tunnel_port, mx, my) {
                    Some(DbConnField::TunnelPort)
                } else if rect_contains(self.tunnel_user, mx, my) {
                    Some(DbConnField::TunnelUser)
                } else if rect_contains(self.tunnel_key_path, mx, my) {
                    Some(DbConnField::TunnelKeyPath)
                } else if rect_contains(self.tunnel_fingerprint, mx, my) {
                    Some(DbConnField::TunnelFingerprint)
                } else {
                    None
                }
            }
            DbConnFormTab::Advanced => None,
        }
    }
}

/// Geometría de la vista principal de base de datos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DbViewRects {
    pub container: Rect,
    /// Barra de sub-navegación superior (Conexiones, Tabla, Editor SQL, Esquema)
    pub sub_nav: Rect,
    pub nav_tabs: Vec<Rect>,
    pub prod_banner: Option<Rect>,
    pub main_area: Rect,
    // --- Vista Browse (06) ---
    pub filter_bar: Option<Rect>,
    pub where_input: Option<Rect>,
    pub order_input: Option<Rect>,
    pub page_prev_btn: Option<Rect>,
    pub page_next_btn: Option<Rect>,
    pub grid_rect: Option<Rect>,
    pub inspector_rect: Option<Rect>,
    pub pending_bar: Option<Rect>,
    pub pending_discard_btn: Option<Rect>,
    pub pending_apply_btn: Option<Rect>,
    // --- Vista Query (07) ---
    pub query_toolbar: Option<Rect>,
    pub query_exec_btn: Option<Rect>,
    pub query_explain_btn: Option<Rect>,
    pub query_format_btn: Option<Rect>,
    pub query_tx_toggle: Option<Rect>,
    pub query_save_btn: Option<Rect>,
    pub query_editor_rect: Option<Rect>,
    pub query_status_bar: Option<Rect>,
    pub query_results_nav: Option<Rect>,
    pub query_results_tabs: Vec<Rect>,
    pub query_export_csv_btn: Option<Rect>,
    pub query_export_json_btn: Option<Rect>,
    pub query_copy_btn: Option<Rect>,
    pub query_results_grid: Option<Rect>,
    // --- Vista Schema (08) ---
    pub schema_tables_list: Option<Rect>,
    pub schema_toolbar: Option<Rect>,
    pub schema_keys_toggle: Option<Rect>,
    pub schema_arrange_btn: Option<Rect>,
    pub schema_zoom_minus_btn: Option<Rect>,
    pub schema_zoom_plus_btn: Option<Rect>,
    pub schema_export_btn: Option<Rect>,
    pub schema_cards_area: Option<Rect>,
    pub schema_card_rects: Vec<(String, Rect)>,
    pub schema_detail_panel: Option<Rect>,
    pub schema_detail_ddl_btn: Option<Rect>,
    pub schema_detail_open_browse_btn: Option<Rect>,
    // --- Vista Connections (05) ---
    pub conn_list_rect: Option<Rect>,
    pub conn_items: Vec<(String, Rect)>,
    pub conn_form_rect: Option<Rect>,
    pub conn_form_tabs: Vec<Rect>,
    pub conn_connect_btn: Option<Rect>,
    pub conn_test_btn: Option<Rect>,
    pub conn_save_btn: Option<Rect>,
    pub conn_delete_btn: Option<Rect>,
    pub conn_adv_duplicate_btn: Option<Rect>,
    pub conn_adv_settings_btn: Option<Rect>,
    pub conn_fields: Option<DbConnFormRects>,
    pub conn_test_panel: Option<Rect>,
}

impl DbViewRects {
    pub(crate) const SUB_NAV_H: usize = 38;
    pub(crate) const PROD_BANNER_H: usize = 32;
    pub(crate) const FILTER_BAR_H: usize = 36;
    pub(crate) const PENDING_BAR_H: usize = 42;
    pub(crate) const INSPECTOR_W: usize = 280;
    pub(crate) const SIDE_PANEL_W: usize = 260;

    /// Calcula la geometría de la vista de datos según el modo activo.
    pub(crate) fn compute(
        container: Rect,
        mode: DbViewMode,
        is_prod_write: bool,
        has_pending_changes: bool,
        show_inspector: bool,
        connections: &[Connection],
    ) -> Self {
        let (cx, cy, cw, ch) = container;
        if cw < 300 || ch < 200 {
            return Self::default();
        }

        // 1. Barra de sub-navegación superior (38 px)
        let sub_nav = (cx, cy, cw, Self::SUB_NAV_H);
        let nav_tabs = vec![
            (cx + 12, cy + 4, 124, Self::SUB_NAV_H - 8),
            (cx + 12 + 128, cy + 4, 132, Self::SUB_NAV_H - 8),
            (cx + 12 + 128 + 136, cy + 4, 118, Self::SUB_NAV_H - 8),
            (cx + 12 + 128 + 136 + 122, cy + 4, 108, Self::SUB_NAV_H - 8),
        ];

        let mut content_y = cy + Self::SUB_NAV_H;
        let mut content_h = ch.saturating_sub(Self::SUB_NAV_H);

        // 2. Banner de producción opcional (32 px)
        let prod_banner = if is_prod_write && mode == DbViewMode::Browse {
            let r = (cx, content_y, cw, Self::PROD_BANNER_H);
            content_y += Self::PROD_BANNER_H;
            content_h = content_h.saturating_sub(Self::PROD_BANNER_H);
            Some(r)
        } else {
            None
        };

        let main_area = (cx, content_y, cw, content_h);

        let mut res = Self {
            container,
            sub_nav,
            nav_tabs,
            prod_banner,
            main_area,
            ..Default::default()
        };

        match mode {
            DbViewMode::Connections => {
                let list_w = Self::SIDE_PANEL_W.min(cw / 3);
                let form_w = cw.saturating_sub(list_w + 1);
                res.conn_list_rect = Some((cx, content_y, list_w, content_h));
                let form_x = cx + list_w + 1;
                res.conn_form_rect = Some((form_x, content_y, form_w, content_h));

                // 2a. Rectángulos exactos para cada conexión agrupada por entorno
                let mut cur_y = content_y + 12;
                let mut conn_items = Vec::new();
                for env in [Env::Prod, Env::Stg, Env::Local] {
                    let in_env: Vec<_> = connections.iter().filter(|c| c.env == env).collect();
                    if in_env.is_empty() {
                        continue;
                    }
                    cur_y += 24; // Cabecera del grupo
                    for c in in_env {
                        conn_items
                            .push((c.id.clone(), (cx + 8, cur_y, list_w.saturating_sub(16), 34)));
                        cur_y += 38;
                    }
                    cur_y += 8;
                }
                res.conn_items = conn_items;

                // 2b. Pestañas del formulario (General, SSL/TLS, Túnel SSH, Avanzado)
                let form_tab_y = content_y + 64;
                res.conn_form_tabs = vec![
                    (form_x + 16, form_tab_y, 96, 28),
                    (form_x + 16 + 102, form_tab_y, 102, 28),
                    (form_x + 16 + 102 + 108, form_tab_y, 112, 28),
                    (form_x + 16 + 102 + 108 + 118, form_tab_y, 104, 28),
                ];

                // 2c. Botones de acción en cabecera superior derecha
                let btn_h = 30;
                let btn_y = content_y + 20;
                let del_w = 32;
                res.conn_delete_btn =
                    Some((form_x + form_w.saturating_sub(44), btn_y, del_w, btn_h));
                let save_x = form_x + form_w.saturating_sub(44 + 84);
                res.conn_save_btn = Some((save_x, btn_y, 76, btn_h));
                res.conn_test_btn = Some((save_x.saturating_sub(138), btn_y, 130, btn_h));
                res.conn_connect_btn = Some((save_x.saturating_sub(138 + 104), btn_y, 96, btn_h));

                // 2d. Campos interactivos del formulario
                let pad = 20;
                let body_y = content_y + 108;
                let body_w = form_w.saturating_sub(pad * 2);

                let name_input = (
                    form_x + 16,
                    btn_y,
                    240.min(form_w.saturating_sub(458)),
                    btn_h,
                );

                // General:
                let r1_y = body_y;
                let engine_btns = vec![
                    (Engine::Postgres, (form_x + pad, r1_y + 18, 105, 28)),
                    (Engine::Mysql, (form_x + pad + 109, r1_y + 18, 100, 28)),
                    (
                        Engine::SqlServer,
                        (form_x + pad + 109 + 104, r1_y + 18, 115, 28),
                    ),
                ];
                let env_x = form_x + pad + 350;
                let env_btns = vec![
                    (Env::Local, (env_x, r1_y + 18, 75, 28)),
                    (Env::Stg, (env_x + 79, r1_y + 18, 85, 28)),
                    (Env::Prod, (env_x + 79 + 89, r1_y + 18, 110, 28)),
                ];

                let r2_y = r1_y + 62;
                let port_w = 90.min(body_w / 4);
                let host_w = body_w.saturating_sub(port_w + 16).min(420);
                let host_input = (form_x + pad, r2_y + 18, host_w, 32);
                let port_input = (form_x + pad + host_w + 12, r2_y + 18, port_w, 32);

                let r3_y = r2_y + 64;
                let database_input = (
                    form_x + pad,
                    r3_y + 18,
                    (body_w / 2).max(280).min(body_w),
                    32,
                );

                let r4_y = r3_y + 64;
                let half_w = (body_w.saturating_sub(16) / 2).min(280);
                let user_input = (form_x + pad, r4_y + 18, half_w, 32);
                let password_input = (form_x + pad + half_w + 16, r4_y + 18, half_w, 32);

                let r5_y = r4_y + 62;
                let read_only_toggle = (form_x + pad, r5_y + 10, body_w, 28);

                // SSL / TLS:
                let ssl_mode_btns = vec![
                    (SslMode::Disable, (form_x + pad, body_y + 18, 116, 28)),
                    (SslMode::Require, (form_x + pad + 122, body_y + 18, 96, 28)),
                    (
                        SslMode::VerifyCa,
                        (form_x + pad + 224, body_y + 18, 110, 28),
                    ),
                    (
                        SslMode::VerifyFull,
                        (form_x + pad + 340, body_y + 18, 144, 28),
                    ),
                ];
                let tls_server_name = (form_x + pad, body_y + 64 + 18, body_w.min(420), 32);
                let tls_ca_path = (form_x + pad, body_y + 64 * 2 + 18, body_w.min(520), 32);
                let tls_cert_path = (form_x + pad, body_y + 64 * 3 + 18, body_w.min(520), 32);
                let tls_key_path = (form_x + pad, body_y + 64 * 4 + 18, body_w.min(520), 32);

                // Túnel SSH:
                let tunnel_toggle = (form_x + pad, body_y + 6, body_w, 28);
                let tun_r2_y = body_y + 48;
                let tunnel_port_w = 90.min(body_w / 4);
                let tunnel_host_w = body_w.saturating_sub(tunnel_port_w + 16).min(380);
                let tunnel_host = (form_x + pad, tun_r2_y + 18, tunnel_host_w, 32);
                let tunnel_port = (
                    form_x + pad + tunnel_host_w + 12,
                    tun_r2_y + 18,
                    tunnel_port_w,
                    32,
                );
                let tunnel_user = (form_x + pad, tun_r2_y + 64 + 18, body_w.min(320), 32);
                let tunnel_key_path = (form_x + pad, tun_r2_y + 64 * 2 + 18, body_w.min(520), 32);
                let tunnel_fingerprint =
                    (form_x + pad, tun_r2_y + 64 * 3 + 18, body_w.min(450), 32);

                res.conn_fields = Some(DbConnFormRects {
                    name_input,
                    engine_btns,
                    env_btns,
                    host_input,
                    port_input,
                    database_input,
                    user_input,
                    password_input,
                    read_only_toggle,
                    ssl_mode_btns,
                    tls_server_name,
                    tls_ca_path,
                    tls_cert_path,
                    tls_key_path,
                    tunnel_toggle,
                    tunnel_host,
                    tunnel_port,
                    tunnel_user,
                    tunnel_key_path,
                    tunnel_fingerprint,
                });

                res.conn_adv_duplicate_btn = Some((form_x + pad, body_y + 88, 176, 30));
                res.conn_adv_settings_btn = Some((form_x + pad + 188, body_y + 88, 210, 30));

                let test_panel_h = 160;
                res.conn_test_panel = Some((
                    form_x + 16,
                    content_y + content_h.saturating_sub(test_panel_h + 12),
                    form_w.saturating_sub(32),
                    test_panel_h,
                ));
            }

            DbViewMode::Browse => {
                // Barra de filtros superior
                let filter_bar = (cx, content_y, cw, Self::FILTER_BAR_H);
                let w_inp_w = 260.min(cw.saturating_sub(320));
                let o_inp_w = 180.min(cw.saturating_sub(w_inp_w + 200));
                let where_input = (cx + 70, content_y + 4, w_inp_w, 28);
                let order_input = (cx + 70 + w_inp_w + 80, content_y + 4, o_inp_w, 28);
                let nav_x = cx + cw.saturating_sub(120);
                let page_prev_btn = (nav_x, content_y + 4, 28, 28);
                let page_next_btn = (nav_x + 32, content_y + 4, 28, 28);

                res.filter_bar = Some(filter_bar);
                res.where_input = Some(where_input);
                res.order_input = Some(order_input);
                res.page_prev_btn = Some(page_prev_btn);
                res.page_next_btn = Some(page_next_btn);

                let grid_y = content_y + Self::FILTER_BAR_H;
                let mut grid_h = content_h.saturating_sub(Self::FILTER_BAR_H);

                // Barra inferior de cambios pendientes
                if has_pending_changes {
                    let p_y = content_y + content_h.saturating_sub(Self::PENDING_BAR_H);
                    res.pending_bar = Some((cx, p_y, cw, Self::PENDING_BAR_H));
                    res.pending_discard_btn = Some((cx + cw.saturating_sub(230), p_y + 6, 80, 30));
                    res.pending_apply_btn = Some((cx + cw.saturating_sub(142), p_y + 6, 130, 30));
                    grid_h = grid_h.saturating_sub(Self::PENDING_BAR_H);
                }

                // Inspector lateral
                if show_inspector && cw > 600 {
                    let insp_w = Self::INSPECTOR_W;
                    let grid_w = cw.saturating_sub(insp_w + 1);
                    res.grid_rect = Some((cx, grid_y, grid_w, grid_h));
                    res.inspector_rect = Some((cx + grid_w + 1, grid_y, insp_w, grid_h));
                } else {
                    res.grid_rect = Some((cx, grid_y, cw, grid_h));
                }
            }

            DbViewMode::Query => {
                let tb_h = 38;
                let query_toolbar = (cx, content_y, cw, tb_h);
                res.query_toolbar = Some(query_toolbar);

                let btn_y = content_y + 5;
                let btn_h = 28;
                let exec_w = 120;
                res.query_exec_btn = Some((cx + 12, btn_y, exec_w, btn_h));

                let exp_w = 85;
                res.query_explain_btn = Some((cx + 12 + exec_w + 8, btn_y, exp_w, btn_h));

                let fmt_w = 95;
                res.query_format_btn =
                    Some((cx + 12 + exec_w + 8 + exp_w + 8, btn_y, fmt_w, btn_h));

                let tx_w = 130;
                let tx_x = cx + 12 + exec_w + 8 + exp_w + 8 + fmt_w + 14;
                res.query_tx_toggle = Some((tx_x, btn_y, tx_w, btn_h));
                res.query_save_btn = Some((tx_x + tx_w + 8, btn_y, 88, btn_h));

                let sb_h = 28;
                let ed_y = content_y + tb_h;
                let main_rem = (content_h.saturating_sub(tb_h)).max(100);
                let ed_h = (main_rem * 4 / 10).max(120);
                res.query_editor_rect = Some((cx, ed_y, cw, ed_h.saturating_sub(sb_h)));
                res.query_status_bar = Some((cx, ed_y + ed_h.saturating_sub(sb_h), cw, sb_h));

                let res_y = ed_y + ed_h;
                let res_h = (content_y + content_h).saturating_sub(res_y);
                let res_nav_h = 36;
                res.query_results_nav = Some((cx, res_y, cw, res_nav_h));

                let r_tab_w = 100;
                let r_plan_w = 135;
                let r_msg_w = 90;
                let tab_y = res_y + 4;
                let tab_h = res_nav_h.saturating_sub(8);
                res.query_results_tabs = vec![
                    (cx + 12, tab_y, r_tab_w, tab_h),
                    (cx + 12 + r_tab_w + 4, tab_y, r_plan_w, tab_h),
                    (cx + 12 + r_tab_w + 4 + r_plan_w + 4, tab_y, r_msg_w, tab_h),
                ];

                let copy_w = 60;
                let json_w = 50;
                let csv_w = 46;
                let export_btn_h = 24;
                let export_btn_y = res_y + (res_nav_h.saturating_sub(export_btn_h)) / 2;
                let r_end = cx + cw.saturating_sub(14);
                res.query_copy_btn = Some((
                    r_end.saturating_sub(copy_w),
                    export_btn_y,
                    copy_w,
                    export_btn_h,
                ));
                res.query_export_json_btn = Some((
                    r_end.saturating_sub(copy_w + 6 + json_w),
                    export_btn_y,
                    json_w,
                    export_btn_h,
                ));
                res.query_export_csv_btn = Some((
                    r_end.saturating_sub(copy_w + 6 + json_w + 6 + csv_w),
                    export_btn_y,
                    csv_w,
                    export_btn_h,
                ));

                let grid_y = res_y + res_nav_h;
                let grid_h = res_h.saturating_sub(res_nav_h);
                res.query_results_grid = Some((cx, grid_y, cw, grid_h));
            }

            DbViewMode::Schema => {
                let tree_w = Self::SIDE_PANEL_W.min(cw / 3);
                res.schema_tables_list = Some((cx, content_y, tree_w, content_h));

                let det_w = if cw > 800 { Self::INSPECTOR_W } else { 0 };
                let center_x = cx + tree_w + 1;
                let center_w = cw.saturating_sub(tree_w + det_w + 2);

                // Barra superior de herramientas del lienzo
                let tb_h = 38;
                let schema_toolbar = (center_x, content_y, center_w, tb_h);
                res.schema_toolbar = Some(schema_toolbar);

                let btn_y = content_y + 5;
                let btn_h = 28;

                // Segmentado: Columnas / Claves
                let segm_w = 140;
                let segm_x = center_x + center_w.saturating_sub(320);
                res.schema_keys_toggle = Some((segm_x, btn_y, segm_w, btn_h));

                // Botón Ordenar
                let arr_w = 70;
                let arr_x = segm_x.saturating_sub(arr_w + 8);
                res.schema_arrange_btn = Some((arr_x, btn_y, arr_w, btn_h));

                // Control Zoom: [ - | 100% | + ]
                let zoom_x = segm_x + segm_w + 8;
                res.schema_zoom_minus_btn = Some((zoom_x, btn_y, 26, btn_h));
                res.schema_zoom_plus_btn = Some((zoom_x + 26 + 36, btn_y, 26, btn_h));

                // Botón Exportar
                let exp_w = 74;
                let exp_x = center_x + center_w.saturating_sub(exp_w + 12);
                res.schema_export_btn = Some((exp_x, btn_y, exp_w, btn_h));

                let canvas_y = content_y + tb_h;
                let canvas_h = content_h.saturating_sub(tb_h);
                res.schema_cards_area = Some((center_x, canvas_y, center_w, canvas_h));

                if det_w > 0 {
                    let det_x = center_x + center_w + 1;
                    res.schema_detail_panel = Some((det_x, content_y, det_w, content_h));
                    let d_btn_w = det_w.saturating_sub(32);
                    res.schema_detail_ddl_btn = Some((
                        det_x + 16,
                        content_y + content_h.saturating_sub(76),
                        d_btn_w,
                        30,
                    ));
                    res.schema_detail_open_browse_btn = Some((
                        det_x + 16,
                        content_y + content_h.saturating_sub(38),
                        d_btn_w,
                        30,
                    ));
                }
            }
        }

        res
    }

    /// Detecta qué pestaña superior de la barra de navegación se pulsó (0: Conexiones, 1: Datos, 2: Query, 3: Schema).
    pub(crate) fn hit_nav_tab(&self, mx: usize, my: usize) -> Option<usize> {
        self.nav_tabs
            .iter()
            .position(|&tab| rect_contains(tab, mx, my))
    }
}

/// Geometría para el panel contextual lateral del módulo de Datos (riel -> DATOS).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DbSidebarRects {
    pub container: Rect,
    pub conn_selector: Rect,
    pub subtab_switch: Rect,
    pub schema_tab_btn: Rect,
    pub query_tab_btn: Rect,
    pub search_input: Rect,
    pub items_area: Rect,
    pub new_conn_btn: Rect,
}

impl DbSidebarRects {
    pub(crate) fn compute(container: Rect) -> Self {
        let (cx, cy, cw, ch) = container;
        let conn_selector = (cx + 8, cy + 6, cw.saturating_sub(16), 34);

        let sw_y = cy + 46;
        let subtab_w = (cw.saturating_sub(20)) / 2;
        let subtab_switch = (cx + 8, sw_y, cw.saturating_sub(16), 28);
        let schema_tab_btn = (cx + 8, sw_y, subtab_w, 28);
        let query_tab_btn = (cx + 10 + subtab_w, sw_y, subtab_w, 28);

        let search_y = sw_y + 34;
        let search_input = (cx + 8, search_y, cw.saturating_sub(16), 28);

        let items_y = search_y + 34;
        let bottom_btn_h = 36;
        let items_h = (cy + ch).saturating_sub(items_y + bottom_btn_h + 8);
        let items_area = (cx, items_y, cw, items_h);

        let new_conn_btn = (
            cx + 8,
            cy + ch.saturating_sub(bottom_btn_h + 4),
            cw.saturating_sub(16),
            bottom_btn_h,
        );

        Self {
            container,
            conn_selector,
            subtab_switch,
            schema_tab_btn,
            query_tab_btn,
            search_input,
            items_area,
            new_conn_btn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_view_rects_compute_modes() {
        let container = (56, 40, 1384, 832);

        // Browse mode con inspector y pending changes
        let r_browse = DbViewRects::compute(container, DbViewMode::Browse, true, true, true, &[]);
        assert_eq!(r_browse.nav_tabs.len(), 4);
        assert!(r_browse.prod_banner.is_some());
        assert!(r_browse.filter_bar.is_some());
        assert!(r_browse.grid_rect.is_some());
        assert!(r_browse.inspector_rect.is_some());
        assert!(r_browse.pending_bar.is_some());

        // Hit testing nav tabs
        let tab0 = r_browse.nav_tabs[0];
        assert_eq!(r_browse.hit_nav_tab(tab0.0 + 2, tab0.1 + 2), Some(0));

        // Query mode
        let r_query = DbViewRects::compute(container, DbViewMode::Query, false, false, false, &[]);
        assert!(r_query.query_toolbar.is_some());
        assert!(r_query.query_exec_btn.is_some());
        assert!(r_query.query_explain_btn.is_some());
        assert!(r_query.query_format_btn.is_some());
        assert!(r_query.query_editor_rect.is_some());
        assert!(r_query.query_status_bar.is_some());
        assert!(r_query.query_export_csv_btn.is_some());
        assert!(r_query.query_export_json_btn.is_some());
        assert!(r_query.query_copy_btn.is_some());
        assert!(r_query.query_results_grid.is_some());

        // Schema mode
        let r_schema =
            DbViewRects::compute(container, DbViewMode::Schema, false, false, false, &[]);
        assert!(r_schema.schema_tables_list.is_some());
        assert!(r_schema.schema_toolbar.is_some());
        assert!(r_schema.schema_keys_toggle.is_some());
        assert!(r_schema.schema_arrange_btn.is_some());
        assert!(r_schema.schema_zoom_minus_btn.is_some());
        assert!(r_schema.schema_zoom_plus_btn.is_some());
        assert!(r_schema.schema_export_btn.is_some());
        assert!(r_schema.schema_cards_area.is_some());
        assert!(r_schema.schema_detail_panel.is_some());
        assert!(r_schema.schema_detail_ddl_btn.is_some());
        assert!(r_schema.schema_detail_open_browse_btn.is_some());

        // Connections mode
        let mut sample_conn = Connection::new("test-conn", Engine::Postgres);
        sample_conn.env = Env::Prod;
        let r_conn = DbViewRects::compute(
            container,
            DbViewMode::Connections,
            false,
            false,
            false,
            &[sample_conn],
        );
        assert!(r_conn.conn_list_rect.is_some());
        assert!(r_conn.conn_form_rect.is_some());
        assert_eq!(r_conn.conn_items.len(), 1);
        assert!(r_conn.conn_fields.is_some());
        assert!(r_conn.conn_connect_btn.is_some());
    }

    #[test]
    fn test_db_sidebar_rects() {
        let container = (56, 40, 264, 832);
        let sb = DbSidebarRects::compute(container);
        assert_eq!(sb.container, container);
        assert!(sb.items_area.3 > 100);
    }
}

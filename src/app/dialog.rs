//! Diálogos modales y avisos (toasts) unificados para todo el sistema (DISENO §5, pantallas 11 y 12).
//!
//! Cubre el catálogo completo de alertas por gravedad, confirmaciones escritas y avisos no intrusivos.

use crate::theme::{Color, Theme};
use crate::ui::LucideIcon;
use std::borrow::Cow;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Gravedad de un diálogo modal o aviso. Determina el color de la franja y el ícono.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogSeverity {
    /// Rojo: acciones destructivas o irreversibles (descartar, eliminar, sin WHERE).
    Danger,
    /// Amarillo: cambios sin guardar, permisos inseguros o huellas no reconocidas.
    Warning,
    /// Azul: información, actualizaciones y pasos guiados.
    Info,
}

impl DialogSeverity {
    /// Color semántico correspondiente de la paleta Catppuccin.
    pub(crate) fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Danger => theme.red(),
            Self::Warning => theme.yellow(),
            Self::Info => theme.accent_color(),
        }
    }

    /// Color de la franja superior tintada con mezcla suave sobre el fondo mantle.
    pub(crate) fn tint(self, theme: &Theme) -> Color {
        let c = self.color(theme);
        Color::from_u32_xrgb(theme.mantle().blend_over(c, 42))
    }
}

/// Estilo visual del botón de acción.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogButtonKind {
    /// Botón secundario con contorno (Cancelar, Ignorar).
    Ghost,
    /// Botón primario en azul o acento.
    Primary,
    /// Botón de acción peligrosa en rojo.
    Danger,
}

/// Acciones semánticas disparadas al pulsar botones en un diálogo.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DialogAction {
    Close,
    TrustHost {
        host: String,
        ip: String,
        fingerprint: String,
    },
    DiscardUnappliedChanges,
    ExecuteDangerousQuery,
    DeleteConnection(String),
    FixPemPermissions(PathBuf),
    DiscardGitChanges,
    CloseTabWithRunningProcess(usize),
    KillProcess(u32),
    TriggerAppUpdate,
    ApplyProdChanges,
    ExecuteProdQuery,
    Custom(String),
}

/// Botón interactivo de pie de diálogo.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DialogButton {
    pub label: Cow<'static, str>,
    pub kind: DialogButtonKind,
    pub action: DialogAction,
    /// Si es `true`, solo estará habilitado cuando la confirmación escrita coincida exactamente.
    pub requires_confirmation: bool,
}

#[allow(dead_code)]
impl DialogButton {
    pub(crate) fn ghost(label: impl Into<Cow<'static, str>>, action: DialogAction) -> Self {
        Self {
            label: label.into(),
            kind: DialogButtonKind::Ghost,
            action,
            requires_confirmation: false,
        }
    }

    pub(crate) fn primary(
        label: impl Into<Cow<'static, str>>,
        action: DialogAction,
        requires_confirmation: bool,
    ) -> Self {
        Self {
            label: label.into(),
            kind: DialogButtonKind::Primary,
            action,
            requires_confirmation,
        }
    }

    pub(crate) fn danger(
        label: impl Into<Cow<'static, str>>,
        action: DialogAction,
        requires_confirmation: bool,
    ) -> Self {
        Self {
            label: label.into(),
            kind: DialogButtonKind::Danger,
            action,
            requires_confirmation,
        }
    }
}

/// Fila de impacto estimado para el diálogo de confirmación de base de datos (DbConfirm).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbImpactRow {
    pub n: String,
    pub op: String,
    pub table: String,
    pub rows: String,
}

/// Opciones de ejecución de escrituras en producción según `DbConfirm.dc.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DbConfirmOptions {
    pub in_transaction: bool,
    pub backup_rows: bool,
    pub remember_tab: bool,
}

impl Default for DbConfirmOptions {
    fn default() -> Self {
        Self {
            in_transaction: true,
            backup_rows: true,
            remember_tab: false,
        }
    }
}

/// Especificación declarativa de un diálogo modal.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DialogSpec {
    pub severity: DialogSeverity,
    pub icon: LucideIcon,
    pub title: Cow<'static, str>,
    pub body: Cow<'static, str>,
    pub mono_detail: Option<String>,
    pub confirm_keyword: Option<String>,
    pub buttons: Vec<DialogButton>,
    // Extensiones especializadas para DbConfirm (DISENO §5 pantalla 10 y pantalla 07):
    pub header_meta: Option<String>,
    pub subtitle: Option<Cow<'static, str>>,
    pub sql_code: Option<String>,
    pub impact_rows: Vec<DbImpactRow>,
    pub has_db_options: bool,
    pub can_copy_sql: bool,
}

#[allow(dead_code)]
impl DialogSpec {
    fn standard(
        severity: DialogSeverity,
        icon: LucideIcon,
        title: impl Into<Cow<'static, str>>,
        body: impl Into<Cow<'static, str>>,
        mono_detail: Option<String>,
        confirm_keyword: Option<String>,
        buttons: Vec<DialogButton>,
    ) -> Self {
        Self {
            severity,
            icon,
            title: title.into(),
            body: body.into(),
            mono_detail,
            confirm_keyword,
            buttons,
            header_meta: None,
            subtitle: None,
            sql_code: None,
            impact_rows: Vec::new(),
            has_db_options: false,
            can_copy_sql: false,
        }
    }

    /// Comprueba si este diálogo corresponde a la confirmación enriquecida de base de datos (DbConfirm).
    pub(crate) fn is_db_confirm(&self) -> bool {
        self.sql_code.is_some() || !self.impact_rows.is_empty() || self.has_db_options
    }

    // -------------------------------------------------------------------------
    // Catálogo estándar de diálogos de la aplicación (DISENO §5, pantalla 11 y pantalla 07)
    // -------------------------------------------------------------------------

    /// 1. Huella de host desconocida (Aviso).
    pub(crate) fn unknown_host_fingerprint(
        host: impl Into<String>,
        ip: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        let h = host.into();
        let i = ip.into();
        let fp = fingerprint.into();
        Self::standard(
            DialogSeverity::Warning,
            LucideIcon::FileKey,
            "Huella de host desconocida",
            format!(
                "No se puede verificar la autenticidad de {} ({}). Confirma que la huella coincide.",
                h, i
            ),
            Some(fp.clone()),
            None,
            vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::primary(
                    "Confiar y conectar",
                    DialogAction::TrustHost {
                        host: h,
                        ip: i,
                        fingerprint: fp,
                    },
                    false,
                ),
            ],
        )
    }

    /// 2. Cambios sin aplicar en base de datos (Aviso).
    pub(crate) fn unapplied_changes(
        target: &str,
        count: usize,
        sql_preview: impl Into<String>,
    ) -> Self {
        Self::standard(
            DialogSeverity::Warning,
            LucideIcon::Info,
            "Cambios sin aplicar",
            format!(
                "Hay {} {} sin aplicar en {}. Si cierras la pestaña se perderán.",
                count,
                if count == 1 { "cambio" } else { "cambios" },
                target
            ),
            Some(sql_preview.into()),
            None,
            vec![
                DialogButton::ghost("Descartar", DialogAction::DiscardUnappliedChanges),
                DialogButton::primary("Revisar y aplicar", DialogAction::Close, false),
            ],
        )
    }

    /// 3. Sentencia sin WHERE o masiva (Peligro + confirmación escrita).
    pub(crate) fn statement_without_where(
        table: &str,
        row_count: usize,
        sql_preview: &str,
    ) -> Self {
        Self::standard(
            DialogSeverity::Danger,
            LucideIcon::Trash2,
            "Sentencia sin WHERE",
            format!(
                "La sentencia afectaría a todas las filas ({} {}) en la tabla {}. No se puede deshacer fuera de una transacción.",
                row_count,
                if row_count == 1 { "fila" } else { "filas" },
                table
            ),
            Some(sql_preview.to_string()),
            Some(table.to_string()),
            vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::danger("Ejecutar", DialogAction::ExecuteDangerousQuery, true),
            ],
        )
    }

    /// Confirmación avanzada de escritura en producción según `DbConfirm.dc.html`.
    pub(crate) fn db_prod_write_confirm(
        conn_name: &str,
        db_name: &str,
        title: impl Into<Cow<'static, str>>,
        subtitle: impl Into<Cow<'static, str>>,
        sql: &str,
        impact_rows: Vec<DbImpactRow>,
    ) -> Self {
        let clean_sql = sql.trim().to_string();
        Self {
            severity: DialogSeverity::Danger,
            icon: LucideIcon::Lock,
            title: "Escritura en producción".into(),
            body: title.into(),
            mono_detail: None,
            confirm_keyword: Some(conn_name.to_string()),
            buttons: vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::danger("Aplicar en PROD", DialogAction::ApplyProdChanges, true),
            ],
            header_meta: Some(format!("{} · {}", conn_name, db_name)),
            subtitle: Some(subtitle.into()),
            sql_code: Some(clean_sql),
            impact_rows,
            has_db_options: true,
            can_copy_sql: true,
        }
    }

    /// Confirmación avanzada para ejecutar consultas SQL destructivas o en producción.
    pub(crate) fn db_prod_query_confirm(
        conn_name: &str,
        db_name: &str,
        title: impl Into<Cow<'static, str>>,
        subtitle: impl Into<Cow<'static, str>>,
        sql: &str,
        impact_rows: Vec<DbImpactRow>,
    ) -> Self {
        let mut spec =
            Self::db_prod_write_confirm(conn_name, db_name, title, subtitle, sql, impact_rows);
        if let Some(btn) = spec.buttons.get_mut(1) {
            btn.label = "Ejecutar en PROD".into();
            btn.action = DialogAction::ExecuteProdQuery;
        }
        spec
    }

    /// Confirmación de escritura en producción (DISENO §5 pantalla 10 y pantalla 07 DbConfirm).
    pub(crate) fn prod_write_confirm(
        conn_name: &str,
        db_name: &str,
        change_count: usize,
        table: &str,
        sql_preview: &str,
        impact_rows: Vec<DbImpactRow>,
    ) -> Self {
        let title = format!(
            "Aplicar {} {} en {}",
            change_count,
            if change_count == 1 {
                "cambio"
            } else {
                "cambios"
            },
            table
        );
        let subtitle = "Estás conectado a una base de datos de producción. Revisa el SQL y el impacto antes de continuar.";
        Self::db_prod_write_confirm(
            conn_name,
            db_name,
            title,
            subtitle,
            sql_preview,
            impact_rows,
        )
    }

    /// 4. Eliminar conexión a base de datos (Peligro).
    pub(crate) fn delete_connection(conn_id: &str, conn_name: &str) -> Self {
        Self::standard(
            DialogSeverity::Danger,
            LucideIcon::Trash2,
            "Eliminar conexión",
            format!(
                "Se quitarán {} y su contraseña guardada del Administrador de credenciales. La base de datos remota no se toca.",
                conn_name
            ),
            None,
            None,
            vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::danger(
                    "Eliminar",
                    DialogAction::DeleteConnection(conn_id.to_string()),
                    false,
                ),
            ],
        )
    }

    /// 5. Permisos inseguros de clave `.pem` (Aviso).
    pub(crate) fn pem_permissions(key_name: &str, path: PathBuf) -> Self {
        let p_str = path.display().to_string();
        Self::standard(
            DialogSeverity::Warning,
            LucideIcon::FileKey,
            "Permisos de clave .pem",
            format!(
                "La clave privada {} es legible por otros usuarios de Windows. El túnel SSH la rechazará.",
                key_name
            ),
            Some(p_str),
            None,
            vec![
                DialogButton::ghost("Ignorar", DialogAction::Close),
                DialogButton::primary(
                    "Corregir permisos",
                    DialogAction::FixPemPermissions(path),
                    false,
                ),
            ],
        )
    }

    /// 6. Descartar cambios de Git (Peligro).
    pub(crate) fn discard_git_changes(count: usize) -> Self {
        Self::standard(
            DialogSeverity::Danger,
            LucideIcon::Trash2,
            "Descartar cambios",
            format!(
                "Se descartarán {} {} sin preparar. Los archivos nuevos sin seguimiento no se tocan.",
                count,
                if count == 1 { "cambio" } else { "cambios" }
            ),
            Some("git restore .".into()),
            None,
            vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::danger("Descartar", DialogAction::DiscardGitChanges, false),
            ],
        )
    }

    /// 7. Proceso en ejecución al cerrar pestaña (Aviso).
    pub(crate) fn closing_tab_running_process(tab_index: usize, process_name: &str) -> Self {
        Self::standard(
            DialogSeverity::Warning,
            LucideIcon::Terminal,
            "Proceso en ejecución",
            format!(
                "Hay un proceso activo ({}) ejecutándose en esta pestaña. Si la cierras, el proceso finalizará.",
                process_name
            ),
            None,
            None,
            vec![
                DialogButton::ghost("Mantener pestaña", DialogAction::Close),
                DialogButton::danger(
                    "Cerrar pestaña",
                    DialogAction::CloseTabWithRunningProcess(tab_index),
                    false,
                ),
            ],
        )
    }

    /// 8. Actualización disponible (Información).
    pub(crate) fn update_available(new_version: &str) -> Self {
        Self::standard(
            DialogSeverity::Info,
            LucideIcon::RefreshCw,
            "Actualización disponible",
            format!(
                "TEArminal v{} está lista para instalar. La descarga está verificada con firma digital.",
                new_version
            ),
            None,
            None,
            vec![
                DialogButton::ghost("Más tarde", DialogAction::Close),
                DialogButton::primary("Actualizar ahora", DialogAction::TriggerAppUpdate, false),
            ],
        )
    }

    /// 9. Terminar proceso del sistema (Peligro).
    pub(crate) fn kill_process_confirm(pid: u32, process_name: &str) -> Self {
        Self::standard(
            DialogSeverity::Danger,
            LucideIcon::Trash2,
            "Terminar proceso",
            format!(
                "Se forzará la finalización de {} (PID {}). Cualquier trabajo no guardado en el proceso se perderá.",
                process_name, pid
            ),
            Some(format!("taskkill /PID {} /F", pid)),
            None,
            vec![
                DialogButton::ghost("Cancelar", DialogAction::Close),
                DialogButton::danger("Terminar proceso", DialogAction::KillProcess(pid), false),
            ],
        )
    }
}

/// Estado interactivo de un diálogo en ejecución.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DialogState {
    pub spec: DialogSpec,
    pub input: crate::ui::TextInput,
    pub hovered_button: Option<usize>,
    pub hovered_close: bool,
    pub db_options: DbConfirmOptions,
    pub hovered_copy_sql: bool,
    pub hovered_option: Option<usize>,
}

impl DialogState {
    pub(crate) fn new(spec: DialogSpec) -> Self {
        Self {
            spec,
            input: crate::ui::TextInput::default(),
            hovered_button: None,
            hovered_close: false,
            db_options: DbConfirmOptions::default(),
            hovered_copy_sql: false,
            hovered_option: None,
        }
    }

    /// Alterna el estado de una de las 3 opciones del diálogo de base de datos.
    pub(crate) fn toggle_db_option(&mut self, idx: usize) {
        match idx {
            0 => self.db_options.in_transaction = !self.db_options.in_transaction,
            1 => self.db_options.backup_rows = !self.db_options.backup_rows,
            2 => self.db_options.remember_tab = !self.db_options.remember_tab,
            _ => {}
        }
    }

    /// Comprueba si el texto de confirmación escrita coincide exactamente con la palabra clave.
    pub(crate) fn is_confirmed(&self) -> bool {
        let Some(kw) = &self.spec.confirm_keyword else {
            return true;
        };
        self.input.text().trim() == kw.trim()
    }

    /// Determina si un botón está habilitado para hacer clic.
    pub(crate) fn is_button_enabled(&self, btn_idx: usize) -> bool {
        let Some(btn) = self.spec.buttons.get(btn_idx) else {
            return false;
        };
        if btn.requires_confirmation {
            self.is_confirmed()
        } else {
            true
        }
    }

    /// Acción por defecto al presionar `Enter` (el botón de confirmación si está habilitado).
    pub(crate) fn default_confirm_action(&self) -> Option<DialogAction> {
        for (i, btn) in self.spec.buttons.iter().enumerate() {
            if (btn.kind == DialogButtonKind::Primary || btn.kind == DialogButtonKind::Danger)
                && self.is_button_enabled(i)
            {
                return Some(btn.action.clone());
            }
        }
        None
    }
}

/// Gravedad de un aviso emergente (Toast).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastSeverity {
    Success,
    Warning,
    Error,
    Info,
}

impl ToastSeverity {
    pub(crate) fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Success => theme.green(),
            Self::Warning => theme.yellow(),
            Self::Error => theme.red(),
            Self::Info => theme.accent_color(),
        }
    }
}

/// Notificación breve no intrusiva con borde semántico.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Toast {
    pub id: u64,
    pub severity: ToastSeverity,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Toast {
    pub(crate) fn new(id: u64, severity: ToastSeverity, message: impl Into<String>) -> Self {
        Self {
            id,
            severity,
            message: message.into(),
            created_at: Instant::now(),
            duration: Duration::from_secs(4),
        }
    }

    pub(crate) fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

use super::TearminalApp;

impl TearminalApp {
    /// Calcula la geometría exacta del diálogo modal activo usando las métricas reales de fuente.
    pub(super) fn dialog_rects_for_active(
        &mut self,
        win_w: usize,
        win_h: usize,
    ) -> Option<crate::renderer::DialogRects> {
        let dlg = self.active_dialog.as_ref()?;
        let button_widths: Vec<usize> = {
            let renderer = self.renderer.as_mut()?;
            dlg.spec
                .buttons
                .iter()
                .map(|b| {
                    let text_w = renderer.measure_ui_text_tracked(
                        &b.label,
                        crate::renderer::tokens::DETAIL,
                        crate::renderer::tokens::TRACKING,
                    );
                    text_w + crate::renderer::DialogRects::BTN_PAD_X * 2
                })
                .collect()
        };
        Some(crate::renderer::DialogRects::compute_for_spec(
            win_w,
            win_h,
            &dlg.spec,
            &button_widths,
        ))
    }

    /// Clic con un diálogo modal abierto. Consume el evento por completo.
    pub(super) fn click_dialog(&mut self, mx: usize, my: usize) {
        let (win_w, win_h) = self.window_size();
        let Some(rects) = self.dialog_rects_for_active(win_w, win_h) else {
            return;
        };

        let hit = rects.hit_test(mx, my);
        match hit {
            Some(crate::renderer::DialogHit::Close) => {
                self.close_dialog();
            }
            Some(crate::renderer::DialogHit::CopySql) => {
                let sql_opt = self
                    .active_dialog
                    .as_ref()
                    .and_then(|d| d.spec.sql_code.clone());
                if let Some(sql) = sql_opt {
                    self.set_clipboard_text(&sql);
                    self.show_toast(ToastSeverity::Success, "SQL copiado al portapapeles");
                }
            }
            Some(crate::renderer::DialogHit::OptionCheckbox(idx)) => {
                if let Some(dlg) = &mut self.active_dialog {
                    dlg.toggle_db_option(idx);
                }
            }
            Some(crate::renderer::DialogHit::Button(idx)) => {
                if let Some(dlg) = &self.active_dialog {
                    if dlg.is_button_enabled(idx) {
                        let action = dlg.spec.buttons[idx].action.clone();
                        let db_options = dlg.db_options.clone();
                        let sql_opt = dlg.spec.sql_code.clone();
                        self.close_dialog();
                        self.run_dialog_action_with_db_options(action, db_options, sql_opt);
                    }
                }
            }
            Some(crate::renderer::DialogHit::Input) => {
                if let Some(dlg) = &mut self.active_dialog {
                    if let Some(inp_rect) = rects.confirm_input {
                        let mut input = std::mem::take(&mut dlg.input);
                        self.handle_text_input_click(&mut input, mx, inp_rect.0 + 8);
                        if let Some(dlg) = &mut self.active_dialog {
                            dlg.input = input;
                        }
                    }
                }
            }
            None => {
                // Si el clic fue fuera de la tarjeta, cerramos solo si no es Danger
                if !crate::renderer::rect_contains(rects.card, mx, my) {
                    if let Some(dlg) = &self.active_dialog {
                        if dlg.spec.severity != DialogSeverity::Danger {
                            self.close_dialog();
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    /// Ejecuta la acción semántica disparada por un diálogo modal considerando opciones de base de datos.
    pub(super) fn run_dialog_action_with_db_options(
        &mut self,
        action: DialogAction,
        db_options: DbConfirmOptions,
        sql_opt: Option<String>,
    ) {
        if matches!(
            action,
            DialogAction::ApplyProdChanges | DialogAction::ExecuteProdQuery
        ) {
            if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(ref mut db) = active_tab.db_tab {
                    if db_options.remember_tab {
                        db.prod_write_expires_at = Some(Instant::now() + db.write_window);
                    }
                    let conn = db
                        .active_connection_id
                        .as_ref()
                        .and_then(|id| db.connections.store.find(id))
                        .cloned()
                        .unwrap_or_else(|| db.connections.build_connection_from_form());
                    let conn_name = conn.name.clone();
                    let confirm = crate::db::session::WriteConfirmation {
                        typed_name: Some(conn_name),
                        dangers_acknowledged: true,
                    };
                    if action == DialogAction::ExecuteProdQuery {
                        let sql = sql_opt
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| db.query.editor_input.text.trim().to_string());
                        if !sql.is_empty() {
                            self.spawn_db_execute_query(conn, sql, confirm);
                        }
                    } else {
                        let sql = sql_opt
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| db.browse.generated_sql());
                        if !sql.is_empty() {
                            self.spawn_db_apply_changes(conn, sql, confirm);
                        }
                    }
                }
            }
            return;
        }
        self.run_dialog_action(action);
    }

    /// Ejecuta la acción semántica disparada por un diálogo.
    pub(super) fn run_dialog_action(&mut self, action: DialogAction) {
        self.close_dialog();
        match action {
            DialogAction::Close => {}
            DialogAction::TrustHost {
                host, fingerprint, ..
            } => {
                if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = active_tab.db_tab {
                        let mut changed = false;
                        for conn in &mut db.connections.store.connections {
                            if conn.tunnel.host.eq_ignore_ascii_case(&host) {
                                conn.tunnel.known_fingerprint = Some(fingerprint.clone());
                                changed = true;
                            }
                        }
                        if db
                            .connections
                            .tunnel_host
                            .text
                            .trim()
                            .eq_ignore_ascii_case(&host)
                        {
                            db.connections.tunnel_fingerprint =
                                crate::ui::TextInput::new(&fingerprint);
                        }
                        if changed {
                            let _ = db
                                .connections
                                .store
                                .save_to(&crate::db::store::ConnectionStore::default_path());
                        }
                    }
                }
                self.show_toast(
                    ToastSeverity::Success,
                    format!("Huella de {} verificada", host),
                );
            }
            DialogAction::DiscardUnappliedChanges => {
                if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = active_tab.db_tab {
                        db.browse.discard_pending();
                    }
                }
                self.show_toast(ToastSeverity::Info, "Cambios sin aplicar descartados");
            }
            DialogAction::ExecuteDangerousQuery => {
                self.execute_db_query_from_editor();
            }
            DialogAction::DeleteConnection(conn_id) => {
                if let Some(active_tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(ref mut db) = active_tab.db_tab {
                        db.connections.store.remove(&conn_id);
                        let _ = db
                            .connections
                            .store
                            .save_to(&crate::db::store::ConnectionStore::default_path());
                        let _ = crate::db::credentials::delete(
                            crate::db::credentials::SecretKind::DbPassword,
                            &conn_id,
                        );
                        if db.connections.selected_id.as_deref() == Some(&conn_id) {
                            db.connections.selected_id = db
                                .connections
                                .store
                                .connections
                                .first()
                                .map(|c| c.id.clone());
                            if let Some(ref sel) = db.connections.selected_id.clone() {
                                db.connections.select_connection(sel);
                            }
                        }
                    }
                }
                self.show_toast(
                    ToastSeverity::Info,
                    format!("Conexión {} eliminada", conn_id),
                );
            }
            DialogAction::FixPemPermissions(path) => {
                match crate::db::perm::restrict_to_current_user(&path) {
                    Ok(_) => {
                        self.show_toast(
                            ToastSeverity::Success,
                            "Permisos de clave .pem corregidos",
                        );
                    }
                    Err(e) => {
                        self.show_toast(ToastSeverity::Error, format!("Error de permisos: {e}"));
                    }
                }
            }
            DialogAction::DiscardGitChanges => {
                if let Some(repo) = self.file_explorer.git_root.clone() {
                    self.spawn_git_bulk_action(
                        repo,
                        crate::renderer::GitBulkActionKind::DiscardAll,
                    );
                    self.show_toast(ToastSeverity::Info, "Cambios de Git descartados");
                }
            }
            DialogAction::KillProcess(pid) => {
                crate::monitor::kill_process_by_pid(pid);
                self.monitor.selected_process_pid = None;
                self.request_immediate_monitor_sample();
                self.show_toast(
                    ToastSeverity::Info,
                    format!("Proceso PID {} terminado", pid),
                );
            }
            DialogAction::CloseTabWithRunningProcess(tab_idx) => {
                self.force_close_tab(tab_idx);
            }
            DialogAction::TriggerAppUpdate => {
                crate::updater::trigger_update();
                self.show_toast(ToastSeverity::Info, "Iniciando actualización...");
            }
            DialogAction::ApplyProdChanges => {
                self.run_dialog_action_with_db_options(
                    DialogAction::ApplyProdChanges,
                    DbConfirmOptions::default(),
                    None,
                );
            }
            DialogAction::ExecuteProdQuery => {
                self.run_dialog_action_with_db_options(
                    DialogAction::ExecuteProdQuery,
                    DbConfirmOptions::default(),
                    None,
                );
            }
            DialogAction::Custom(msg) => {
                self.show_toast(ToastSeverity::Info, msg);
            }
        }
    }

    /// Procesa pulsaciones de teclado cuando hay un diálogo modal activo.
    pub(super) fn handle_dialog_key(
        &mut self,
        logical_key: &winit::keyboard::Key,
        physical_key: winit::keyboard::PhysicalKey,
        text: &Option<winit::keyboard::SmolStr>,
    ) {
        use winit::keyboard::{Key, NamedKey};
        match logical_key {
            Key::Named(NamedKey::Escape) => self.close_dialog(),
            Key::Named(NamedKey::Enter) => {
                if let Some(dlg) = &self.active_dialog {
                    if let Some(action) = dlg.default_confirm_action() {
                        let db_options = dlg.db_options.clone();
                        let sql_opt = dlg.spec.sql_code.clone();
                        self.close_dialog();
                        self.run_dialog_action_with_db_options(action, db_options, sql_opt);
                    }
                }
            }
            _ => {
                let Some(dlg) = &mut self.active_dialog else {
                    return;
                };
                if dlg.spec.confirm_keyword.is_some() {
                    let mut input = std::mem::take(&mut dlg.input);
                    let (_, changed) = self.handle_text_input_key(
                        &mut input,
                        logical_key,
                        physical_key,
                        text.as_deref(),
                    );
                    if let Some(dlg) = &mut self.active_dialog {
                        dlg.input = input;
                    }
                    if changed {
                        self.request_redraw();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_written_confirmation_validation() {
        let spec = DialogSpec::statement_without_where("usuarios", 120, "DELETE FROM usuarios");
        let mut state = DialogState::new(spec);

        // Sin escribir: no está confirmado y el botón peligroso está bloqueado
        assert!(!state.is_confirmed());
        assert!(!state.is_button_enabled(1));
        assert!(state.is_button_enabled(0)); // Cancelar (ghost) siempre activo

        // Texto parcial o incorrecto
        state.input.set_text("usuario");
        assert!(!state.is_confirmed());
        assert!(!state.is_button_enabled(1));

        // Texto exacto con posibles espacios en blanco alrededor
        state.input.set_text("  usuarios  ");
        assert!(state.is_confirmed());
        assert!(state.is_button_enabled(1));
        assert_eq!(
            state.default_confirm_action(),
            Some(DialogAction::ExecuteDangerousQuery)
        );
    }

    #[test]
    fn test_catalog_specs_construction() {
        let d1 = DialogSpec::unknown_host_fingerprint(
            "api-prod",
            "10.0.4.12",
            "SHA256:3wQ0xJ9p2m1v8Yk0aR5nTz7LdE4hB6uCfS1gXoWiVjM",
        );
        assert_eq!(d1.severity, DialogSeverity::Warning);
        assert!(d1.confirm_keyword.is_none());
        assert_eq!(d1.buttons.len(), 2);

        let d2 = DialogSpec::discard_git_changes(15);
        assert_eq!(d2.severity, DialogSeverity::Danger);
        assert_eq!(d2.mono_detail.as_deref(), Some("git restore ."));

        let d3 = DialogSpec::delete_connection("conn-mysql-1", "mysql-erp");
        assert_eq!(d3.severity, DialogSeverity::Danger);
        assert_eq!(
            d3.buttons[1].action,
            DialogAction::DeleteConnection("conn-mysql-1".into())
        );

        let dq = DialogSpec::db_prod_query_confirm(
            "pg-prod",
            "app",
            "Ejecutar DROP",
            "Confirmar",
            "DROP TABLE tmp",
            Vec::new(),
        );
        assert_eq!(dq.buttons[1].action, DialogAction::ExecuteProdQuery);

        let d4 = DialogSpec::unapplied_changes("pg-main", 3, "UPDATE users SET active = true");
        assert_eq!(d4.severity, DialogSeverity::Warning);

        let d5 = DialogSpec::pem_permissions("id_rsa", PathBuf::from("C:\\keys\\id_rsa.pem"));
        assert_eq!(d5.severity, DialogSeverity::Warning);

        let d6 = DialogSpec::closing_tab_running_process(2, "cargo build");
        assert_eq!(d6.severity, DialogSeverity::Warning);

        let d7 = DialogSpec::update_available("0.30.0");
        assert_eq!(d7.severity, DialogSeverity::Info);
    }

    #[test]
    fn test_toast_expiration() {
        let toast = Toast::new(1, ToastSeverity::Success, "Conexión restablecida");
        assert!(!toast.is_expired());
    }

    #[test]
    fn test_dialog_severity_color_and_tint() {
        let theme = Theme::from_name(Theme::DEFAULT_DARK);
        assert_eq!(DialogSeverity::Danger.color(&theme), theme.red());
        assert_eq!(DialogSeverity::Warning.color(&theme), theme.yellow());
        assert_eq!(DialogSeverity::Info.color(&theme), theme.accent_color());

        let tint = DialogSeverity::Danger.tint(&theme);
        let expected = Color::from_u32_xrgb(theme.mantle().blend_over(theme.red(), 42));
        assert_eq!(tint, expected);
        assert_ne!(tint, theme.mantle());
    }
}

//! Estado y recolección de consumo de IA: cuotas en vivo por modelo, ventanas de 5 horas y tokens locales.
//!
//! Combina tres fuentes sin bloquear nunca el bucle de eventos:
//! 1. Archivos locales de sesión (`~/.gemini/antigravity-cli/brain/*/transcript.jsonl`, `settings.json`,
//!    `~/.claude.json`, `~/.claude/projects/**/*.jsonl`) con caché de varios segundos.
//! 2. Consulta en segundo plano de cuotas por modelo (`fetchAvailableModels`) usando la credencial nativa de
//!    Windows (`CredReadW`) y `curl.exe -K -` por `stdin` (sin exponer tokens en línea de comandos).
//! 3. Lectura de la rejilla de la terminal activa por si una TUI muestra métricas de contexto/coste en pantalla.

use super::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Cuota de un modelo o familia de modelos (con ventana de reinicio de 5h o multi-día)
#[derive(Debug, Clone)]
pub struct AiModelQuota {
    pub name: String,
    /// Fracción consumida `0.0..=1.0` (`1.0 - remainingFraction`)
    pub used_fraction: f32,
    /// Etiqueta de ventana / reinicio (p. ej. `"5h · Reinicia en 4h 48m"`)
    pub reset_label: String,
    pub is_active: bool,
}

/// Estadísticas de tokens acumulados en un intervalo (Sesión actual, Ventana 5h, Últimas 24h)
#[derive(Debug, Clone, Default)]
pub struct AiTokenWindow {
    pub input_tokens: u64,
    pub cache_tokens: u64,
    pub output_tokens: u64,
    pub turns: u32,
    /// Segundos hasta que se reinicia la ventana de 5h (basado en el turno más antiguo dentro de las 5h)
    pub reset_in_secs: Option<u64>,
}

impl AiTokenWindow {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.cache_tokens + self.output_tokens
    }
}

/// Fila informativa de un agente o modelo detectado localmente
#[derive(Debug, Clone)]
pub struct AiAgentSummary {
    pub agent: String,
    pub model: String,
    pub detail: String,
}

/// Instantánea lista para renderizar en el menú flotante de la barra de estado
#[derive(Debug, Clone, Default)]
pub struct AiUsageSnapshot {
    pub active_model: Option<String>,
    pub active_plan: Option<String>,
    pub quotas: Vec<AiModelQuota>,
    pub window_5h: AiTokenWindow,
    pub current_session: AiTokenWindow,
    pub window_24h_tokens: u64,
    pub window_24h_sessions: u32,
    pub agents: Vec<AiAgentSummary>,
    pub terminal_hint: Option<String>,
}

/// Resultado del escaneo en segundo plano de configuraciones y transcripciones locales (`.jsonl`)
pub(super) struct LocalUsageScan {
    pub active_model: Option<String>,
    pub active_plan: Option<String>,
    pub window_5h: AiTokenWindow,
    pub current_session: AiTokenWindow,
    pub window_24h_tokens: u64,
    pub window_24h_sessions: u32,
    pub agents: Vec<AiAgentSummary>,
}

pub struct AiUsageState {
    pub snapshot: AiUsageSnapshot,
    last_local_scan: Option<Instant>,
    pub local_scan_in_flight: bool,
}

impl Default for AiUsageState {
    fn default() -> Self {
        let mut state = Self {
            snapshot: AiUsageSnapshot::default(),
            last_local_scan: None,
            local_scan_in_flight: false,
        };
        state.refresh_config_only();
        state
    }
}

impl AiUsageState {
    /// Actualiza la pista de la terminal y devuelve si se debe lanzar el escaneo local en segundo plano
    pub fn refresh_if_needed(&mut self, terminal: Option<&TerminalState>, force: bool) -> bool {
        self.snapshot.terminal_hint = terminal.and_then(scrape_terminal_usage_line);

        let now = Instant::now();
        let should_scan = !self.local_scan_in_flight
            && (force
                || self
                    .last_local_scan
                    .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(3)));
        if should_scan {
            self.local_scan_in_flight = true;
            self.last_local_scan = Some(now);
        }
        should_scan
    }

    pub(super) fn apply_local_scan(&mut self, scan: LocalUsageScan) {
        self.local_scan_in_flight = false;
        self.snapshot.active_model = scan.active_model;
        self.snapshot.active_plan = scan.active_plan;
        self.snapshot.window_5h = scan.window_5h;
        self.snapshot.current_session = scan.current_session;
        self.snapshot.window_24h_tokens = scan.window_24h_tokens;
        self.snapshot.window_24h_sessions = scan.window_24h_sessions;
        self.snapshot.agents = scan.agents;
        self.rebuild_quota_rows();
    }

    fn rebuild_quota_rows(&mut self) {
        let mut rows = Vec::new();
        if self.snapshot.window_5h.total_tokens() > 0 {
            let reset_label = match self.snapshot.window_5h.reset_in_secs {
                Some(rem) if rem > 0 => format!("5h · Reinicia en {}", format_duration(rem)),
                _ => "Ventana 5h activa".to_string(),
            };
            let model_name = self
                .snapshot
                .active_model
                .clone()
                .unwrap_or_else(|| "Modelo activo (ventana 5h)".to_string());
            let approx_frac = (self.snapshot.window_5h.turns as f32 / 150.0).clamp(0.02, 0.99);
            rows.push(AiModelQuota {
                name: model_name,
                used_fraction: approx_frac,
                reset_label,
                is_active: true,
            });
        }
        self.snapshot.quotas = rows;
    }

    /// Lectura ligera al arrancar: solo los JSON de configuración (< 4 KB), sin abrir transcripciones `.jsonl`
    fn refresh_config_only(&mut self) {
        let Some(home_dir) = std::env::var("USERPROFILE").ok().map(PathBuf::from) else {
            return;
        };
        let agy_dir = home_dir.join(".gemini").join("antigravity-cli");
        let mut agy_model = None;
        let mut agy_mode = None;
        if let Ok(cfg) = std::fs::read_to_string(agy_dir.join("settings.json")) {
            agy_model = extract_json_str(&cfg, "model");
            agy_mode = extract_json_str(&cfg, "agentMode");
        }
        let mut claude_model = None;
        let mut claude_plan = None;
        if let Ok(c_set) = std::fs::read_to_string(home_dir.join(".claude").join("settings.json")) {
            claude_model = extract_json_str(&c_set, "model");
        }
        let mut gemini_model = None;
        if let Ok(g_set) = std::fs::read_to_string(home_dir.join(".gemini").join("settings.json")) {
            gemini_model = extract_json_str(&g_set, "model");
        }
        if claude_plan.is_none() {
            if let Some(cfg) = read_transcript_tail(&home_dir.join(".claude.json"), 64 * 1024) {
                if let Some(org_type) = extract_json_str(&cfg, "organizationType") {
                    claude_plan = Some(match org_type.as_str() {
                        "claude_pro" => "Claude Pro".to_string(),
                        "claude_max" => "Claude Max".to_string(),
                        "claude_team" => "Claude Team".to_string(),
                        "claude_enterprise" => "Claude Enterprise".to_string(),
                        other => other.replace('_', " "),
                    });
                }
            }
        }
        self.snapshot.active_model = agy_model.or(claude_model).or(gemini_model);
        self.snapshot.active_plan = claude_plan.or(agy_mode);
    }
}

pub(super) fn scan_local_usage_blocking(ai_status: AiAgentStatus) -> LocalUsageScan {
    let home = std::env::var("USERPROFILE").ok().map(PathBuf::from);
    let now_epoch = now_unix_secs();

    let mut agy_model: Option<String> = None;
    let mut agy_mode: Option<String> = None;
    let mut claude_model: Option<String> = None;
    let mut claude_plan: Option<String> = None;
    let mut gemini_model: Option<String> = None;

    let mut window_5h = AiTokenWindow::default();
    let mut current_session = AiTokenWindow::default();
    let mut window_24h_tokens = 0u64;
    let mut window_24h_sessions = 0u32;
    let mut oldest_in_5h: Option<u64> = None;

    let mut agy_5h_tokens = 0u64;
    let mut agy_5h_turns = 0u32;
    let mut claude_5h_tokens = 0u64;
    let mut claude_5h_turns = 0u32;

    if let Some(ref home_dir) = home {
        // 1. Configuración y transcripts de Antigravity (AGY)
        let agy_dir = home_dir.join(".gemini").join("antigravity-cli");
        if let Ok(cfg) = std::fs::read_to_string(agy_dir.join("settings.json")) {
            agy_model = extract_json_str(&cfg, "model");
            agy_mode = extract_json_str(&cfg, "agentMode");
        }

        let brain_dir = agy_dir.join("brain");
        if let Ok(entries) = std::fs::read_dir(&brain_dir) {
            let mut transcripts: Vec<(SystemTime, PathBuf)> = entries
                .flatten()
                .filter_map(|e| {
                    let p = e
                        .path()
                        .join(".system_generated")
                        .join("logs")
                        .join("transcript.jsonl");
                    let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
                    Some((mtime, p))
                })
                .collect();
            transcripts.sort_by_key(|a| std::cmp::Reverse(a.0));

            for (idx, (mtime, path)) in transcripts.iter().take(12).enumerate() {
                let age_secs = SystemTime::now()
                    .duration_since(*mtime)
                    .unwrap_or_default()
                    .as_secs();
                if age_secs > 48 * 3600 && idx > 0 {
                    continue;
                }
                let stats = scan_agy_transcript(path, now_epoch, &mut oldest_in_5h);
                if idx == 0 {
                    current_session = stats.session;
                    if stats.last_model.is_some() && agy_model.is_none() {
                        agy_model = stats.last_model;
                    }
                }
                window_5h.input_tokens += stats.win_5h.input_tokens;
                window_5h.cache_tokens += stats.win_5h.cache_tokens;
                window_5h.output_tokens += stats.win_5h.output_tokens;
                window_5h.turns += stats.win_5h.turns;
                agy_5h_tokens += stats.win_5h.total_tokens();
                agy_5h_turns += stats.win_5h.turns;

                if stats.tokens_24h > 0 {
                    window_24h_tokens += stats.tokens_24h;
                    window_24h_sessions += 1;
                }
            }
        }

        // 2. Configuración y proyectos de Claude Code
        if let Some(claude_cfg) = read_transcript_tail(&home_dir.join(".claude.json"), 64 * 1024) {
            if let Some(org_type) = extract_json_str(&claude_cfg, "organizationType") {
                claude_plan = Some(match org_type.as_str() {
                    "claude_pro" => "Claude Pro".to_string(),
                    "claude_max" => "Claude Max".to_string(),
                    "claude_team" => "Claude Team".to_string(),
                    "claude_enterprise" => "Claude Enterprise".to_string(),
                    other => other.replace('_', " "),
                });
            }
            if let Some(label) = extract_json_str(&claude_cfg, "label") {
                if let Some(val) = extract_json_str(&claude_cfg, "value") {
                    claude_model = Some(format!("{label} ({val})"));
                } else {
                    claude_model = Some(label);
                }
            }
        }
        if let Ok(c_set) = std::fs::read_to_string(home_dir.join(".claude").join("settings.json")) {
            if let Some(m) = extract_json_str(&c_set, "model") {
                claude_model = Some(m);
            }
        }

        // Sesiones JSONL de Claude Code en ~/.claude/projects/**/*.jsonl
        let claude_projects = home_dir.join(".claude").join("projects");
        if let Ok(proj_dirs) = std::fs::read_dir(&claude_projects) {
            let mut dirs: Vec<(SystemTime, PathBuf)> = proj_dirs
                .flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok()?.modified().ok()?;
                    Some((m, e.path()))
                })
                .collect();
            dirs.sort_by_key(|a| std::cmp::Reverse(a.0));

            let mut session_files: Vec<(SystemTime, PathBuf)> = Vec::new();
            for (_, dir_path) in dirs.iter().take(3) {
                if let Ok(files) = std::fs::read_dir(dir_path) {
                    for f in files.flatten() {
                        let p = f.path();
                        if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                            if let Ok(m) = f.metadata().and_then(|m| m.modified()) {
                                session_files.push((m, p));
                            }
                        }
                    }
                }
            }
            session_files.sort_by_key(|a| std::cmp::Reverse(a.0));
            for (idx, (mtime, path)) in session_files.iter().take(5).enumerate() {
                let age_secs = SystemTime::now()
                    .duration_since(*mtime)
                    .unwrap_or_default()
                    .as_secs();
                if age_secs > 48 * 3600 {
                    continue;
                }
                let stats = scan_claude_transcript(path, now_epoch, &mut oldest_in_5h);
                if idx == 0
                    && (ai_status == AiAgentStatus::Claude || current_session.total_tokens() == 0)
                {
                    current_session = stats.session;
                }
                if stats.last_model.is_some() {
                    claude_model = stats.last_model;
                }
                window_5h.input_tokens += stats.win_5h.input_tokens;
                window_5h.cache_tokens += stats.win_5h.cache_tokens;
                window_5h.output_tokens += stats.win_5h.output_tokens;
                window_5h.turns += stats.win_5h.turns;
                claude_5h_tokens += stats.win_5h.total_tokens();
                claude_5h_turns += stats.win_5h.turns;
                if stats.tokens_24h > 0 {
                    window_24h_tokens += stats.tokens_24h;
                    window_24h_sessions += 1;
                }
            }
        }

        // 3. Gemini CLI (~/.gemini/settings.json)
        if let Ok(g_set) = std::fs::read_to_string(home_dir.join(".gemini").join("settings.json")) {
            gemini_model = extract_json_str(&g_set, "model");
        }
    }

    if let Some(oldest) = oldest_in_5h {
        let expires_at = oldest + 5 * 3600;
        if expires_at > now_epoch {
            window_5h.reset_in_secs = Some(expires_at - now_epoch);
        }
    }

    let active_model = match ai_status {
        AiAgentStatus::Claude => claude_model.clone().or_else(|| agy_model.clone()),
        AiAgentStatus::Agy => agy_model.clone(),
        AiAgentStatus::Gemini => gemini_model.clone().or_else(|| agy_model.clone()),
        _ => agy_model
            .clone()
            .or_else(|| claude_model.clone())
            .or_else(|| gemini_model.clone()),
    };

    let mut agents = Vec::new();
    if agy_model.is_some() || agy_5h_tokens > 0 {
        let m = agy_model
            .clone()
            .unwrap_or_else(|| "Gemini / Claude".to_string());
        let detail = if agy_5h_tokens > 0 {
            format!(
                "5h: {} tok ({} turnos)",
                format_tokens(agy_5h_tokens),
                agy_5h_turns
            )
        } else if let Some(ref mode) = agy_mode {
            format!("Modo: {mode}")
        } else {
            "Listo".to_string()
        };
        agents.push(AiAgentSummary {
            agent: "AGY".to_string(),
            model: m,
            detail,
        });
    }
    if claude_plan.is_some() || claude_model.is_some() || claude_5h_tokens > 0 {
        let m = claude_model
            .clone()
            .or_else(|| claude_plan.clone())
            .unwrap_or_else(|| "Claude Sonnet / Opus".to_string());
        let detail = if claude_5h_tokens > 0 {
            format!(
                "5h: {} tok ({} turnos)",
                format_tokens(claude_5h_tokens),
                claude_5h_turns
            )
        } else if let Some(ref plan) = claude_plan {
            format!("Plan: {plan}")
        } else {
            "Configurado".to_string()
        };
        agents.push(AiAgentSummary {
            agent: "Claude".to_string(),
            model: m,
            detail,
        });
    }
    if let Some(gm) = gemini_model {
        agents.push(AiAgentSummary {
            agent: "Gemini".to_string(),
            model: gm,
            detail: "CLI local".to_string(),
        });
    }

    LocalUsageScan {
        active_model,
        active_plan: claude_plan.or(agy_mode),
        window_5h,
        current_session,
        window_24h_tokens,
        window_24h_sessions,
        agents,
    }
}

const MAX_TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;

/// Lee un archivo de texto acotando la lectura a los últimos `max_bytes` si es muy grande
fn read_transcript_tail(path: &Path, max_bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len <= max_bytes {
        let mut buf = Vec::with_capacity(len as usize);
        f.read_to_end(&mut buf).ok()?;
        return Some(String::from_utf8_lossy(&buf).into_owned());
    }
    f.seek(SeekFrom::Start(len - max_bytes)).ok()?;
    let mut buf = Vec::with_capacity(max_bytes as usize);
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    Some(match text.split_once('\n') {
        Some((_, rest)) => rest.to_string(),
        None => text.into_owned(),
    })
}

struct TranscriptScan {
    session: AiTokenWindow,
    win_5h: AiTokenWindow,
    tokens_24h: u64,
    last_model: Option<String>,
}

fn scan_agy_transcript(
    path: &Path,
    now_epoch: u64,
    oldest_in_5h: &mut Option<u64>,
) -> TranscriptScan {
    let mut scan = TranscriptScan {
        session: AiTokenWindow::default(),
        win_5h: AiTokenWindow::default(),
        tokens_24h: 0,
        last_model: None,
    };
    let Some(content) = read_transcript_tail(path, MAX_TRANSCRIPT_TAIL_BYTES) else {
        return scan;
    };
    let cutoff_5h = now_epoch.saturating_sub(5 * 3600);
    let cutoff_24h = now_epoch.saturating_sub(24 * 3600);

    for line in content.lines() {
        if let Some((_, after)) = line.split_once("Model Selection` from ") {
            if let Some((_, to_part)) = after.split_once(" to ") {
                let model: String = to_part
                    .chars()
                    .take_while(|&c| c != '.' && c != '\\' && c != '"')
                    .collect();
                let trimmed = model.trim();
                if !trimmed.is_empty() {
                    scan.last_model = Some(trimmed.to_string());
                }
            }
        }
        if !line.contains("\"input_tokens\":") && !line.contains("\"output_tokens\":") {
            continue;
        }
        let inp = extract_json_u64(line, "input_tokens").unwrap_or(0);
        let cache = extract_json_u64(line, "cache_read_tokens").unwrap_or(0);
        let out = extract_json_u64(line, "output_tokens").unwrap_or(0);
        if inp == 0 && cache == 0 && out == 0 {
            continue;
        }
        scan.session.input_tokens += inp;
        scan.session.cache_tokens += cache;
        scan.session.output_tokens += out;
        scan.session.turns += 1;

        if let Some(ts_str) = extract_json_str(line, "created_at") {
            if let Some(ts) = parse_iso8601_utc(&ts_str) {
                if ts >= cutoff_24h {
                    scan.tokens_24h += inp + cache + out;
                }
                if ts >= cutoff_5h {
                    scan.win_5h.input_tokens += inp;
                    scan.win_5h.cache_tokens += cache;
                    scan.win_5h.output_tokens += out;
                    scan.win_5h.turns += 1;
                    *oldest_in_5h = Some(oldest_in_5h.map_or(ts, |old| old.min(ts)));
                }
            }
        }
    }
    scan
}

fn scan_claude_transcript(
    path: &Path,
    now_epoch: u64,
    oldest_in_5h: &mut Option<u64>,
) -> TranscriptScan {
    let mut scan = TranscriptScan {
        session: AiTokenWindow::default(),
        win_5h: AiTokenWindow::default(),
        tokens_24h: 0,
        last_model: None,
    };
    let Some(content) = read_transcript_tail(path, MAX_TRANSCRIPT_TAIL_BYTES) else {
        return scan;
    };
    let cutoff_5h = now_epoch.saturating_sub(5 * 3600);
    let cutoff_24h = now_epoch.saturating_sub(24 * 3600);

    for line in content.lines() {
        if !line.contains("\"usage\"") {
            continue;
        }
        if let Some(m) = extract_json_str(line, "model") {
            if !m.starts_with('<') {
                scan.last_model = Some(m);
            }
        }
        let inp = extract_json_u64(line, "input_tokens").unwrap_or(0);
        let cache_create = extract_json_u64(line, "cache_creation_input_tokens").unwrap_or(0);
        let cache_read = extract_json_u64(line, "cache_read_input_tokens").unwrap_or(0);
        let out = extract_json_u64(line, "output_tokens").unwrap_or(0);
        let cache = cache_create + cache_read;
        if inp == 0 && cache == 0 && out == 0 {
            continue;
        }
        scan.session.input_tokens += inp;
        scan.session.cache_tokens += cache;
        scan.session.output_tokens += out;
        scan.session.turns += 1;

        if let Some(ts_str) = extract_json_str(line, "timestamp") {
            if let Some(ts) = parse_iso8601_utc(&ts_str) {
                if ts >= cutoff_24h {
                    scan.tokens_24h += inp + cache + out;
                }
                if ts >= cutoff_5h {
                    scan.win_5h.input_tokens += inp;
                    scan.win_5h.cache_tokens += cache;
                    scan.win_5h.output_tokens += out;
                    scan.win_5h.turns += 1;
                    *oldest_in_5h = Some(oldest_in_5h.map_or(ts, |old| old.min(ts)));
                }
            }
        }
    }
    scan
}

/// Busca en las últimas líneas visibles de la terminal indicadores de contexto/tokens/coste de TUIs
fn scrape_terminal_usage_line(term: &TerminalState) -> Option<String> {
    for row in term.grid.iter().rev().take(8) {
        let line: String = row.iter().map(|c| c.c).collect();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if (lower.contains("tokens")
            || lower.contains("context")
            || lower.contains("cost:")
            || lower.contains("quota")
            || lower.contains("5h"))
            && trimmed.len() <= 96
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins:02}m")
    } else {
        format!("{}m", mins.max(1))
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Convierte una fecha ISO-8601 UTC (`"YYYY-MM-DDTHH:MM:SS..."`) a segundos Unix sin dependencias externas
pub(crate) fn parse_iso8601_utc(s: &str) -> Option<u64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..10].parse().ok()?;
    let hh: u64 = s[11..13].parse().ok()?;
    let mm: u64 = s[14..16].parse().ok()?;
    let ss: u64 = s[17..19].parse().ok()?;

    // Algoritmo civil-from-days de Howard Hinnant
    let y_adj = y - if m <= 2 { 1 } else { 0 };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = (y_adj - era * 400) as u64;
    let mp = (m + if m > 2 { -3 } else { 9 }) as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe as i64 - 719468;
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86400 + hh * 3600 + mm * 60 + ss)
}

fn extract_json_str(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let idx = text.find(&pattern)?;
    let rest = text[idx + pattern.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(esc) = chars.next() {
                out.push(esc);
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn extract_json_u64(text: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{key}\"");
    let idx = text.find(&pattern)?;
    let rest = text[idx + pattern.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso8601_utc_and_formatting() {
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_iso8601_utc("2026-10-18T15:54:26Z"),
            Some(1_792_338_866)
        );
        assert_eq!(
            parse_iso8601_utc("  2026-10-18T15:54:26Z  "),
            Some(1_792_338_866)
        );
        assert_eq!(
            parse_iso8601_utc("2026-10-18T15:54:26.517580Z"),
            Some(1_792_338_866)
        );
        assert_eq!(format_tokens(850), "850");
        assert_eq!(format_tokens(18_400), "18.4K");
        assert_eq!(format_tokens(1_250_000), "1.25M");
        assert_eq!(format_duration(4 * 3600 + 48 * 60), "4h 48m");
        assert_eq!(format_duration(2 * 86400 + 5 * 3600), "2d 5h");
    }

    #[test]
    fn test_ai_usage_popup_renders_and_hit_tests() {
        let cfg = AxonConfig::default();
        let theme = Theme::ghostty_dark();
        let mut renderer = TerminalRenderer::new(&cfg, 1.0);
        let (w, h) = (900, 600);
        let mut buf = vec![theme.background.to_u32_xrgb(); w * h];

        let mut state = AiUsageState::default();
        state.snapshot.active_model = Some("Gemini 4 Argon (Medium)".to_string());
        state.snapshot.active_plan = Some("Claude Pro".to_string());
        state.snapshot.quotas = vec![
            AiModelQuota {
                name: "Gemini 4 Argon / 3.8 Flash".to_string(),
                used_fraction: 0.12,
                reset_label: "Reinicia en 4h 30m".to_string(),
                is_active: true,
            },
            AiModelQuota {
                name: "Claude Opus 5.5 (Low / Med / High)".to_string(),
                used_fraction: 0.55,
                reset_label: "Reinicia en 2d".to_string(),
                is_active: false,
            },
        ];

        renderer.render_status_bar_with_ai(
            &mut buf,
            w,
            h,
            0,
            Some("~\\dev\\axon"),
            Some("main"),
            None,
            100,
            30,
            AiAgentStatus::Agy,
            state.snapshot.active_model.as_deref(),
            true,
            false,
            &theme,
        );
        let before_popup = buf.clone();
        renderer.render_ai_usage_popup(&mut buf, w, h, &state.snapshot, &theme);
        assert_ne!(
            buf, before_popup,
            "El menú flotante de consumo de IA debe modificar el framebuffer al renderizarse"
        );

        let popup_h = TerminalRenderer::ai_usage_popup_height(&state.snapshot);
        let popup_rect = StatusBarRects::popup_rect(w, h, popup_h);
        let sb_rects = StatusBarRects::compute(w, h, 180);
        assert!(StatusBarRects::inside_badge_or_popup(
            sb_rects.ai_badge.0 + 4,
            sb_rects.ai_badge.1 + 4,
            sb_rects.ai_badge,
            popup_rect
        ));
        assert!(StatusBarRects::inside_badge_or_popup(
            popup_rect.0 + 10,
            popup_rect.1 + 10,
            sb_rects.ai_badge,
            popup_rect
        ));
    }
}

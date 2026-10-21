//! Estado y recolección de consumo de IA: cuotas en vivo por modelo, ventanas de 5 horas y tokens locales.
//!
//! Combina dos fuentes sin bloquear nunca el bucle de eventos:
//! 1. Archivos locales de sesión (`~/.gemini/antigravity-cli/brain/*/transcript.jsonl`, `settings.json`,
//!    `~/.claude.json`, `~/.claude/projects/**/*.jsonl`) con caché de varios segundos.
//! 2. Lectura de la rejilla de la terminal activa por si una TUI muestra métricas de contexto/coste en pantalla.

use super::*;
use crate::json::{extract_json_str, extract_json_u64};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Límite de la cuenta (ventana de 5 h, semanal…) tal como lo informa el servidor
#[derive(Debug, Clone)]
pub struct AiLimit {
    pub label: String,
    /// Consumido, `0.0..=100.0`
    pub percent: f32,
    /// Instante Unix del reinicio
    pub resets_at: Option<u64>,
}

impl AiLimit {
    pub fn resets_in_secs(&self) -> Option<u64> {
        let left = self.resets_at?.saturating_sub(now_unix_secs());
        (left > 0).then_some(left)
    }
}

const USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

/// Token de la sesión iniciada de Claude Code (`~/.claude/.credentials.json`). Solo lectura: nunca se
/// renueva (rotaría el refresh token y cerraría la sesión de Claude Code); caducado → `None` hasta que
/// Claude lo renueve. El token no se registra ni se muestra.
fn token_from_credentials(creds: &str, now_epoch: u64) -> Option<String> {
    let oauth = &creds[creds.find("\"claudeAiOauth\"")?..];
    if extract_json_u64(oauth, "expiresAt")? / 1000 <= now_epoch + 30 {
        return None;
    }
    // Va como cabecera HTTP: nada de saltos de línea ni caracteres de control
    extract_json_str(oauth, "accessToken")
        .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
}

/// Pide los límites reales a la API de uso con la sesión de Claude Code. Vacío si no hay sesión válida,
/// no hay red o la respuesta no es la esperada.
fn fetch_claude_limits(home: &Path, now_epoch: u64) -> Vec<AiLimit> {
    let Ok(creds) = std::fs::read_to_string(home.join(".claude").join(".credentials.json")) else {
        return Vec::new();
    };
    let Some(token) = token_from_credentials(&creds, now_epoch) else {
        return Vec::new();
    };
    let mut cmd = Command::new("curl.exe");
    // `-H @-` lee las cabeceras de stdin: el token no aparece en la lista de procesos
    cmd.args([
        "-s",
        "--max-time",
        "5",
        "-H",
        "@-",
        "-H",
        "anthropic-beta: oauth-2025-04-20",
        "-H",
        "User-Agent: Axon-Terminal",
        USAGE_ENDPOINT,
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    super::platform::hide_console(&mut cmd);
    let Ok(mut child) = cmd.spawn() else {
        return Vec::new();
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "Authorization: Bearer {token}");
    }
    match child.wait_with_output() {
        Ok(out) if out.status.success() => {
            parse_claude_limits(&String::from_utf8_lossy(&out.stdout))
        }
        _ => Vec::new(),
    }
}

/// Número `"key": 45.0` del texto (los helpers de `crate::json` solo leen enteros)
fn json_f32(text: &str, key: &str) -> Option<f32> {
    let marker = format!("\"{key}\":");
    let rest = text[text.find(&marker)? + marker.len()..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Entradas del arreglo `limits` de la respuesta (`kind`, `percent`, `resets_at`)
fn parse_claude_limits(body: &str) -> Vec<AiLimit> {
    let Some(start) = body.find("\"limits\":[") else {
        return Vec::new();
    };
    let list = body[start..].split(']').next().unwrap_or_default();
    list.split('{')
        .skip(1)
        .filter_map(|obj| {
            let kind = extract_json_str(obj, "kind")?;
            let percent = json_f32(obj, "percent")?;
            let label = match kind.as_str() {
                "session" => "Sesión (5 h)".to_string(),
                "weekly_all" => "Semana (todos los modelos)".to_string(),
                other => other.replace('_', " "),
            };
            Some(AiLimit {
                label,
                percent: percent.clamp(0.0, 100.0),
                resets_at: extract_json_str(obj, "resets_at").and_then(|s| parse_iso8601_utc(&s)),
            })
        })
        .collect()
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

/// Consumo de un agente de IA con sesión en la pestaña activa: cada uno con su modelo y sus ventanas
#[derive(Debug, Clone)]
pub struct AiAgentUsage {
    pub kind: AiAgentStatus,
    pub model: String,
    /// Plan (Claude) o modo (AGY)
    pub plan: Option<String>,
    pub window_5h: AiTokenWindow,
    pub session: AiTokenWindow,
}

impl AiUsageSnapshot {
    /// Límites reales de un agente (hoy solo Claude los expone)
    pub fn limits_for(&self, kind: AiAgentStatus) -> &[AiLimit] {
        if kind == AiAgentStatus::Claude {
            &self.claude_limits
        } else {
            &[]
        }
    }

    /// Modelo de un agente: el del último escaneo o, antes del primero, el de su configuración
    pub fn model_for(&self, kind: AiAgentStatus) -> Option<&str> {
        let name = match kind {
            AiAgentStatus::Claude => "Claude",
            AiAgentStatus::Agy => "AGY",
            AiAgentStatus::Gemini => "Gemini",
            _ => "",
        };
        self.usages
            .iter()
            .find(|u| u.kind == kind)
            .map(|u| u.model.as_str())
            .or_else(|| {
                self.agents
                    .iter()
                    .find(|a| a.agent == name)
                    .map(|a| a.model.as_str())
            })
    }
}

impl AiAgentUsage {
    /// Hay tokens registrados localmente (Aider, Copilot y Ollama no dejan transcripciones)
    pub fn has_tokens(&self) -> bool {
        self.window_5h.total_tokens() > 0 || self.session.total_tokens() > 0
    }
}

/// Instantánea lista para renderizar en el menú flotante de la barra de estado
#[derive(Debug, Clone, Default)]
pub struct AiUsageSnapshot {
    /// Un bloque por agente distinto en los paneles de la pestaña activa; el del panel enfocado primero
    pub usages: Vec<AiAgentUsage>,
    /// Límites reales de la cuenta de Claude según el servidor (vacío si no hay sesión o no respondió)
    pub claude_limits: Vec<AiLimit>,
    pub window_24h_tokens: u64,
    pub window_24h_sessions: u32,
    pub agents: Vec<AiAgentSummary>,
    pub terminal_hint: Option<String>,
}

/// Resultado del escaneo en segundo plano de configuraciones y transcripciones locales (`.jsonl`)
pub(super) struct LocalUsageScan {
    /// `Some` solo si en este escaneo se consultó al servidor (vacío = sin sesión válida o sin respuesta)
    pub claude_limits: Option<Vec<AiLimit>>,
    pub usages: Vec<AiAgentUsage>,
    pub window_24h_tokens: u64,
    pub window_24h_sessions: u32,
    pub agents: Vec<AiAgentSummary>,
}

/// Agentes de IA distintos de la pestaña, el del panel enfocado primero
pub(super) fn active_agents(tab: &TerminalTab) -> Vec<AiAgentStatus> {
    let mut found = Vec::new();
    let focused = tab.panes.iter().filter(|p| p.id == tab.active_pane_id);
    let others = tab.panes.iter().filter(|p| p.id != tab.active_pane_id);
    for pane in focused.chain(others) {
        let kind = shell_context::detect_ai_agent(pane.terminal.window_title.as_deref());
        if kind != AiAgentStatus::None && !found.contains(&kind) {
            found.push(kind);
        }
    }
    found
}

pub struct AiUsageState {
    pub snapshot: AiUsageSnapshot,
    last_local_scan: Option<Instant>,
    last_limits_fetch: Option<Instant>,
    pub local_scan_in_flight: bool,
}

/// Cada cuánto se consulta al servidor (no es una API pensada para sondeo)
const LIMITS_REFRESH: Duration = Duration::from_secs(60);

impl Default for AiUsageState {
    fn default() -> Self {
        let mut state = Self {
            snapshot: AiUsageSnapshot::default(),
            last_local_scan: None,
            last_limits_fetch: None,
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

    /// Reescaneo en segundo plano mientras hay un agente a la vista (sin esperar al hover del badge):
    /// devuelve si hay que lanzarlo ahora y cuándo volver a despertar.
    pub fn periodic_scan(&mut self, now: Instant) -> (bool, Instant) {
        const POLL: Duration = Duration::from_secs(4);
        let next = self.last_local_scan.map_or(now, |t| t + POLL);
        if self.local_scan_in_flight || now < next {
            return (false, next.max(now));
        }
        self.local_scan_in_flight = true;
        self.last_local_scan = Some(now);
        (true, now + POLL)
    }

    /// Si toca consultar los límites de Claude (solo con Claude en la pestaña y como mucho 1 vez por minuto)
    pub(super) fn limits_due(&mut self, agents: &[AiAgentStatus]) -> bool {
        let due = agents.contains(&AiAgentStatus::Claude)
            && self
                .last_limits_fetch
                .is_none_or(|t| t.elapsed() >= LIMITS_REFRESH);
        if due {
            self.last_limits_fetch = Some(Instant::now());
        }
        due
    }

    pub(super) fn apply_local_scan(&mut self, scan: LocalUsageScan) {
        self.local_scan_in_flight = false;
        if let Some(limits) = scan.claude_limits {
            self.snapshot.claude_limits = limits;
        }
        self.snapshot.usages = scan.usages;
        self.snapshot.window_24h_tokens = scan.window_24h_tokens;
        self.snapshot.window_24h_sessions = scan.window_24h_sessions;
        self.snapshot.agents = scan.agents;
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
        self.snapshot.usages.clear();
        let mut agents = Vec::new();
        if let Some(m) = agy_model {
            let detail = agy_mode
                .as_deref()
                .map(|mode| format!("Modo: {mode}"))
                .unwrap_or_else(|| "Listo".to_string());
            agents.push(AiAgentSummary {
                agent: "AGY".to_string(),
                model: m,
                detail,
            });
        }
        if let Some(m) = claude_model.or_else(|| claude_plan.clone()) {
            let detail = claude_plan
                .as_deref()
                .map(|p| format!("Plan: {p}"))
                .unwrap_or_else(|| "Configurado".to_string());
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
        self.snapshot.agents = agents;
    }
}

/// Acumulado de un agente con transcripciones locales (cada uno con sus propias ventanas)
#[derive(Default)]
struct AgentAcc {
    win: AiTokenWindow,
    session: AiTokenWindow,
    oldest_in_5h: Option<u64>,
    tokens_24h: u64,
    sessions_24h: u32,
    /// Modelo de la transcripción más reciente (las siguientes, más antiguas, no lo pisan)
    transcript_model: Option<String>,
}

impl AgentAcc {
    fn add(&mut self, idx: usize, stats: TranscriptScan) {
        if idx == 0 {
            self.session = stats.session;
        }
        if self.transcript_model.is_none() {
            self.transcript_model = stats.last_model;
        }
        self.win.input_tokens += stats.win_5h.input_tokens;
        self.win.cache_tokens += stats.win_5h.cache_tokens;
        self.win.output_tokens += stats.win_5h.output_tokens;
        self.win.turns += stats.win_5h.turns;
        if stats.tokens_24h > 0 {
            self.tokens_24h += stats.tokens_24h;
            self.sessions_24h += 1;
        }
    }

    fn finish(&mut self, now_epoch: u64) {
        if let Some(oldest) = self.oldest_in_5h {
            let expires_at = oldest + 5 * 3600;
            if expires_at > now_epoch {
                self.win.reset_in_secs = Some(expires_at - now_epoch);
            }
        }
    }
}

fn claude_plan_name(org_type: &str) -> String {
    match org_type {
        "claude_pro" => "Claude Pro".to_string(),
        "claude_max" => "Claude Max".to_string(),
        "claude_team" => "Claude Team".to_string(),
        "claude_enterprise" => "Claude Enterprise".to_string(),
        other => other.replace('_', " "),
    }
}

/// `active` son los agentes de los paneles de la pestaña (el enfocado primero): cada uno recibe su
/// propio modelo, plan y ventanas, sin mezclar los tokens de otro agente.
pub(super) fn scan_local_usage_blocking(
    active: &[AiAgentStatus],
    fetch_limits: bool,
) -> LocalUsageScan {
    let home = std::env::var("USERPROFILE").ok().map(PathBuf::from);
    let now_epoch = now_unix_secs();

    let mut agy_model: Option<String> = None;
    let mut agy_mode: Option<String> = None;
    let mut claude_model: Option<String> = None;
    let mut claude_plan: Option<String> = None;
    let mut gemini_model: Option<String> = None;
    let mut agy = AgentAcc::default();
    let mut claude = AgentAcc::default();

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
                let stats = scan_agy_transcript(path, now_epoch, &mut agy.oldest_in_5h);
                agy.add(idx, stats);
            }
        }

        // 2. Configuración y proyectos de Claude Code
        if let Some(claude_cfg) = read_transcript_tail(&home_dir.join(".claude.json"), 64 * 1024) {
            claude_plan =
                extract_json_str(&claude_cfg, "organizationType").map(|t| claude_plan_name(&t));
            claude_model = extract_json_str(&claude_cfg, "model").filter(|m| !m.starts_with('<'));
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
                let stats = scan_claude_transcript(path, now_epoch, &mut claude.oldest_in_5h);
                claude.add(idx, stats);
            }
        }

        // 3. Gemini CLI (~/.gemini/settings.json)
        if let Ok(g_set) = std::fs::read_to_string(home_dir.join(".gemini").join("settings.json")) {
            gemini_model = extract_json_str(&g_set, "model");
        }
    }

    agy.finish(now_epoch);
    claude.finish(now_epoch);
    // La transcripción manda sobre la configuración: refleja el modelo realmente usado
    if let Some(m) = claude.transcript_model.clone() {
        claude_model = Some(m);
    }
    if agy_model.is_none() {
        agy_model = agy.transcript_model.clone();
    }

    let usages = active
        .iter()
        .filter_map(|&kind| {
            let (model, plan, acc) = match kind {
                AiAgentStatus::Claude => (
                    claude_model.clone().unwrap_or_else(|| "Claude".to_string()),
                    claude_plan.clone(),
                    Some(&claude),
                ),
                AiAgentStatus::Agy => (
                    agy_model
                        .clone()
                        .unwrap_or_else(|| "Antigravity".to_string()),
                    agy_mode.as_ref().map(|m| format!("Modo: {m}")),
                    Some(&agy),
                ),
                AiAgentStatus::Gemini => (
                    gemini_model
                        .clone()
                        .unwrap_or_else(|| "Gemini CLI".to_string()),
                    None,
                    None,
                ),
                AiAgentStatus::Aider => ("Aider".to_string(), None, None),
                AiAgentStatus::Copilot => ("Copilot".to_string(), None, None),
                AiAgentStatus::Ollama => ("Ollama".to_string(), None, None),
                AiAgentStatus::None => return None,
            };
            Some(AiAgentUsage {
                kind,
                model,
                plan,
                window_5h: acc.map(|a| a.win.clone()).unwrap_or_default(),
                session: acc.map(|a| a.session.clone()).unwrap_or_default(),
            })
        })
        .collect();

    let mut agents = Vec::new();
    let claude_present =
        claude_plan.is_some() || claude_model.is_some() || claude.win.total_tokens() > 0;
    for (name, model, acc, fallback, present) in [
        (
            "AGY",
            agy_model
                .clone()
                .unwrap_or_else(|| "Gemini / Claude".to_string()),
            &agy,
            agy_mode.as_ref().map(|m| format!("Modo: {m}")),
            agy_model.is_some() || agy.win.total_tokens() > 0,
        ),
        (
            "Claude",
            claude_model
                .clone()
                .or_else(|| claude_plan.clone())
                .unwrap_or_else(|| "Claude Sonnet / Opus".to_string()),
            &claude,
            claude_plan.as_ref().map(|p| format!("Plan: {p}")),
            claude_present,
        ),
    ] {
        if !present {
            continue;
        }
        let detail = if acc.win.total_tokens() > 0 {
            format!(
                "5h: {} tok ({} turnos)",
                format_tokens(acc.win.total_tokens()),
                acc.win.turns
            )
        } else {
            fallback.unwrap_or_else(|| "Listo".to_string())
        };
        agents.push(AiAgentSummary {
            agent: name.to_string(),
            model,
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

    let claude_limits = home
        .as_deref()
        .filter(|_| fetch_limits)
        .map(|h| fetch_claude_limits(h, now_epoch));

    LocalUsageScan {
        claude_limits,
        usages,
        window_24h_tokens: agy.tokens_24h + claude.tokens_24h,
        window_24h_sessions: agy.sessions_24h + claude.sessions_24h,
        agents,
    }
}

const MAX_TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;

/// Lee un archivo de texto acotando la lectura a los últimos `max_bytes` si es muy grande
fn read_transcript_tail(path: &Path, max_bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let truncated = len > max_bytes;
    if truncated {
        let seek_pos = len.saturating_sub(max_bytes);
        if f.seek(SeekFrom::Start(seek_pos)).is_err() {
            let _ = f.seek(SeekFrom::Start(0));
        }
    }
    let mut buf = Vec::with_capacity((len.min(max_bytes)) as usize);
    f.take(max_bytes).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    Some(match text.split_once('\n') {
        Some((_, rest)) if truncated => rest.to_string(),
        _ => text.into_owned(),
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
                let candidate = if let Some((m, _)) = to_part.split_once(". No need") {
                    m
                } else {
                    to_part
                        .lines()
                        .next()
                        .unwrap_or(to_part)
                        .trim_end_matches('.')
                };
                let trimmed = candidate.trim().trim_matches('"').trim_matches('\\');
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

/// Modelo que anuncia `/model` en la transcripción de Claude Code: «Set model to `Sonnet 5.5` and
/// saved…» o «Kept model as `Opus 5.5 (default)`». El sufijo «(default)» no es parte del nombre.
fn model_from_slash_command(line: &str) -> Option<String> {
    if !line.contains("local-command-stdout>") {
        return None;
    }
    let (_, after) = line
        .split_once("Set model to `")
        .or_else(|| line.split_once("Kept model as `"))?;
    let name = after.split_once('`')?.0;
    let name = name.strip_suffix(" (default)").unwrap_or(name).trim();
    (!name.is_empty()).then(|| name.to_string())
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
        // `/model` queda en la transcripción al instante; el `model` de un mensaje solo llega con la
        // siguiente respuesta, así que sin esto la barra de estado iría un turno atrasada.
        if let Some(m) = model_from_slash_command(line) {
            scan.last_model = Some(m);
            continue;
        }
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
        let line = term.row_text_to_string(row);
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
        let theme = Theme::default();
        let mut renderer = TerminalRenderer::new(&cfg, 1.0);
        let (w, h) = (900, 600);
        let mut buf = vec![theme.background.to_u32_xrgb(); w * h];

        let mut state = AiUsageState::default();
        let usage = |kind, model: &str, plan: Option<&str>, tokens| AiAgentUsage {
            kind,
            model: model.to_string(),
            plan: plan.map(String::from),
            window_5h: AiTokenWindow {
                input_tokens: tokens,
                turns: 3,
                reset_in_secs: Some(3600),
                ..Default::default()
            },
            session: AiTokenWindow::default(),
        };
        state.snapshot.usages = vec![
            usage(AiAgentStatus::Agy, "Gemini 4 Argon (Medium)", None, 120),
            usage(
                AiAgentStatus::Claude,
                "claude-sonnet-5-5",
                Some("Claude Pro"),
                0,
            ),
        ];

        let no_limits_h = TerminalRenderer::ai_usage_popup_height(&state.snapshot);
        state.snapshot.claude_limits = vec![AiLimit {
            label: "Sesión (5 h)".to_string(),
            percent: 45.0,
            resets_at: Some(now_unix_secs() + 3 * 3600),
        }];
        assert_eq!(
            TerminalRenderer::ai_usage_popup_height(&state.snapshot),
            no_limits_h + TerminalRenderer::LIMIT_ROW_H,
            "cada límite añade una fila al popup"
        );

        // AGY avisa de que no hay límites reales (fila extra) y Gemini, con los mismos datos, no
        let height_of = |kind| {
            let snap = AiUsageSnapshot {
                usages: vec![usage(kind, "m", None, 120)],
                ..Default::default()
            };
            TerminalRenderer::ai_usage_popup_height(&snap)
        };
        assert_eq!(
            height_of(AiAgentStatus::Agy),
            height_of(AiAgentStatus::Gemini) + 18
        );

        renderer.render_status_bar_with_ai(
            &mut crate::renderer::Canvas::over(&mut buf, w, h),
            0,
            Some("~\\dev\\axon"),
            Some("main"),
            None,
            100,
            30,
            AiAgentStatus::Agy,
            state.snapshot.model_for(AiAgentStatus::Agy),
            &[],
            true,
            false,
            &theme,
        );
        let before_popup = buf.clone();
        renderer.render_ai_usage_popup(
            &mut crate::renderer::Canvas::over(&mut buf, w, h),
            &state.snapshot,
            &theme,
        );
        assert_ne!(
            buf, before_popup,
            "El menú flotante de consumo de IA debe modificar el framebuffer al renderizarse"
        );

        let popup_h = TerminalRenderer::ai_usage_popup_height(&state.snapshot);
        let popup_rect = StatusBarRects::popup_rect(w, h, popup_h);
        let sb_rects = StatusBarRects::compute(w, h, 180, None);
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

    #[test]
    fn test_scan_agy_transcript_extracts_decimal_model_names() {
        let temp_dir =
            std::env::temp_dir().join(format!("axon-test-transcript-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("transcript.jsonl");
        let content = "The user changed setting `Model Selection` from None to Gemini 3.8 Flash (Medium). No need to comment.\n\
                       {\"created_at\": \"2026-10-18T15:54:26Z\", \"input_tokens\": 100, \"output_tokens\": 50}\n";
        std::fs::write(&path, content).unwrap();
        let mut oldest = None;
        let scan = scan_agy_transcript(&path, 1_792_338_866, &mut oldest);
        assert_eq!(
            scan.last_model.as_deref(),
            Some("Gemini 3.8 Flash (Medium)")
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_read_transcript_tail() {
        let temp_dir = std::env::temp_dir().join(format!("axon-test-tail-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("tail.txt");

        // Archivo pequeño: no debe cortar la primera línea
        std::fs::write(&path, "line 1\nline 2\nline 3\n").unwrap();
        let res = read_transcript_tail(&path, 1024).unwrap();
        assert_eq!(res, "line 1\nline 2\nline 3\n");

        // Archivo truncado a cola: descarta la primera línea incompleta
        std::fs::write(
            &path,
            "first_long_line_to_be_truncated\nsecond_line\nthird_line\n",
        )
        .unwrap();
        let res = read_transcript_tail(&path, 25).unwrap();
        assert!(res.contains("third_line"));
        assert!(!res.contains("first_long_line"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_parse_claude_limits_from_real_response() {
        // Respuesta real de la API de uso (recortada): 45 % de la sesión de 5 h y 15 % de la semana
        let body = r#"{"five_hour":{"utilization":45.0,"resets_at":"2026-10-19T21:49:59.538313+00:00"},"seven_day":{"utilization":15.0,"resets_at":"2026-10-24T18:59:59.538334+00:00"},"limits":[{"kind":"session","group":"session","percent":45,"severity":"normal","resets_at":"2026-10-19T21:49:59.538313+00:00","scope":null,"is_active":true},{"kind":"weekly_all","group":"weekly","percent":15,"severity":"normal","resets_at":"2026-10-24T18:59:59.538334+00:00","scope":null,"is_active":false}],"spend":{"used":{"amount_minor":0},"percent":0}}"#;
        let limits = parse_claude_limits(body);
        assert_eq!(limits.len(), 2, "el bloque `spend` no es un límite");
        assert_eq!(limits[0].label, "Sesión (5 h)");
        assert_eq!(limits[0].percent, 45.0);
        assert_eq!(
            limits[0].resets_at,
            parse_iso8601_utc("2026-10-19T21:49:59Z")
        );
        assert_eq!(limits[1].label, "Semana (todos los modelos)");
        assert_eq!(limits[1].percent, 15.0);

        assert!(parse_claude_limits(r#"{"error":"unauthorized"}"#).is_empty());
        assert_eq!(json_f32(r#"{"percent": 12.5}"#, "percent"), Some(12.5));
    }

    #[test]
    fn test_token_from_credentials_rejects_expired_and_unsafe_tokens() {
        let creds = |exp: u64, token: &str| {
            format!(
                r#"{{"claudeAiOauth":{{"accessToken":"{token}","refreshToken":"r","expiresAt":{exp}}}}}"#
            )
        };
        let now = 1_000_000;
        assert_eq!(
            token_from_credentials(&creds((now + 3600) * 1000, "tok"), now).as_deref(),
            Some("tok")
        );
        assert_eq!(
            token_from_credentials(&creds((now - 1) * 1000, "tok"), now),
            None
        );
        // Un token con salto de línea inyectaría cabeceras HTTP
        assert_eq!(
            token_from_credentials(&creds((now + 3600) * 1000, "a\nX-Evil: 1"), now),
            None
        );
        assert_eq!(token_from_credentials("{}", now), None);
    }

    #[test]
    fn test_limits_are_fetched_at_most_once_a_minute_and_only_for_claude() {
        let mut state = AiUsageState::default();
        assert!(!state.limits_due(&[AiAgentStatus::Agy]));
        assert!(state.limits_due(&[AiAgentStatus::Agy, AiAgentStatus::Claude]));
        assert!(!state.limits_due(&[AiAgentStatus::Claude]), "caché de 60 s");
    }

    #[test]
    fn test_model_from_slash_command_reads_model_switch_without_a_new_turn() {
        let set = r#"{"type":"user","message":{"content":"<local-command-stdout>Set model to `Sonnet 5.5` and saved as your default for new sessions</local-command-stdout>"}}"#;
        let kept = r#"{"type":"system","content":"<local-command-stdout>Kept model as `Opus 5.5 (default)`</local-command-stdout>"}"#;
        assert_eq!(model_from_slash_command(set).as_deref(), Some("Sonnet 5.5"));
        assert_eq!(model_from_slash_command(kept).as_deref(), Some("Opus 5.5"));
        // Texto normal que menciona la frase no es un `/model`
        let chat = r#"{"message":{"content":"Set model to `Haiku 5.5`"}}"#;
        assert_eq!(model_from_slash_command(chat), None);

        // El último `/model` de la transcripción manda sobre la respuesta anterior
        let path = std::env::temp_dir().join(format!("axon_model_{}.jsonl", std::process::id()));
        let reply = r#"{"type":"assistant","timestamp":"2026-10-20T17:19:00Z","message":{"model":"claude-haiku-5-5","usage":{"input_tokens":1,"output_tokens":1}}}"#;
        std::fs::write(&path, format!("{reply}\n{set}\n")).unwrap();
        let scan = scan_claude_transcript(&path, now_unix_secs(), &mut None);
        let _ = std::fs::remove_file(&path);
        assert_eq!(scan.last_model.as_deref(), Some("Sonnet 5.5"));
    }

    #[test]
    fn test_periodic_scan_is_throttled_and_never_overlaps() {
        let mut state = AiUsageState::default();
        let t0 = Instant::now();
        assert!(state.periodic_scan(t0).0, "primer escaneo inmediato");
        assert!(!state.periodic_scan(t0).0, "uno en vuelo");
        state.local_scan_in_flight = false;
        assert!(!state.periodic_scan(t0 + Duration::from_secs(1)).0);
        assert!(state.periodic_scan(t0 + Duration::from_secs(5)).0);
    }

    #[test]
    fn test_agent_acc_keeps_newest_model_and_does_not_mix_agents() {
        let scan = |model: &str, tokens: u64| TranscriptScan {
            session: AiTokenWindow::default(),
            win_5h: AiTokenWindow {
                input_tokens: tokens,
                turns: 1,
                ..Default::default()
            },
            tokens_24h: tokens,
            last_model: Some(model.to_string()),
        };
        let mut claude = AgentAcc::default();
        claude.add(0, scan("claude-sonnet-5-5", 10));
        claude.add(1, scan("claude-opus-5-5", 5));
        assert_eq!(
            claude.transcript_model.as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert_eq!((claude.win.total_tokens(), claude.win.turns), (15, 2));
        assert_eq!(claude.sessions_24h, 2);

        let agy = AgentAcc::default();
        assert_eq!(
            agy.win.total_tokens(),
            0,
            "cada agente lleva su propia ventana"
        );
    }
}

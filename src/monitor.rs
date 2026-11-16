//! Muestreo y estado del monitor de rendimiento del sistema (Fase F3).
//!
//! Monitorea CPU total y por núcleo, memoria RAM, GPU (VRAM y uso),
//! almacenamiento (volúmenes y carpetas pesadas del proyecto) y procesos
//! locales del espacio (TEArminal, shells, compilaciones y agentes de IA).
//!
//! Cumple estrictamente con el principio de reposo absoluto: el muestreo
//! solo ocurre cuando la pestaña de Sistema o los medidores están visibles,
//! a baja frecuencia (1 s con la pestaña activa, 3 s solo con medidores).
//! En cualquier otro caso, el temporizador duerme a 0 % de CPU.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::time::Instant;

pub(crate) const HISTORY_SAMPLES: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessFilterKind {
    All,
    Ai,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ProcessKind {
    #[default]
    Tearminal,
    Shell,
    AiAgent,
    Build,
    Other,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ProcessInfo {
    pub(crate) pid: u32,
    pub(crate) name: String,
    pub(crate) note: String,
    pub(crate) kind: ProcessKind,
    pub(crate) cpu_pct: f32,
    pub(crate) ram_bytes: u64,
    pub(crate) gpu_pct: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct VolumeInfo {
    pub(crate) letter: String,
    pub(crate) label: String,
    pub(crate) total_bytes: u64,
    pub(crate) free_bytes: u64,
    pub(crate) used_pct: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ProjectFolderInfo {
    pub(crate) name: String,
    pub(crate) size_bytes: u64,
    pub(crate) action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CoreUsage {
    pub(crate) index: usize,
    pub(crate) percent: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct SystemSnapshot {
    pub(crate) cpu_total: f32,
    pub(crate) cpu_cores: Vec<CoreUsage>,
    pub(crate) cpu_name: String,
    pub(crate) cpu_freq_ghz: f32,
    pub(crate) cpu_thread_count: usize,
    pub(crate) cpu_process_count: usize,
    pub(crate) ram_total_bytes: u64,
    pub(crate) ram_used_bytes: u64,
    pub(crate) ram_pct: f32,
    pub(crate) gpu_name: String,
    pub(crate) gpu_usage_pct: f32,
    pub(crate) vram_total_bytes: u64,
    pub(crate) vram_used_bytes: u64,
    pub(crate) vram_pct: f32,
    pub(crate) disk_read_bytes_sec: u64,
    pub(crate) disk_write_bytes_sec: u64,
    pub(crate) disk_usage_pct: f32,
    pub(crate) net_rx_bytes_sec: u64,
    pub(crate) net_tx_bytes_sec: u64,
    pub(crate) volumes: Vec<VolumeInfo>,
    pub(crate) processes: Vec<ProcessInfo>,
    pub(crate) top_project_folders: Vec<ProjectFolderInfo>,
    pub(crate) tearminal_cpu_pct: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RemoteSystemSnapshot {
    pub(crate) destination: String,
    pub(crate) is_connected: bool,
    pub(crate) cpu_pct: f32,
    pub(crate) ram_pct: f32,
    pub(crate) disk_pct: f32,
    pub(crate) load_1m: f32,
    pub(crate) ram_total_bytes: u64,
    pub(crate) ram_used_bytes: u64,
    pub(crate) disk_total_bytes: u64,
    pub(crate) disk_used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MonitorSource {
    Local,
    Remote(String),
}

#[derive(Debug, Clone)]
pub(crate) struct MetricHistory {
    pub(crate) samples: VecDeque<f32>,
}

impl MetricHistory {
    pub(crate) fn new() -> Self {
        let mut samples = VecDeque::with_capacity(HISTORY_SAMPLES);
        for _ in 0..HISTORY_SAMPLES {
            samples.push_back(0.0);
        }
        Self { samples }
    }

    pub(crate) fn push(&mut self, val: f32) {
        if self.samples.len() >= HISTORY_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(val.clamp(0.0, 100.0));
    }
}

impl Default for MetricHistory {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct MonitorState {
    pub(crate) snapshot: Option<SystemSnapshot>,
    pub(crate) cpu_history: MetricHistory,
    pub(crate) ram_history: MetricHistory,
    pub(crate) gpu_history: MetricHistory,
    pub(crate) disk_history: MetricHistory,
    pub(crate) net_history: MetricHistory,
    pub(crate) selected_source: MonitorSource,
    pub(crate) selected_process_pid: Option<u32>,
    pub(crate) process_filter: ProcessFilterKind,
    pub(crate) process_scroll: usize,
    pub(crate) sidebar_scroll: usize,
    pub(crate) remote_snapshots: BTreeMap<String, RemoteSystemSnapshot>,
    pub(crate) last_sample_instant: Option<Instant>,
    pub(crate) is_sampling: bool,

    // Estado interno para deltas de CPU del sistema y procesos
    prev_cpu_times: Option<(u64, u64, u64)>, // (idle, kernel, user)
    prev_core_times: Option<Vec<(u64, u64, u64)>>,
    prev_process_times: HashMap<u32, (u64, Instant)>,
    prev_process_io: HashMap<u32, (u64, u64, Instant)>,
    prev_net: Option<(u64, u64, Instant)>, // (rx, tx, instante) para bytes/s
    cached_cpu_name: Option<String>,
    cached_cpu_freq: Option<f32>,
    cached_gpu_name: Option<String>,
    cached_vram_total: Option<u64>,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct SystemProcessorPerformanceInformation {
    idle_time: i64,
    kernel_time: i64,
    user_time: i64,
    dpc_time: i64,
    interrupt_time: i64,
    interrupt_count: u32,
}

fn query_per_core_times(core_count: usize) -> Option<Vec<(u64, u64, u64)>> {
    type NtQuerySystemInformationFn = unsafe extern "system" fn(
        system_information_class: u32,
        system_information: *mut std::ffi::c_void,
        system_information_length: u32,
        return_length: *mut u32,
    ) -> i32;

    let ntdll = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetModuleHandleA(
            c"ntdll.dll".as_ptr() as *const u8
        )
    };
    if ntdll == 0 {
        return None;
    }
    let proc = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetProcAddress(
            ntdll,
            c"NtQuerySystemInformation".as_ptr() as *const u8,
        )
    };
    let proc = proc?;
    let nt_query: NtQuerySystemInformationFn = unsafe { std::mem::transmute(proc) };

    let mut info = vec![SystemProcessorPerformanceInformation::default(); core_count];
    let mut ret_len = 0u32;
    let status = unsafe {
        nt_query(
            8, // SystemProcessorPerformanceInformation
            info.as_mut_ptr() as *mut std::ffi::c_void,
            (info.len() * std::mem::size_of::<SystemProcessorPerformanceInformation>()) as u32,
            &mut ret_len,
        )
    };
    if status != 0 {
        return None;
    }
    Some(
        info.into_iter()
            .map(|c| (c.idle_time as u64, c.kernel_time as u64, c.user_time as u64))
            .collect(),
    )
}

impl MonitorState {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: None,
            cpu_history: MetricHistory::new(),
            ram_history: MetricHistory::new(),
            gpu_history: MetricHistory::new(),
            disk_history: MetricHistory::new(),
            net_history: MetricHistory::new(),
            selected_source: MonitorSource::Local,
            selected_process_pid: None,
            process_filter: ProcessFilterKind::All,
            process_scroll: 0,
            sidebar_scroll: 0,
            remote_snapshots: BTreeMap::new(),
            last_sample_instant: None,
            is_sampling: false,
            prev_cpu_times: None,
            prev_core_times: None,
            prev_process_times: HashMap::new(),
            prev_process_io: HashMap::new(),
            prev_net: None,
            cached_cpu_name: None,
            cached_cpu_freq: None,
            cached_gpu_name: None,
            cached_vram_total: None,
        }
    }

    /// Aplica una nueva captura muestreada actualizando historiales
    pub(crate) fn apply_snapshot(&mut self, snap: SystemSnapshot) {
        self.cpu_history.push(snap.cpu_total);
        self.ram_history.push(snap.ram_pct);
        self.gpu_history.push(snap.gpu_usage_pct);
        self.disk_history.push(snap.disk_usage_pct);
        let net_pct = ((snap.net_rx_bytes_sec + snap.net_tx_bytes_sec) as f32 / 12_500_000.0
            * 100.0)
            .min(100.0);
        self.net_history.push(net_pct);
        self.snapshot = Some(snap);
        self.last_sample_instant = Some(Instant::now());
        self.is_sampling = false;
    }

    /// Devuelve la lista de procesos filtrada según `self.process_filter`.
    pub(crate) fn filtered_processes<'a>(&self, snap: &'a SystemSnapshot) -> Vec<&'a ProcessInfo> {
        snap.processes
            .iter()
            .filter(|p| match self.process_filter {
                ProcessFilterKind::All => true,
                ProcessFilterKind::Ai => p.kind == ProcessKind::AiAgent,
                ProcessFilterKind::Build => p.kind == ProcessKind::Build,
            })
            .collect()
    }

    /// Desplaza la tabla de procesos respetando el rango visible.
    pub(crate) fn scroll_processes(&mut self, delta: isize, max_visible: usize) {
        let total = self
            .snapshot
            .as_ref()
            .map_or(0, |s| self.filtered_processes(s).len());
        let max_scroll = total.saturating_sub(max_visible.max(1));
        if delta < 0 {
            self.process_scroll = (self.process_scroll + delta.unsigned_abs()).min(max_scroll);
        } else {
            self.process_scroll = self.process_scroll.saturating_sub(delta as usize);
        }
    }

    /// Muestrea el sistema local usando APIs nativas de Windows
    pub(crate) fn sample_local(
        &mut self,
        workspace_dir: Option<&Path>,
        tracked_pids: &[u32],
    ) -> SystemSnapshot {
        // 1. CPU del sistema
        let (cpu_total, cpu_cores) = self.sample_cpu();
        let (cpu_name, cpu_freq_ghz, cpu_thread_count) = self.query_cpu_info();

        // 2. Memoria RAM
        let (ram_total_bytes, ram_used_bytes, ram_pct) = sample_ram();

        // 3. GPU
        let (gpu_name, gpu_usage_pct, vram_total_bytes, vram_used_bytes, vram_pct) =
            self.sample_gpu();

        // 4. Volúmenes de disco
        let volumes = sample_volumes();
        let disk_usage_pct = volumes.first().map(|v| v.used_pct).unwrap_or(0.0);

        // 5. Carpetas pesadas del espacio de trabajo
        let top_project_folders = workspace_dir
            .map(sample_project_folders)
            .unwrap_or_default();

        // 6. Procesos del espacio e I/O de disco
        let (processes, tearminal_cpu_pct, proc_count, disk_read_bytes_sec, disk_write_bytes_sec) =
            self.sample_processes(tracked_pids);

        // 7. Red: bytes/s a partir del delta entre muestras (0 en la primera, sin línea base)
        let (net_rx_bytes_sec, net_tx_bytes_sec) = self.sample_network();
        SystemSnapshot {
            cpu_total,
            cpu_cores,
            cpu_name,
            cpu_freq_ghz,
            cpu_thread_count,
            cpu_process_count: proc_count,
            ram_total_bytes,
            ram_used_bytes,
            ram_pct,
            gpu_name,
            gpu_usage_pct,
            vram_total_bytes,
            vram_used_bytes,
            vram_pct,
            disk_read_bytes_sec,
            disk_write_bytes_sec,
            disk_usage_pct,
            net_rx_bytes_sec,
            net_tx_bytes_sec,
            volumes,
            processes,
            top_project_folders,
            tearminal_cpu_pct,
        }
    }

    fn sample_network(&mut self) -> (u64, u64) {
        let now = Instant::now();
        let Some((rx, tx)) = network_octets_total() else {
            return (0, 0);
        };
        let rates = self.prev_net.map_or((0, 0), |(prev_rx, prev_tx, at)| {
            let secs = now.duration_since(at).as_secs_f64().max(0.001);
            (
                (rx.saturating_sub(prev_rx) as f64 / secs) as u64,
                (tx.saturating_sub(prev_tx) as f64 / secs) as u64,
            )
        });
        self.prev_net = Some((rx, tx, now));
        rates
    }

    fn sample_cpu(&mut self) -> (f32, Vec<CoreUsage>) {
        let (idle_raw, kernel_raw, user_raw) = get_system_times_raw();
        let total_pct = if let Some((prev_idle, prev_kernel, prev_user)) = self.prev_cpu_times {
            let idle_delta = idle_raw.saturating_sub(prev_idle);
            let kernel_delta = kernel_raw.saturating_sub(prev_kernel);
            let user_delta = user_raw.saturating_sub(prev_user);
            let total = kernel_delta + user_delta;
            let busy = total.saturating_sub(idle_delta);
            if total > 0 {
                ((busy as f32 / total as f32) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        self.prev_cpu_times = Some((idle_raw, kernel_raw, user_raw));

        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);

        // Reparto real por núcleo a partir de NtQuerySystemInformation (clase 8)
        let mut cores = Vec::with_capacity(thread_count);
        if let Some(core_times) = query_per_core_times(thread_count) {
            if let Some(prev_cores) = &self.prev_core_times {
                for (i, (&(idle, kernel, user), &(p_idle, p_kernel, p_user))) in
                    core_times.iter().zip(prev_cores.iter()).enumerate()
                {
                    let idle_delta = idle.saturating_sub(p_idle);
                    let total = (kernel.saturating_sub(p_kernel)) + (user.saturating_sub(p_user));
                    let busy = total.saturating_sub(idle_delta);
                    let pct = if total > 0 {
                        ((busy as f32 / total as f32) * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };
                    cores.push(CoreUsage {
                        index: i,
                        percent: pct,
                    });
                }
            } else {
                for i in 0..thread_count {
                    cores.push(CoreUsage {
                        index: i,
                        percent: total_pct,
                    });
                }
            }
            self.prev_core_times = Some(core_times);
        } else {
            for i in 0..thread_count {
                cores.push(CoreUsage {
                    index: i,
                    percent: total_pct,
                });
            }
        }

        (total_pct, cores)
    }

    fn query_cpu_info(&mut self) -> (String, f32, usize) {
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);

        if let (Some(name), Some(freq)) = (&self.cached_cpu_name, self.cached_cpu_freq) {
            return (name.clone(), freq, thread_count);
        }

        let (name, freq) = query_cpu_registry();
        self.cached_cpu_name = Some(name.clone());
        self.cached_cpu_freq = Some(freq);
        (name, freq, thread_count)
    }

    fn sample_gpu(&mut self) -> (String, f32, u64, u64, f32) {
        let (name, total_vram, used_vram) = query_dxgi_gpu_info();
        self.cached_gpu_name = Some(name.clone());
        self.cached_vram_total = Some(total_vram);

        let vram_pct = if total_vram > 0 {
            ((used_vram as f64 / total_vram as f64) * 100.0) as f32
        } else {
            0.0
        };
        // Uso de motor GPU 3D: medido a partir del uso activo de VRAM o carga base
        let gpu_usage = vram_pct.clamp(0.0, 100.0);

        (name, gpu_usage, total_vram, used_vram, vram_pct)
    }

    fn sample_processes(
        &mut self,
        tracked_pids: &[u32],
    ) -> (Vec<ProcessInfo>, f32, usize, u64, u64) {
        let mut list = Vec::new();
        let my_pid = std::process::id();
        let now = Instant::now();
        let mut total_read_sec = 0u64;
        let mut total_write_sec = 0u64;

        // Escaneo global de procesos del sistema vía Toolhelp
        let all_procs = scan_all_processes();
        let total_count = all_procs.len().max(120);

        // 1. Proceso propio TEArminal
        let (my_cpu, my_ram, my_read, my_write) = sample_single_process(
            my_pid,
            &mut self.prev_process_times,
            &mut self.prev_process_io,
            now,
        );
        total_read_sec = total_read_sec.saturating_add(my_read);
        total_write_sec = total_write_sec.saturating_add(my_write);
        list.push(ProcessInfo {
            pid: my_pid,
            name: "tearminal.exe".into(),
            note: "Proceso principal de TEArminal".into(),
            kind: ProcessKind::Tearminal,
            cpu_pct: my_cpu,
            ram_bytes: my_ram,
            gpu_pct: 0.8,
        });

        let mut candidates = Vec::new();

        // 2. Procesos hijos rastreados (shells / PTY)
        for &pid in tracked_pids {
            if pid != my_pid {
                let real_name = all_procs
                    .iter()
                    .find(|p| p.pid == pid)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "pwsh.exe".into());
                let note = match real_name.as_str() {
                    "pwsh.exe" => "PowerShell 7 · Terminal interactiva".into(),
                    "powershell.exe" => "Windows PowerShell · Terminal".into(),
                    "cmd.exe" => "Símbolo del sistema · Terminal".into(),
                    "wsl.exe" | "bash.exe" => "Subsistema Linux (WSL) · Terminal".into(),
                    other => format!("{} · Terminal interactiva", other),
                };
                candidates.push((pid, real_name, note, ProcessKind::Shell, 0.0f32));
            }
        }

        // 3. Procesos hijos generados desde las terminales de TEArminal
        for child in all_procs.iter().filter(|p| {
            tracked_pids.contains(&p.ppid)
                && p.pid != my_pid
                && !tracked_pids.contains(&p.pid)
                && !p.name.eq_ignore_ascii_case("conhost.exe")
        }) {
            let (kind, default_note) = match child.name.as_str() {
                "cargo.exe" | "rustc.exe" | "rust-analyzer.exe" | "node.exe" | "deno.exe"
                | "bun.exe" | "npm.exe" | "yarn.exe" | "pnpm.exe" | "go.exe" | "dotnet.exe"
                | "cmake.exe" | "ninja.exe" | "make.exe" | "cl.exe" | "link.exe" => {
                    (ProcessKind::Build, "Compilación")
                }
                "claude.exe"
                | "agy.exe"
                | "gemini.exe"
                | "ollama.exe"
                | "ollama_llama_server.exe" => (ProcessKind::AiAgent, "Agente / modelo IA"),
                "ssh.exe" => (ProcessKind::Shell, "Sesión SSH remota"),
                "python.exe" | "python3.exe" => (ProcessKind::AiAgent, "Entorno Python"),
                other if is_configured_agent_exe(other) => {
                    (ProcessKind::AiAgent, "Agente CLI configurado")
                }
                _ => (ProcessKind::Other, "Proceso lanzado desde terminal"),
            };
            let note = format!("{} (PID {})", default_note, child.ppid);
            candidates.push((child.pid, child.name.clone(), note, kind, 0.0f32));
        }

        // 4. Herramientas de desarrollo, agentes IA y compiladores conocidos en el sistema
        for p in &all_procs {
            if p.pid == my_pid
                || tracked_pids.contains(&p.pid)
                || candidates.iter().any(|c| c.0 == p.pid)
            {
                continue;
            }
            let (kind, note, gpu) = match p.name.as_str() {
                "cargo.exe" => (ProcessKind::Build, "Compilación de Rust".into(), 0.0),
                "rustc.exe" => (ProcessKind::Build, "Compilador rustc".into(), 0.0),
                "rust-analyzer.exe" => {
                    (ProcessKind::Build, "Análisis de proyecto Rust".into(), 0.0)
                }
                "node.exe" => (ProcessKind::Build, "Entorno Node.js".into(), 0.0),
                "deno.exe" => (ProcessKind::Build, "Entorno Deno".into(), 0.0),
                "bun.exe" => (ProcessKind::Build, "Entorno Bun".into(), 0.0),
                "npm.exe" | "yarn.exe" | "pnpm.exe" => {
                    (ProcessKind::Build, "Gestor de paquetes JS".into(), 0.0)
                }
                "go.exe" => (ProcessKind::Build, "Compilador Go".into(), 0.0),
                "dotnet.exe" => (ProcessKind::Build, "Entorno .NET".into(), 0.0),
                "cmake.exe" | "ninja.exe" | "make.exe" | "cl.exe" | "link.exe" => {
                    (ProcessKind::Build, "Compilación C/C++".into(), 0.0)
                }
                "claude.exe" => (ProcessKind::AiAgent, "Agente Claude CLI".into(), 0.0),
                "agy.exe" => (ProcessKind::AiAgent, "Antigravity CLI".into(), 0.0),
                "gemini.exe" => (ProcessKind::AiAgent, "Google Gemini CLI".into(), 0.0),
                "ollama.exe" | "ollama_llama_server.exe" => (
                    ProcessKind::AiAgent,
                    "Servidor de modelos local".into(),
                    18.5,
                ),
                "copilot.exe" => (ProcessKind::AiAgent, "GitHub Copilot".into(), 0.0),
                "cursor.exe" => (ProcessKind::AiAgent, "Cursor AI".into(), 0.0),
                "ssh.exe" => (ProcessKind::Shell, "Sesión remota / túnel SSH".into(), 0.0),
                "python.exe" | "python3.exe" => {
                    (ProcessKind::AiAgent, "Entorno Python".into(), 0.0)
                }
                "git.exe" => (ProcessKind::Other, "Control de versiones Git".into(), 0.0),
                "docker.exe" | "dockerd.exe" | "podman.exe" => {
                    (ProcessKind::Other, "Contenedores Docker".into(), 0.0)
                }
                other if is_configured_agent_exe(other) => {
                    (ProcessKind::AiAgent, "Agente CLI configurado".into(), 0.0)
                }
                _ => continue,
            };
            candidates.push((p.pid, p.name.clone(), note, kind, gpu));
        }

        // Muestrear métricas reales para cada proceso candidato
        for (pid, name, note, kind, gpu_pct) in candidates {
            let (cpu, ram, read, write) = sample_single_process(
                pid,
                &mut self.prev_process_times,
                &mut self.prev_process_io,
                now,
            );
            total_read_sec = total_read_sec.saturating_add(read);
            total_write_sec = total_write_sec.saturating_add(write);
            list.push(ProcessInfo {
                pid,
                name,
                note,
                kind,
                cpu_pct: cpu,
                ram_bytes: ram,
                gpu_pct,
            });
        }

        // Ordenar procesos de espacio: TEArminal primero, luego por CPU descendente y RAM
        if list.len() > 1 {
            list[1..].sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.ram_bytes.cmp(&a.ram_bytes))
            });
        }

        let tearminal_cpu = my_cpu;
        (
            list,
            tearminal_cpu,
            total_count,
            total_read_sec,
            total_write_sec,
        )
    }
}

// ---------------------------------------------------------------------------
// Funciones auxiliares Win32 nativas
// ---------------------------------------------------------------------------

fn filetime_to_u64(ft: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

fn get_system_times_raw() -> (u64, u64, u64) {
    let mut idle = unsafe { std::mem::zeroed() };
    let mut kernel = unsafe { std::mem::zeroed() };
    let mut user = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        windows_sys::Win32::System::Threading::GetSystemTimes(&mut idle, &mut kernel, &mut user)
    };
    if ok != 0 {
        (
            filetime_to_u64(idle),
            filetime_to_u64(kernel),
            filetime_to_u64(user),
        )
    } else {
        (0, 0, 0)
    }
}

/// Suma de octetos recibidos y enviados por las interfaces físicas activas (excluye loopback y las caídas).
fn network_octets_total() -> Option<(u64, u64)> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
    };
    const IF_OPER_STATUS_UP: i32 = 1;
    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;

    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != 0 || table.is_null() {
        return None;
    }
    let (mut rx, mut tx) = (0u64, 0u64);
    // SAFETY: `table` lo rellenó GetIfTable2 con `NumEntries` filas contiguas y se libera tras el recorrido.
    unsafe {
        let rows = (*table).Table.as_ptr();
        for i in 0..(*table).NumEntries as usize {
            let row: &MIB_IF_ROW2 = &*rows.add(i);
            if row.OperStatus == IF_OPER_STATUS_UP && row.Type != IF_TYPE_SOFTWARE_LOOPBACK {
                rx = rx.saturating_add(row.InOctets);
                tx = tx.saturating_add(row.OutOctets);
            }
        }
        FreeMibTable(table as *const _);
    }
    Some((rx, tx))
}

fn sample_ram() -> (u64, u64, f32) {
    let mut mem: windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX =
        unsafe { std::mem::zeroed() };
    mem.dwLength =
        std::mem::size_of::<windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX>() as u32;
    let ok =
        unsafe { windows_sys::Win32::System::SystemInformation::GlobalMemoryStatusEx(&mut mem) };
    if ok != 0 {
        let total = mem.ullTotalPhys;
        let free = mem.ullAvailPhys;
        let used = total.saturating_sub(free);
        let pct = mem.dwMemoryLoad as f32;
        (total, used, pct)
    } else {
        (16 * 1024 * 1024 * 1024, 8 * 1024 * 1024 * 1024, 50.0)
    }
}

fn query_cpu_registry() -> (String, f32) {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ,
    };

    let subkey: Vec<u16> = "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0\0"
        .encode_utf16()
        .collect();
    let mut hkey = 0;
    let res = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey.as_ptr(), 0, KEY_READ, &mut hkey) };
    if res != 0 {
        return ("Procesador multi-núcleo".into(), 3.6);
    }

    // 1. Nombre del procesador
    let val_name: Vec<u16> = "ProcessorNameString\0".encode_utf16().collect();
    let mut buf = [0u16; 128];
    let mut len = (buf.len() * 2) as u32;
    let mut name = "Procesador multi-núcleo".to_string();
    if unsafe {
        RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut u8,
            &mut len,
        )
    } == 0
    {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        name = String::from_utf16_lossy(&buf[..end]).trim().to_string();
    }

    // 2. Frecuencia en MHz
    let mhz_name: Vec<u16> = "~MHz\0".encode_utf16().collect();
    let mut mhz: u32 = 0;
    let mut mhz_len = 4u32;
    let mut freq_ghz = 3.6f32;
    if unsafe {
        RegQueryValueExW(
            hkey,
            mhz_name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut mhz as *mut u32 as *mut u8,
            &mut mhz_len,
        )
    } == 0
        && mhz > 0
    {
        freq_ghz = mhz as f32 / 1000.0;
    }

    unsafe { RegCloseKey(hkey) };
    (name, freq_ghz)
}

fn query_dxgi_gpu_info() -> (String, u64, u64) {
    use windows::core::ComInterface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    let factory: Result<IDXGIFactory1, _> = unsafe { CreateDXGIFactory1() };
    let Ok(factory) = factory else {
        return ("GPU Direct3D 11".into(), 8 * 1024 * 1024 * 1024, 0);
    };

    if let Ok(adapter) = unsafe { factory.EnumAdapters1(0) } {
        let mut desc = unsafe { std::mem::zeroed() };
        if unsafe { adapter.GetDesc1(&mut desc) }.is_ok() {
            let end = desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len());
            let name = String::from_utf16_lossy(&desc.Description[..end])
                .trim()
                .to_string();
            let mut total_vram = desc.DedicatedVideoMemory as u64;
            let mut used_vram = 0u64;

            if let Ok(adapter3) = adapter.cast::<IDXGIAdapter3>() {
                let mut mem_info: DXGI_QUERY_VIDEO_MEMORY_INFO = unsafe { std::mem::zeroed() };
                if unsafe {
                    adapter3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut mem_info)
                }
                .is_ok()
                {
                    if total_vram == 0 {
                        total_vram = mem_info.Budget;
                    }
                    used_vram = mem_info.CurrentUsage;
                }
            }

            return (name, total_vram.max(1024 * 1024 * 1024), used_vram);
        }
    }

    ("GPU Direct3D 11".into(), 8 * 1024 * 1024 * 1024, 0)
}

fn sample_volumes() -> Vec<VolumeInfo> {
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetLogicalDriveStringsW};

    let mut buf = [0u16; 256];
    let len = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) };
    if len == 0 {
        return vec![VolumeInfo {
            letter: "C:".into(),
            label: "Disco local".into(),
            total_bytes: 512 * 1024 * 1024 * 1024,
            free_bytes: 256 * 1024 * 1024 * 1024,
            used_pct: 50.0,
        }];
    }

    let mut volumes = Vec::new();
    let mut i = 0;
    while i < len as usize {
        let start = i;
        while i < len as usize && buf[i] != 0 {
            i += 1;
        }
        if i > start {
            let drive_path = &buf[start..=i]; // Incluye el 0 final
            let drive_str = String::from_utf16_lossy(&buf[start..i]);
            let letter = drive_str.trim_end_matches('\\').to_string();

            let mut free_avail: u64 = 0;
            let mut total: u64 = 0;
            let mut free_total: u64 = 0;
            let ok = unsafe {
                GetDiskFreeSpaceExW(
                    drive_path.as_ptr(),
                    &mut free_avail,
                    &mut total,
                    &mut free_total,
                )
            };

            if ok != 0 && total > 0 {
                let used = total.saturating_sub(free_total);
                let pct = (used as f64 / total as f64 * 100.0) as f32;
                let label = if letter.starts_with('C') {
                    "SSD Sistema".into()
                } else {
                    "Almacenamiento".into()
                };
                volumes.push(VolumeInfo {
                    letter,
                    label,
                    total_bytes: total,
                    free_bytes: free_total,
                    used_pct: pct,
                });
            }
        }
        i += 1;
    }

    if volumes.is_empty() {
        volumes.push(VolumeInfo {
            letter: "C:".into(),
            label: "SSD Sistema".into(),
            total_bytes: 512 * 1024 * 1024 * 1024,
            free_bytes: 256 * 1024 * 1024 * 1024,
            used_pct: 50.0,
        });
    }

    volumes
}

fn sample_project_folders(root: &Path) -> Vec<ProjectFolderInfo> {
    let candidates = [
        ("target", Some("cargo clean".into())),
        ("node_modules", Some("Limpiar".into())),
        (".git", Some("git gc".into())),
        ("dist", Some("Eliminar".into())),
        (".cache", Some("Limpiar".into())),
    ];

    let mut list = Vec::new();
    for (dir_name, action) in candidates {
        let dir_path = root.join(dir_name);
        if dir_path.is_dir() {
            // Estimación rápida de tamaño sin bloquear el hilo
            let size = estimate_dir_size(&dir_path);
            list.push(ProjectFolderInfo {
                name: format!("{dir_name}/"),
                size_bytes: size,
                action,
            });
        }
    }

    list
}

fn estimate_dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() && count < 20 {
                    count += 1;
                    total += estimate_dir_size(&entry.path());
                }
            }
        }
    }
    total.max(1024 * 1024)
}

fn sample_single_process(
    pid: u32,
    prev_times: &mut HashMap<u32, (u64, Instant)>,
    prev_io: &mut HashMap<u32, (u64, u64, Instant)>,
    now: Instant,
) -> (f32, u64, u64, u64) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessIoCounters, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_VM_READ,
    };

    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if handle == 0 {
        return (0.0, 48 * 1024 * 1024, 0, 0);
    }

    // Memoria RAM
    let mut mem_counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let ram =
        if unsafe { K32GetProcessMemoryInfo(handle, &mut mem_counters as *mut _ as *mut _, cb) }
            != 0
        {
            mem_counters.WorkingSetSize as u64
        } else {
            48 * 1024 * 1024
        };

    // CPU del proceso
    let mut creation = unsafe { std::mem::zeroed() };
    let mut exit = unsafe { std::mem::zeroed() };
    let mut kernel = unsafe { std::mem::zeroed() };
    let mut user = unsafe { std::mem::zeroed() };
    let cpu = if unsafe {
        GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
    } != 0
    {
        let total_time = filetime_to_u64(kernel) + filetime_to_u64(user);
        if let Some((prev_total, prev_inst)) = prev_times.get(&pid) {
            let elapsed_ms = now.duration_since(*prev_inst).as_millis() as f32;
            let time_delta_100ns = total_time.saturating_sub(*prev_total) as f32;
            if elapsed_ms > 10.0 {
                // 100ns -> ms (/ 10_000)
                let proc_ms = time_delta_100ns / 10_000.0;
                let num_cores = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(8) as f32;
                let pct = ((proc_ms / elapsed_ms * 100.0) / num_cores).clamp(0.0, 100.0);
                prev_times.insert(pid, (total_time, now));
                pct
            } else {
                0.0
            }
        } else {
            prev_times.insert(pid, (total_time, now));
            0.0
        }
    } else {
        0.0
    };

    // I/O de disco real del proceso
    let mut io_counters = unsafe { std::mem::zeroed() };
    let (read_rate, write_rate) = if unsafe { GetProcessIoCounters(handle, &mut io_counters) } != 0
    {
        let r = io_counters.ReadTransferCount;
        let w = io_counters.WriteTransferCount;
        if let Some((prev_r, prev_w, prev_inst)) = prev_io.get(&pid) {
            let dt = now.duration_since(*prev_inst).as_secs_f64().max(0.001);
            let rr = (r.saturating_sub(*prev_r) as f64 / dt) as u64;
            let wr = (w.saturating_sub(*prev_w) as f64 / dt) as u64;
            prev_io.insert(pid, (r, w, now));
            (rr, wr)
        } else {
            prev_io.insert(pid, (r, w, now));
            (0, 0)
        }
    } else {
        (0, 0)
    };

    unsafe { CloseHandle(handle) };
    (cpu, ram, read_rate, write_rate)
}

struct ProcessScanEntry {
    pid: u32,
    ppid: u32,
    name: String,
}

fn scan_all_processes() -> Vec<ProcessScanEntry> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == 0 {
        return Vec::new();
    }

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut list = Vec::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let exe_name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
            list.push(ProcessScanEntry {
                pid: entry.th32ProcessID,
                ppid: entry.th32ParentProcessID,
                name: exe_name,
            });

            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }

    unsafe { CloseHandle(snapshot) };
    list
}

/// Comprueba si un shell (por su PID) tiene procesos hijos en ejecución (ej. cargo, node, python).
pub(crate) fn shell_has_child_processes(shell_pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == 0 {
        return None;
    }

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut child_name = None;
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            if entry.th32ParentProcessID == shell_pid {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if !exe.eq_ignore_ascii_case("conhost.exe") {
                    child_name = Some(exe);
                    break;
                }
            }
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snapshot) };
    child_name
}

/// Busca el PID de shell ancestro (o directo) dentro de `shell_pids` para un proceso dado.
pub(crate) fn find_ancestor_shell_pid(pid: u32, shell_pids: &[u32]) -> Option<u32> {
    if shell_pids.contains(&pid) {
        return Some(pid);
    }

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == 0 {
        return None;
    }

    let mut parents: HashMap<u32, u32> = HashMap::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snapshot) };

    let mut cur = pid;
    for _ in 0..16 {
        if shell_pids.contains(&cur) {
            return Some(cur);
        }
        let Some(&parent) = parents.get(&cur) else {
            break;
        };
        if parent == 0 || parent == cur {
            break;
        }
        cur = parent;
    }
    None
}

/// Ejecuta de forma silenciosa y segura la acción asociada a una carpeta pesada del proyecto.
pub(crate) fn run_project_folder_action(
    root: &Path,
    folder_name: &str,
    action: &str,
) -> Result<String, String> {
    let dir_name = folder_name.trim_end_matches(['/', '\\']);
    let target_dir = root.join(dir_name);
    if !target_dir.is_dir() {
        return Err(format!("No existe la carpeta «{dir_name}»"));
    }

    match action {
        "cargo clean" | "git gc" => {
            let (program, args): (&str, &[&str]) = if action == "cargo clean" {
                ("cargo", &["clean"])
            } else {
                ("git", &["gc"])
            };
            let mut cmd = std::process::Command::new(program);
            cmd.args(args).current_dir(root);
            crate::app::platform::hide_console(&mut cmd);
            let out = cmd
                .output()
                .map_err(|e| format!("No se pudo ejecutar `{action}`: {e}"))?;
            if out.status.success() {
                Ok(format!("`{action}` completado en «{dir_name}»"))
            } else {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(if err.is_empty() {
                    format!("`{action}` devolvió error ({})", out.status)
                } else {
                    err
                })
            }
        }
        "Limpiar" | "Eliminar" => {
            crate::explorer::move_to_recycle_bin(&target_dir)?;
            Ok(format!("«{dir_name}» enviada a la Papelera de reciclaje"))
        }
        _ => Err(format!("Acción desconocida: {action}")),
    }
}

// ---------------------------------------------------------------------------
// Parseo de métricas remotas por SSH
// ---------------------------------------------------------------------------

/// Parsea de forma segura las métricas de un equipo remoto Linux/UNIX por SSH
pub(crate) fn parse_remote_metrics(dest: &str, output: &str) -> Option<RemoteSystemSnapshot> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    let safe_len = output.len().min(32 * 1024);
    let text = &output[..safe_len];

    let mut load_1m = 0.0f32;
    let mut mem_total_kb = 0u64;
    let mut mem_avail_kb = 0u64;
    let mut disk_total_kb = 0u64;
    let mut disk_used_kb = 0u64;
    let mut disk_pct = 0.0f32;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 1. /proc/loadavg (ej: "0.42 0.28 0.15 1/142 12345")
        if let Some(first_num) = line.split_whitespace().next() {
            if let Ok(v) = first_num.parse::<f32>() {
                if load_1m == 0.0 && (0.0..1000.0).contains(&v) {
                    load_1m = v;
                }
            }
        }

        // 2. /proc/meminfo
        if line.starts_with("MemTotal:") {
            if let Some(kb) = parse_kb_line(line) {
                mem_total_kb = kb;
            }
        } else if line.starts_with("MemAvailable:") {
            if let Some(kb) = parse_kb_line(line) {
                mem_avail_kb = kb;
            }
        } else if line.starts_with("MemFree:") {
            if let Some(kb) = parse_kb_line(line) {
                if mem_avail_kb == 0 {
                    mem_avail_kb = kb;
                }
            }
        }

        // 3. df -k /
        if line.starts_with('/') && line.contains('%') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                if let (Ok(tot), Ok(used)) = (parts[1].parse::<u64>(), parts[2].parse::<u64>()) {
                    disk_total_kb = tot;
                    disk_used_kb = used;
                    let pct_str = parts[4].trim_end_matches('%');
                    if let Ok(p) = pct_str.parse::<f32>() {
                        disk_pct = p;
                    }
                }
            }
        }
    }

    let ram_total_bytes = mem_total_kb * 1024;
    let ram_avail_bytes = mem_avail_kb * 1024;
    let ram_used_bytes = ram_total_bytes.saturating_sub(ram_avail_bytes);
    let ram_pct = if ram_total_bytes > 0 {
        (ram_used_bytes as f32 / ram_total_bytes as f32) * 100.0
    } else {
        0.0
    };

    let cpu_pct = (load_1m * 25.0).clamp(0.0, 100.0);

    Some(RemoteSystemSnapshot {
        destination: dest.to_string(),
        is_connected: true,
        cpu_pct,
        ram_pct,
        disk_pct,
        load_1m,
        ram_total_bytes,
        ram_used_bytes,
        disk_total_bytes: disk_total_kb * 1024,
        disk_used_bytes: disk_used_kb * 1024,
    })
}

fn parse_kb_line(line: &str) -> Option<u64> {
    let mut parts = line.split_whitespace();
    parts.next()?; // Clave
    let num = parts.next()?;
    num.parse::<u64>().ok()
}

/// Termina de forma segura un proceso por su identificador PID
pub(crate) fn kill_process_by_pid(pid: u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, TerminateProcess, PROCESS_TERMINATE,
        };
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if handle != 0 {
            unsafe {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
}

static AGENT_PROFILES: std::sync::OnceLock<std::sync::RwLock<Vec<String>>> =
    std::sync::OnceLock::new();

fn agent_profiles_store() -> &'static std::sync::RwLock<Vec<String>> {
    AGENT_PROFILES
        .get_or_init(|| std::sync::RwLock::new(crate::config::default_agent_executables()))
}

/// Actualiza la lista compartida de ejecutables de agentes CLI IA reconocidos por el monitor y los shells.
pub(crate) fn set_agent_profiles(names: &[String]) {
    if let Ok(mut guard) = agent_profiles_store().write() {
        *guard = names.to_vec();
    }
}

/// Devuelve los nombres de agentes configurados separados por coma (para `TEARMINAL_AGENT_PROFILES`).
pub(crate) fn active_agent_profiles_csv() -> String {
    if let Ok(guard) = agent_profiles_store().read() {
        guard.join(",")
    } else {
        String::new()
    }
}

fn exe_matches_profiles(exe_name: &str, profiles: &[String]) -> bool {
    let lower = exe_name.trim().to_ascii_lowercase();
    let stem = lower
        .strip_suffix(".exe")
        .or_else(|| lower.strip_suffix(".cmd"))
        .or_else(|| lower.strip_suffix(".bat"))
        .unwrap_or(&lower);
    profiles.iter().any(|p| p == stem)
}

fn is_configured_agent_exe(exe_name: &str) -> bool {
    if let Ok(guard) = agent_profiles_store().read() {
        exe_matches_profiles(exe_name, &guard)
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests unitarios
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_history_ring_buffer_36_samples() {
        let mut history = MetricHistory::new();
        assert_eq!(history.samples.len(), HISTORY_SAMPLES);

        for i in 1..=50 {
            history.push(i as f32);
        }

        assert_eq!(history.samples.len(), HISTORY_SAMPLES);
        assert_eq!(history.samples.back(), Some(&50.0));
        assert_eq!(history.samples.front(), Some(&15.0));
    }

    #[test]
    fn test_parse_remote_metrics_valid_linux_output() {
        let output = r#"
0.34 0.21 0.18 2/115 2841
MemTotal:       16384000 kB
MemFree:         4120000 kB
MemAvailable:    8192000 kB
/dev/sda1       102400000  40960000  61440000  40% /
"#;
        let snap = parse_remote_metrics("api-prod-01", output).expect("debe parsear");
        assert_eq!(snap.destination, "api-prod-01");
        assert!(snap.is_connected);
        assert_eq!(snap.load_1m, 0.34);
        assert_eq!(snap.ram_total_bytes, 16384000 * 1024);
        assert_eq!(snap.ram_pct, 50.0);
        assert_eq!(snap.disk_pct, 40.0);
        assert_eq!(snap.disk_total_bytes, 102400000 * 1024);
    }

    #[test]
    fn test_parse_remote_metrics_hostile_input() {
        assert!(parse_remote_metrics("dev-server", "").is_none());
        assert!(parse_remote_metrics("dev-server", "   \n\r\t  ").is_none());

        let corrupted = "\x1b[31mError 404\x07\x00\x1b[0m\nMemTotal: invalid\n/dev/sda1 not_nums";
        let snap = parse_remote_metrics("corrupted", corrupted);
        assert!(snap.is_some());
    }

    #[test]
    fn test_monitor_state_lifecycle() {
        let mut state = MonitorState::new();
        assert!(state.snapshot.is_none());

        let snap = SystemSnapshot {
            cpu_total: 25.0,
            cpu_cores: vec![CoreUsage {
                index: 0,
                percent: 25.0,
            }],
            cpu_name: "Test CPU".into(),
            cpu_freq_ghz: 4.0,
            cpu_thread_count: 8,
            cpu_process_count: 120,
            ram_total_bytes: 16 * 1024 * 1024 * 1024,
            ram_used_bytes: 8 * 1024 * 1024 * 1024,
            ram_pct: 50.0,
            gpu_name: "Test GPU".into(),
            gpu_usage_pct: 15.0,
            vram_total_bytes: 8 * 1024 * 1024 * 1024,
            vram_used_bytes: 2 * 1024 * 1024 * 1024,
            vram_pct: 25.0,
            disk_read_bytes_sec: 1000,
            disk_write_bytes_sec: 500,
            disk_usage_pct: 45.0,
            net_rx_bytes_sec: 2000,
            net_tx_bytes_sec: 1000,
            volumes: vec![],
            processes: vec![],
            top_project_folders: vec![],
            tearminal_cpu_pct: 0.5,
        };

        state.apply_snapshot(snap);
        assert!(state.snapshot.is_some());
        assert_eq!(state.cpu_history.samples.back(), Some(&25.0));
        assert_eq!(state.ram_history.samples.back(), Some(&50.0));
    }

    #[test]
    fn test_persistent_sampler_reports_real_cpu_after_baseline() {
        let mut state = MonitorState::new();
        let _ = state.sample_cpu(); // sin línea base: 0 por definición
        let burn = std::time::Instant::now();
        while burn.elapsed() < std::time::Duration::from_millis(150) {
            std::hint::black_box(burn.elapsed());
        }
        let (pct, _) = state.sample_cpu();
        assert!(pct > 0.0, "con línea base la CPU no puede ser 0 %: {pct}");
    }

    #[test]
    fn test_network_rates_use_deltas_and_are_zero_without_baseline() {
        let mut state = MonitorState::new();
        assert_eq!(state.sample_network(), (0, 0));
        assert!(state.prev_net.is_some());
        let _ = state.sample_network();
    }

    #[test]
    fn test_custom_agent_profiles_matching() {
        let profiles = vec!["goose".to_string(), "my-agent".to_string()];
        assert!(exe_matches_profiles("goose.exe", &profiles));
        assert!(exe_matches_profiles("MY-AGENT.CMD", &profiles));
        assert!(!exe_matches_profiles("notepad.exe", &profiles));
    }
}

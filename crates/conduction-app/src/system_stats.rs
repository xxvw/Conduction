//! 自プロセスの CPU / メモリ使用率を取得する。
//!
//! `sysinfo` でプロセス情報を採取。CPU 使用率は前回 refresh からの差分で算出されるため、
//! 連続呼び出しが必要。Tauri State として shared instance を保持する。

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// UI に渡す統計値。
#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
pub struct ResourceStats {
    /// プロセス CPU 使用率（0..100、論理コア合計を 100 と見なすホスト依存値）。
    pub cpu_percent: f32,
    /// プロセス常駐メモリ（MiB）。
    pub memory_mb: f64,
    /// 推定論理コア数（CPU% を補正したい時の参考）。
    pub logical_cores: u32,
}

#[derive(Clone)]
pub struct SystemStatsHandle {
    inner: Arc<Mutex<SystemStats>>,
}

struct SystemStats {
    sys: System,
    pid: Pid,
}

impl SystemStatsHandle {
    pub fn new() -> Self {
        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
        );
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
        Self {
            inner: Arc::new(Mutex::new(SystemStats { sys, pid })),
        }
    }

    pub fn snapshot(&self) -> ResourceStats {
        let mut state = self.inner.lock();
        let pid = state.pid;
        state.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
        let logical_cores = num_cpus_logical();
        let proc = state.sys.process(pid);
        let cpu_percent = proc.map(|p| p.cpu_usage()).unwrap_or(0.0);
        let memory_mb = proc
            .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
            .unwrap_or(0.0);
        ResourceStats {
            cpu_percent,
            memory_mb,
            logical_cores,
        }
    }
}

fn num_cpus_logical() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
}

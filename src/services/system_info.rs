use crate::panels::taskbar::events;
use log::{debug, info};
use std::sync::{LazyLock, Mutex, OnceLock, RwLock};
use sysinfo::{MemoryRefreshKind, Networks, System};
use tokio::time::{Duration, sleep};

/// Tracker is shared across the application and is used to track which metrics are being used by the UI.
static SYSTEM_INFO_TRACKER: LazyLock<Mutex<RequirementTracker>> =
    LazyLock::new(|| Mutex::new(RequirementTracker::default()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Represents a category of system metrics.
/// These categories are used to track which metrics are being used by the UI.
pub enum MetricCategory {
    Cpu,
    Ram,
    Network,
    Disks,
    Processes,
    Temperatures,
}

#[derive(Default)]
/// Tracks which metrics are being used by the UI.
///
/// Each metric is tracked as a counter (RAII).
/// When a component needs a metric, it acquires a handle.
/// When the handle is dropped, the counter is decremented.
///
/// The polling worker uses this to determine which metrics to refresh.
struct RequirementTracker {
    cpu: usize,
    ram: usize,
    network: usize,
    disks: usize,
    processes: usize,
    temperatures: usize,
}

impl RequirementTracker {
    fn to_filter(&self) -> RefreshFilter {
        RefreshFilter {
            cpu: self.cpu > 0,
            ram: self.ram > 0,
            network: self.network > 0,
            disks: self.disks > 0,
            process_info: self.processes > 0,
            components_temperature: self.temperatures > 0,
        }
    }
}

/// A handle that ensures a specific system metric is refreshed as long as it exists.
/// When dropped, it automatically updates the tracking system.
pub struct RefreshHandle {
    category: MetricCategory,
}

impl Drop for RefreshHandle {
    fn drop(&mut self) {
        if let Ok(mut tracker) = SYSTEM_INFO_TRACKER.lock() {
            match self.category {
                MetricCategory::Cpu => tracker.cpu = tracker.cpu.saturating_sub(1),
                MetricCategory::Ram => tracker.ram = tracker.ram.saturating_sub(1),
                MetricCategory::Network => tracker.network = tracker.network.saturating_sub(1),
                MetricCategory::Disks => tracker.disks = tracker.disks.saturating_sub(1),
                MetricCategory::Processes => {
                    tracker.processes = tracker.processes.saturating_sub(1)
                }
                MetricCategory::Temperatures => {
                    tracker.temperatures = tracker.temperatures.saturating_sub(1)
                }
            }
            debug!(
                "Metric release: {:?} (Remaining: CPU={}, RAM={}, Net={})",
                self.category, tracker.cpu, tracker.ram, tracker.network
            );
        }
    }
}

/// Requests a refresh handle to indicate that the CPU metric is being used and should be refreshed periodically.
pub fn require_cpu() -> RefreshHandle {
    acquire(MetricCategory::Cpu)
}

/// Requests a refresh handle to indicate that the RAM metric is being used and should be refreshed periodically.
pub fn require_ram() -> RefreshHandle {
    acquire(MetricCategory::Ram)
}

/// Requests a refresh handle to indicate that the Network metric is being used and should be refreshed periodically.
pub fn require_network() -> RefreshHandle {
    acquire(MetricCategory::Network)
}

/// Requests a refresh handle to indicate that the Disks metric is being used and should be refreshed periodically.
pub fn require_disks() -> RefreshHandle {
    acquire(MetricCategory::Disks)
}

/// Requests a refresh handle to indicate that the Processes metric is being used and should be refreshed periodically.
pub fn require_processes() -> RefreshHandle {
    acquire(MetricCategory::Processes)
}

/// Requests a refresh handle to indicate that the Temperatures metric is being used and should be refreshed periodically.
pub fn require_temperatures() -> RefreshHandle {
    acquire(MetricCategory::Temperatures)
}

fn acquire(category: MetricCategory) -> RefreshHandle {
    if let Ok(mut tracker) = SYSTEM_INFO_TRACKER.lock() {
        match category {
            MetricCategory::Cpu => tracker.cpu += 1,
            MetricCategory::Ram => tracker.ram += 1,
            MetricCategory::Network => tracker.network += 1,
            MetricCategory::Disks => tracker.disks += 1,
            MetricCategory::Processes => tracker.processes += 1,
            MetricCategory::Temperatures => tracker.temperatures += 1,
        }
        debug!("Metric lease acquired: {:?}", category);
    }
    RefreshHandle { category }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RefreshFilter {
    cpu: bool,
    ram: bool,
    network: bool,
    disks: bool,
    process_info: bool,
    components_temperature: bool,
}

impl RefreshFilter {
    fn new() -> Self {
        Self {
            cpu: true,
            ram: true,
            network: true,
            disks: true,
            process_info: true,
            components_temperature: true,
        }
    }

    fn nothing_was_refreshed(&self) -> bool {
        self == &RefreshFilter::default()
    }
}

static SYSTEM_STATE: OnceLock<Mutex<SystemState>> = OnceLock::new();

struct SystemState {
    sys: System,
    networks: Option<Networks>,
}

/// Static system information that does not change during runtime.
/// This is fetched once at startup.
pub struct StaticInfo {
    /// OS name (e.g. "Ubuntu", "Windows", "Fedora")
    pub name: String,
    /// Kernel version (e.g. "5.15.0-52-generic")
    pub kernel: String,
    /// OS version (e.g. "22.04 LTS")
    pub os_ver: String,
    /// long OS version (e.g. "Linux (Ubuntu 24.04)")
    pub long_os_ver: String,
    /// distribution id (e.g. "arch", "ubuntu", "fedora")
    pub distribution_id: String,
    /// Hostname (e.g. "my-pc")
    pub host: String,
    /// Total memory in bytes
    pub total_mem: u64,
    /// Number of physical CPU cores
    pub cpu_count: usize,
}

static STATIC_INFO: OnceLock<StaticInfo> = OnceLock::new();

pub fn get_static_info() -> &'static StaticInfo {
    STATIC_INFO.get_or_init(|| {
        let mut sys = System::new();
        sys.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());

        StaticInfo {
            name: System::name().unwrap_or_else(|| "Unknown".into()),
            kernel: System::kernel_version().unwrap_or_else(|| "Unknown".into()),
            os_ver: System::os_version().unwrap_or_else(|| "Unknown".into()),
            long_os_ver: System::long_os_version().unwrap_or_else(|| "Unknown".into()),
            distribution_id: System::distribution_id(),
            host: System::host_name().unwrap_or_else(|| "Unknown".into()),
            total_mem: sys.total_memory(),
            cpu_count: sys.physical_core_count().unwrap_or(0),
        }
    })
}

#[derive(Debug, Clone)]
pub struct SystemStatus {
    // percentage (0.0 - 100.0)
    pub cpu_usage: f32,
    // percentage (0.0 - 100.0)
    pub ram_usage: f32,
    // in bytes
    pub ram_used: u64,
    // in bytes
    pub network_transmitted: u64,
    // in bytes
    pub network_received: u64,
    // percentage (0.0 - 100.0)
    pub disk_usage: f32,
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            ram_usage: 0.0,
            ram_used: 0,
            network_transmitted: 0,
            network_received: 0,
            disk_usage: 0.0,
        }
    }
}

static SYSTEM_STATUS: OnceLock<RwLock<SystemStatus>> = OnceLock::new();

/// Get the latest cached system status.
pub fn get_status() -> SystemStatus {
    SYSTEM_STATUS
        .get_or_init(|| RwLock::new(SystemStatus::default()))
        .read()
        .unwrap()
        .clone()
}

/// Start the system info monitor.
///
/// This function spawns a background thread that periodically refreshes the system information.
/// It uses a requirement tracking system to only refresh the metrics that are being used.
/// If no metrics are being used, the thread will sleep.
pub fn start_monitor() {
    info!("Starting optimized system info service with requirement tracking...");

    std::thread::spawn(move || {
        let _ = get_static_info();
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            polling_worker().await;
        });
    });
}

async fn polling_worker() {
    loop {
        let filter = if let Ok(tracker) = SYSTEM_INFO_TRACKER.lock() {
            tracker.to_filter()
        } else {
            RefreshFilter::default()
        };

        if !filter.nothing_was_refreshed() {
            let stats = update_info(filter).await;
            events::send(events::TaskbarEvent::SystemStatus(stats));
        }

        sleep(Duration::from_millis(2000)).await;
    }
}

async fn update_info(filter: RefreshFilter) -> SystemStatus {
    let state_lock = SYSTEM_STATE.get_or_init(|| {
        Mutex::new(SystemState {
            sys: System::new(),
            networks: None,
        })
    });

    let mut stats = get_status();
    let mut state = state_lock.lock().unwrap();

    // Perform granular refreshes
    if filter.cpu {
        state.sys.refresh_cpu_usage();
        stats.cpu_usage = state.sys.global_cpu_usage();
    }

    if filter.ram {
        state
            .sys
            .refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
        let used = state.sys.used_memory();
        let total = state.sys.total_memory();
        if total > 0 {
            stats.ram_usage = (used as f32 / total as f32) * 100.0;
        }
    }

    if filter.network {
        if state.networks.is_none() {
            let mut networks = sysinfo::Networks::new();
            networks.refresh_list();
            state.networks = Some(networks);
        } else if let Some(ref mut net) = state.networks {
            net.refresh_list();

            let mut transmitted = 0;
            let mut received = 0;
            for (_interface_name, data) in net.iter() {
                transmitted += data.transmitted();
                received += data.received();
            }
            stats.network_transmitted = transmitted;
            stats.network_received = received;
        }
    }

    // Update global cache
    if let Ok(mut cache) = SYSTEM_STATUS
        .get_or_init(|| RwLock::new(SystemStatus::default()))
        .write()
    {
        *cache = stats.clone();
    }

    debug!("Updated system info with filter: {:?}", filter);
    stats
}

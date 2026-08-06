#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorMode {
    Primary,
    All,
}

impl Default for MonitorMode {
    fn default() -> Self {
        MonitorMode::Primary
    }
}

impl MonitorMode {
    pub fn label(&self) -> &'static str {
        match self {
            MonitorMode::Primary => "Primary Monitor",
            MonitorMode::All => "All Monitors",
        }
    }


    pub fn toggle(&self) -> Self {
        match self {
            MonitorMode::Primary => MonitorMode::All,
            MonitorMode::All => MonitorMode::Primary,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub monitor_mode: MonitorMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            monitor_mode: MonitorMode::Primary,
        }
    }
}

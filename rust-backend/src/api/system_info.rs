use axum::extract::State;
use axum::response::Json;
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct SystemInfoResponse {
    total_ram_mb: u64,
    available_ram_mb: u64,
    cpu_cores: usize,
    current_mode: String,
}

pub async fn get_system_info(State(state): State<AppState>) -> Json<SystemInfoResponse> {
    let total_ram = get_total_ram();
    let available_ram = get_available_ram();
    let cpu_cores = num_cpus::get();
    
    let current_mode = match state.config.performance_mode {
        crate::config::PerformanceMode::Lightweight => "lightweight",
        crate::config::PerformanceMode::Normal => "normal",
    };

    Json(SystemInfoResponse {
        total_ram_mb: total_ram,
        available_ram_mb: available_ram,
        cpu_cores,
        current_mode: current_mode.to_string(),
    })
}

fn get_total_ram() -> u64 {
    let system = sysinfo::System::new_all();
    (system.total_memory() / (1024 * 1024)) as u64
}

fn get_available_ram() -> u64 {
    let mut system = sysinfo::System::new_all();
    system.refresh_memory();
    (system.available_memory() / (1024 * 1024)) as u64
}

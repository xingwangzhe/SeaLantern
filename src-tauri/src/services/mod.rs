pub mod async_loader;
pub mod config_parser;
pub mod download_manager;
pub mod global;
pub mod http_command_handlers;
#[cfg(feature = "docker")]
pub mod http_server;
#[cfg(not(feature = "docker"))]
pub mod http_server {
    /// Stub implementation for non-docker builds.
    /// When the `docker` feature is not enabled, this module provides a
    /// no-op `run_http_server` function so that code referencing
    /// `services::http_server::run_http_server` still compiles.
    ///
    /// The real implementation (using `axum` / `tower-http`) is compiled
    /// only when the `docker` feature is enabled.
    pub async fn run_http_server(_addr: &str, _static_dir: Option<String>) {
        // no-op when docker feature is not enabled
    }
}
pub mod i18n;
pub mod java_detector;
pub mod java_installer;
pub mod join_manager;
pub mod mcs_plugin_manager;
pub mod mod_manager;
pub mod panic_report;
pub mod player_manager;
pub mod server_id_manager;
pub mod server_installer;
pub mod server_log_pipeline;
pub mod server_manager;
pub mod settings_manager;
pub mod starter_installer_links;

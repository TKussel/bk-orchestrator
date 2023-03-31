use tracing::info;

pub(crate) fn print_banner() {
    info!("🎼 BK-Orchestrator ({}) v{}) starting up ...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}

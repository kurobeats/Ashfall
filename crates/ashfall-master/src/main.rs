fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Ashfall master server starting...");

    println!("Ashfall master v{}", ashfall_core::constants::MASTER_VERSION);
}

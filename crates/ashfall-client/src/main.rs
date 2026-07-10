fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Ashfall client starting...");

    println!("Ashfall client v{}", ashfall_core::constants::CLIENT_VERSION);
}

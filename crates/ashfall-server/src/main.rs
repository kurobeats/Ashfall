fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Ashfall dedicated server starting...");

    // TODO: config load, server init (PR19-28)
    println!("Ashfall server v{}", ashfall_core::constants::DEDICATED_VERSION);
}

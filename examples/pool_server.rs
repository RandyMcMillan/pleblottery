use integration_tests_sv2::*;
use pleblottery::{service::PlebLotteryService, state::SharedStateHandle};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing/logging using pleblottery's standard setup or fallback
    let _ = tracing_subscriber::fmt::try_init();
    
    println!("🚀 Starting mock Stratum V2 Template Provider...");
    
    // 1. Start the upstream template provider on a local port
    let (_tp, tp_address) = start_template_provider(None);
    println!("📦 Template Provider running at {}", tp_address);

    // 2. Load config using `from_file` and point it to our mock template provider
    let mut config = pleblottery::config::PleblotteryConfig::from_file("./config.toml")
        .unwrap_or_else(|_| panic!("Failed to load config.toml"));
    
    config.template_distribution_config.server_addr = tp_address;

    let shared_state = SharedStateHandle::default();

    println!("⚡ Starting Pleblottery Mining Server on port {}...", config.mining_server_config.listening_port);

    // 3. Initialize and start PlebLottery Service
    let mut pleblottery_service = PlebLotteryService::new(
        config.mining_server_config.clone(),
        config.template_distribution_config.clone(),
        shared_state,
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = pleblottery_service.start().await {
            eprintln!("PlebLottery service error: {:?}", e);
        }
    });

    println!("✅ Pleblottery is live! Downstream miners can now connect to port 3333.");
    println!("Press Ctrl-C to stop.");

    // Keep alive so miners can connect and hash
    loop {
        sleep(Duration::from_secs(3600)).await;
    }
}

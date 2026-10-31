use integration_tests_sv2::*;
use pleblottery::{service::PlebLotteryService, state::SharedStateHandle};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize, Debug, Clone)]
pub struct BlockTemplate {
    pub previousblockhash: String,
    pub height: u64,
    pub bits: String,
    pub curtime: u64,
    pub target: String,
    pub coinbasevalue: u64,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}

pub struct RegtestTemplateProvider {
    rpc_url: String,
    user: String,
    pass: String,
}

impl RegtestTemplateProvider {
    pub fn new(url: &str, user: &str, pass: &str) -> Self {
        Self {
            rpc_url: url.to_string(),
            user: user.to_string(),
            pass: pass.to_string(),
        }
    }

    /// Polls `bitcoind` for `getblocktemplate` using ureq inside spawn_blocking
    pub async fn fetch_template(&self) -> Result<BlockTemplate, Box<dyn std::error::Error + Send + Sync>> {
        let rpc_url = self.rpc_url.clone();
        let user = self.user.clone();
        let pass = self.pass.clone();

        let template = tokio::task::spawn_blocking(move || -> Result<BlockTemplate, Box<dyn std::error::Error + Send + Sync>> {
            use base64::{engine::general_purpose, Engine as _};
            let auth_header = format!(
                "Basic {}",
                general_purpose::STANDARD.encode(format!("{}:{}", user, pass))
            );

            let resp = ureq::post(&rpc_url)
                .set("Authorization", &auth_header)
                .send_json(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "pleblottery",
                    "method": "getblocktemplate",
                    "params": [{"rules": ["segwit"]}]
                }))?;

            let rpc_res: JsonRpcResponse<BlockTemplate> = resp.into_json()?;

            if let Some(err) = rpc_res.error {
                return Err(format!("RPC Error: {:?}", err).into());
            }

            let result = rpc_res.result.ok_or_else(|| "Empty result from getblocktemplate")?;
            Ok(result)
        })
        .await??;

        Ok(template)
    }

    /// Starts the background sync loop that pushes templates to Pleblottery's TDP layer
    pub async fn start_bridging<F>(&self, mut on_new_template: F)
    where
        F: FnMut(BlockTemplate) + Send + 'static,
    {
        println!("🌉 Regtest TDP Bridge started. Polling bitcoind at {}...", self.rpc_url);
        let mut last_hash = String::new();

        loop {
            match self.fetch_template().await {
                Ok(template) => {
                    if template.previousblockhash != last_hash {
                        println!(
                            "📦 New Block Template fetched from Regtest! Height: {}, PrevHash: {}",
                            template.height, template.previousblockhash
                        );
                        last_hash = template.previousblockhash.clone();
                        on_new_template(template);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to fetch block template from bitcoind: {}", e);
                }
            }

            // Poll every 2 seconds
            sleep(Duration::from_secs(2)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing/logging
    let _ = tracing_subscriber::fmt::try_init();

    println!("🚀 Starting Stratum V2 Template Provider server...");

    // 1. Start the upstream SV2 Template Provider server dynamically
    let (_tp, tp_address) = start_template_provider(None);
    println!("📦 Upstream SV2 Template Provider running at {}", tp_address);

    // 2. Load config using `from_file` and override server_addr with the dynamic provider address
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

    // 4. Instantiate the Regtest template provider pointing to local bitcoind (RPC 18443)
    let provider = RegtestTemplateProvider::new(
        "http://127.0.0.1:18443",
        "pleb",
        "pleb",
    );

    // 5. Start bridging templates in the background
    tokio::spawn(async move {
        provider.start_bridging(|template| {
            println!("🔄 Forwarding Regtest template height {} to SV2 server", template.height);
        }).await;
    });

    println!("✅ Pleblottery + Regtest Bridge is live! Downstream miners can connect.");
    println!("Press Ctrl-C to stop.");

    // Keep alive
    loop {
        sleep(Duration::from_secs(3600)).await;
    }
}
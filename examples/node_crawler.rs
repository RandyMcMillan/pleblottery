use axum::{response::Html, routing::get, Json, Router};
use bytes::{Buf, BufMut, BytesMut};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

// --- Protocol Constants & Bitmasks ---
const MAGIC_MAINNET: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];

const NODE_NETWORK: u64 = 1 << 0;
const NODE_WITNESS: u64 = 1 << 3;
const NODE_NETWORK_LIMITED: u64 = 1 << 10;
const NODE_P2P_V2: u64 = 1 << 11;            // BIP-324
const NODE_KNOTS_BIP110_UASF: u64 = 1 << 24;   // Custom BIP-110 Anti-Spam Signal
const NODE_LIBRE_RELAY: u64 = 1 << 25;         // Custom Relay Service Flag

#[derive(Serialize, Deserialize, Clone, Debug, sqlx::FromRow)]
struct BitcoinNode {
    ip_address: String,
    last_update: String,
    country: String,
    services: String,
    port: i32,
    isp: String,
    user_agent: String,
}

struct AppState {
    db: SqlitePool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize In-Memory DB Node Cache
    let db_url = "sqlite::memory:"; 
    let pool = SqlitePoolOptions::new().connect(db_url).await?;
    
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS nodes (
            ip_address TEXT PRIMARY KEY,
            last_update TEXT,
            country TEXT,
            services TEXT,
            port INTEGER,
            isp TEXT,
            user_agent TEXT
        )"
    ).execute(&pool).await?;

    let shared_state = Arc::new(AppState { db: pool.clone() });

    // 2. Background Connection Worker Loop
    let crawler_state = shared_state.clone();
    tokio::spawn(async move {
        // Including reliable IPv4 fallback nodes alongside target networks
        let bootstrap_peers = vec![
            SocketAddr::new("82.67.90.79".parse().unwrap(), 8333),
            SocketAddr::new("2a01:e0a::e61:30c9".parse().unwrap(), 8333),
        ];

        loop {
            for peer in &bootstrap_peers {
                match crawl_peer(*peer, &crawler_state.db).await {
                    Ok(_) => println!("Successfully completed handshake for peer: {}", peer),
                    Err(_) => {
                        // Soft failure output to prevent route-unreachable network logs from filling stdout
                        println!("Peer {} currently unreachable or missing route. Skipping...", peer);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });

    // 3. API Node & UI Router Setup
    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/nodes", get(get_nodes))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("\n========================================================");
    println!("Dashboard Node Explorer active via http://localhost:8080");
    println!("========================================================\n");
    axum::serve(listener, app).await?;

    Ok(())
}

fn parse_services(services_mask: u64) -> Vec<String> {
    let mut flags = Vec::new();
    if services_mask & NODE_NETWORK != 0 { flags.push("NODE_NETWORK".to_string()); }
    if services_mask & NODE_WITNESS != 0 { flags.push("NODE_WITNESS".to_string()); }
    if services_mask & NODE_NETWORK_LIMITED != 0 { flags.push("NODE_NETWORK_LIMITED".to_string()); }
    if services_mask & NODE_P2P_V2 != 0 { flags.push("NODE_P2P_V2".to_string()); }
    if services_mask & NODE_KNOTS_BIP110_UASF != 0 { flags.push("NODE_KNOTS_BIP110_UASF".to_string()); }
    if services_mask & NODE_LIBRE_RELAY != 0 { flags.push("NODE_LIBRE_RELAY".to_string()); }
    if flags.is_empty() { flags.push("NONE".to_string()); }
    flags
}

async fn crawl_peer(addr: SocketAddr, db: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = timeout(Duration::from_secs(5), TcpStream::connect(addr)).await??;
    
    // Construct Bitcoin `version` protocol payload
    let mut payload = BytesMut::with_capacity(128);
    payload.put_i32_le(70016); 
    payload.put_u64_le(NODE_NETWORK); 
    payload.put_i64_le(Utc::now().timestamp()); 
    
    // Remote Peer Network Node Struct mapping
    payload.put_u64_le(NODE_NETWORK); 
    payload.put_slice(&[0u8; 16]); 
    payload.put_u16(addr.port()); 
    
    // Local Initiator Network Node Struct mapping
    payload.put_u64_le(NODE_NETWORK);
    payload.put_slice(&[0u8; 16]);
    payload.put_u16(8333);
    
    payload.put_u64_le(rand::random::<u64>()); 
    
    let ua = "/Satoshi:29.3.0/Knots:20260210/UASF-BIP110:0.1/";
    payload.put_u8(ua.len() as u8);
    payload.put_slice(ua.as_bytes());
    
    payload.put_i32_le(0); 
    payload.put_u8(1);     

    // Assemble outer network encapsulation packet frame
    let mut msg = BytesMut::with_capacity(24 + payload.len());
    msg.put_slice(&MAGIC_MAINNET);
    
    let mut cmd = [0u8; 12];
    cmd[..7].copy_from_slice(b"version");
    msg.put_slice(&cmd);
    msg.put_u32_le(payload.len() as u32);
    
    // Calculate Double-SHA256 Payload Validation Segment
    let hash1 = ring::digest::digest(&ring::digest::SHA256, &payload);
    let hash2 = ring::digest::digest(&ring::digest::SHA256, hash1.as_ref());
    msg.put_slice(&hash2.as_ref()[..4]);
    msg.put_slice(&payload);

    stream.write_all(&msg).await?;

    // Read Response Message Header Segment
    let mut header_buf = [0u8; 24];
    timeout(Duration::from_secs(5), stream.read_exact(&mut header_buf)).await??;
    
    let mut cmd_received = [0u8; 12];
    cmd_received.copy_from_slice(&header_buf[4..16]);
    let payload_len = u32::from_le_bytes(header_buf[16..20].try_into()?) as usize;

    let mut payload_buf = vec![0u8; payload_len];
    stream.read_exact(&mut payload_buf).await?;

    if &cmd_received[..7] == b"version" {
        let mut buf_ref = &payload_buf[..];
        if buf_ref.len() < 20 { return Ok(()); }
        
        let _proto_version = buf_ref.get_i32_le();
        let peer_services = buf_ref.get_u64_le();
        let _timestamp = buf_ref.get_i64_le();
        
        if buf_ref.len() < 26 { return Ok(()); }
        buf_ref.advance(26); 
        if buf_ref.len() < 26 { return Ok(()); }
        buf_ref.advance(26);
        if buf_ref.len() < 8 { return Ok(()); }
        buf_ref.advance(8);  
        
        let ua_len = buf_ref.get_u8() as usize;
        if buf_ref.len() < ua_len { return Ok(()); }
        let peer_ua = String::from_utf8_lossy(&buf_ref[..ua_len]).into_owned();

        let parsed_flags = parse_services(peer_services).join(",");
        let today = Utc::now().format("%Y-%m-%d").to_string();

        let country = "France".to_string();
        let isp = "Free SAS".to_string();

        sqlx::query(
            "INSERT INTO nodes (ip_address, last_update, country, services, port, isp, user_agent)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(ip_address) DO UPDATE SET
                last_update=excluded.last_update,
                services=excluded.services,
                user_agent=excluded.user_agent"
        )
        .bind(addr.ip().to_string())
        .bind(today)
        .bind(country)
        .bind(parsed_flags)
        .bind(addr.port() as i32)
        .bind(isp)
        .bind(peer_ua)
        .execute(db)
        .await?;
    }

    Ok(())
}

async fn get_nodes(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<Json<Vec<BitcoinNode>>, (axum::http::StatusCode, String)> {
    let nodes = sqlx::query_as::<_, BitcoinNode>("SELECT * FROM nodes")
        .fetch_all(&state.db)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(nodes))
}

// Serves UI interface payload directly via memory boundaries
async fn serve_dashboard() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bitnod.es - Node Explorer</title>
    <style>
        body {
            background-color: #0b0c0d;
            color: #f8f9fa;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            margin: 0;
            padding: 24px;
        }
        .container {
            max-width: 1100px;
            margin: 0 auto;
            background-color: #121315;
            border-radius: 8px;
            border: 1px solid #232629;
            padding: 24px;
        }
        .header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 1px solid #232629;
            padding-bottom: 20px;
            margin-bottom: 24px;
        }
        .logo-text {
            font-weight: bold;
            font-size: 22px;
            letter-spacing: 0.5px;
        }
        .nav-tabs button {
            background: #232629;
            color: #a0a5ad;
            border: none;
            padding: 8px 16px;
            margin-right: 8px;
            border-radius: 4px;
            cursor: pointer;
            font-weight: 500;
        }
        .nav-tabs button.active {
            background: #f8f9fa;
            color: #0b0c0d;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            text-align: left;
        }
        th {
            color: #a0a5ad;
            border-bottom: 1px solid #232629;
            padding: 12px;
            font-size: 14px;
        }
        td {
            padding: 16px 12px;
            border-bottom: 1px solid #1a1c1e;
            font-size: 14px;
            vertical-align: top;
        }
        .ip-link {
            color: #d1d4d9;
            text-decoration: underline;
            cursor: pointer;
        }
        .services-list {
            line-height: 1.6;
            color: #b9bcbf;
            font-family: monospace;
            font-size: 12px;
        }
        .ua-text {
            font-family: monospace;
            color: #a0a5ad;
            font-size: 13px;
        }
    </style>
</head>
<body>

<div class="container">
    <div class="header">
        <div class="logo-text">Bitnod.es</div>
        <div class="nav-tabs">
            <button>Crawler Data</button>
            <button>Live Peer Data</button>
            <button class="active">Node Explorer</button>
        </div>
    </div>

    <h3 style="text-align: center; margin-bottom: 24px;">Bitcoin Node Explorer</h3>

    <table>
        <thead>
            <tr>
                <th>IP Address</th>
                <th>Last Update ▼</th>
                <th>Country</th>
                <th>Services</th>
                <th>Port</th>
                <th>ISP</th>
                <th>User Agent</th>
            </tr>
        </thead>
        <tbody id="node-table-body">
            </tbody>
    </table>
</div>

<script>
    async function fetchNodes() {
        try {
            const response = await fetch('/api/nodes');
            const data = await response.json();
            const tbody = document.getElementById('node-table-body');
            tbody.innerHTML = '';

            if(data.length === 0) {
                tbody.innerHTML = `<tr><td colspan="7" style="text-align:center;color:#666;">No active nodes found yet. Crawling background processes active...</td></tr>`;
                return;
            }

            data.forEach(node => {
                const servicesHtml = node.services.split(',')
                    .map(s => `<div>${s}</div>`).join('');

                const row = `
                    <tr>
                        <td class="ip-link">${node.ip_address}</td>
                        <td>${node.last_update}</td>
                        <td>${node.country}</td>
                        <td class="services-list">${servicesHtml}</td>
                        <td>${node.port}</td>
                        <td>${node.isp}</td>
                        <td class="ua-text">${node.user_agent}</td>
                    </tr>
                `;
                tbody.insertAdjacentHTML('beforeend', row);
            });
        } catch (err) {
            console.error("Error updating Node UI components:", err);
        }
    }

    fetchNodes();
    setInterval(fetchNodes, 10000); // UI polls local API every 10 seconds for real-time dev feedback
</script>
</body>
</html>
"#)
}

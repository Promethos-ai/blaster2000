//! Example: Node announcement to DHT
//! 
//! This shows how an inference node announces itself to the DHT
//! so clients can discover it.

use anyhow::Result;
use distributed_client::{NodeAnnouncer, NodeConfig, ModelInfo};
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // Load this node's configuration
    // Nodes read their own config to know their role
    let node_config: NodeConfig = serde_json::from_str(&std::fs::read_to_string(
        "./node_configs/node-00.json"
    )?)?;
    
    let model_info = ModelInfo {
        n_layers: 32,
        n_ctx: 8192,
        n_vocab: 128256,
        n_embd: 4096,
        n_head: 32,
        n_kv_head: 8,
    };
    
    // Cluster info_hash (all nodes use the same hash)
    let cluster_info_hash = "llama-3.1-8b-cluster";
    
    // DHT tracker address
    let tracker_host = "162.221.207.169";
    let tracker_port = 7001;
    
    // This node's QUIC port (where it serves inference requests)
    let node_port = node_config.server_port;
    
    println!("Starting node announcement...");
    println!("Node: {}", node_config.node_name);
    println!("Cluster info_hash: {}", cluster_info_hash);
    println!("Announcing to tracker: {}:{}", tracker_host, tracker_port);
    println!("Serving on port: {}\n", node_port);
    
    // Create announcer
    let announcer = NodeAnnouncer::new(
        cluster_info_hash.to_string(),
        node_config,
        model_info,
        tracker_host,
        tracker_port,
        node_port,
    )?;
    
    // Start announcing (runs in background)
    announcer.start().await?;
    
    println!("[OK] Node is now announcing to DHT");
    println!("Press Ctrl+C to stop...\n");
    
    // Keep running
    tokio::signal::ctrl_c().await?;
    
    println!("\nStopping announcement...");
    announcer.stop().await;
    
    Ok(())
}

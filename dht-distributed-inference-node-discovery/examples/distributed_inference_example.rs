//! Example: Using the Distributed Inference Client with DHT Discovery
//! 
//! This example shows how to use DHT-based node discovery
//! to send inference requests through the distributed pipeline.

use anyhow::Result;
use distributed_client::{
    DistributedInferenceClient, 
    InferenceParams, 
    Message
};
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // Discover nodes from DHT using cluster info_hash
    // Nodes announce themselves to the DHT with this info_hash
    let cluster_info_hash = "llama-3.1-8b-cluster"; // Info hash for the cluster
    
    // DHT tracker address (nodes announce to this tracker)
    let tracker_host = "162.221.207.169";  // Your tracker server
    let tracker_port = 7001;
    
    println!("Discovering nodes from DHT...");
    println!("Cluster info_hash: {}", cluster_info_hash);
    println!("Tracker: {}:{}\n", tracker_host, tracker_port);
    
    // Create client with DHT discovery
    let client = DistributedInferenceClient::new(
        cluster_info_hash.to_string(),
        tracker_host,
        tracker_port,
    ).await?;
    
    // Enable automatic node discovery refresh
    // This will automatically detect new nodes joining every 30 seconds
    println!("Enabling auto-refresh (30 second interval)...");
    client.start_auto_refresh(std::time::Duration::from_secs(30));
    println!("[OK] Auto-refresh enabled - you will see new nodes automatically!\n");
    
    // Display cluster information
    let info = client.cluster_info().await;
    println!("========================================");
    println!("Distributed Inference Client");
    println!("========================================");
    println!("Cluster: {}", info.cluster_name);
    println!("Nodes discovered: {}", info.num_nodes);
    println!("Entry Node: {:?}", info.entry_node);
    println!("Exit Node: {:?}", info.exit_node);
    println!("Model: {} layers, {} context", 
             info.model_info.n_layers, 
             info.model_info.n_ctx);
    println!("========================================\n");
    
    // Verify entry node is available
    let entry = client.entry_node()
        .await
        .ok_or_else(|| anyhow::anyhow!("No entry node discovered from DHT"))?;
    
    println!("Connecting to entry node: {}", entry.addr);
    println!("Node: {} (Shard {})", entry.node_name, entry.shard_id);
    println!("This node handles layers {}-{}\n", 
             entry.layer_start, 
             entry.layer_end);
    
    // Prepare inference parameters
    let params = InferenceParams {
        temperature: 0.7,
        max_tokens: 512,
        top_p: 0.9,
    };
    
    // Optional: Add context messages
    let context = vec![
        Message {
            role: "system".to_string(),
            content: "You are a helpful AI assistant.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: "What is the capital of France?".to_string(),
        },
    ];
    
    println!("Sending query through distributed pipeline...");
    println!("Query: What is the capital of France?\n");
    
    // Process query through the pipeline
    // This automatically forwards through all nodes:
    // entry_node → node-01 → node-02 → exit_node → response
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        client.process_query(
            "What is the capital of France?",
            Some(&context),
            params,
        )
    ).await??;
    
    println!("========================================");
    println!("Response Received");
    println!("========================================");
    println!("Answer: {}", response.answer);
    println!("\nMetadata:");
    if let Some(shard_id) = response.metadata.shard_id {
        println!("  Processed by shard: {}", shard_id);
    }
    if let Some(layers) = &response.metadata.layers_processed {
        println!("  Layers: {}", layers);
    }
    if let Some(tokens) = response.metadata.tokens_generated {
        println!("  Tokens generated: {}", tokens);
    }
    if let Some(latency) = response.metadata.latency_ms {
        println!("  Latency: {:.2} ms", latency);
    }
    println!("========================================");
    
    Ok(())
}

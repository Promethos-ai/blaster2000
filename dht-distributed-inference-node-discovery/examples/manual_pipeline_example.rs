//! Example: Manual Pipeline Processing
//! 
//! This example shows how to manually control the pipeline,
//! processing each node step-by-step.

use anyhow::Result;
use distributed_client::{DistributedInferenceClient, InferenceParams, Message};
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    let client = DistributedInferenceClient::new("./node_configs/cluster_config.json")?;
    
    // Get all nodes in order
    let mut nodes = client.nodes().to_vec();
    nodes.sort_by_key(|n| n.node_id);
    
    println!("Processing through {} nodes manually:\n", nodes.len());
    
    let params = InferenceParams {
        temperature: 0.7,
        max_tokens: 512,
        top_p: 0.9,
    };
    
    let mut current_query = "Explain quantum computing in simple terms".to_string();
    let mut current_context: Option<Vec<Message>> = None;
    
    // Process through each node in sequence
    for (step, node) in nodes.iter().enumerate() {
        println!("Step {}: Processing on {}", step + 1, node.node_name);
        println!("  Server: {}:{}", node.server_host, node.server_port);
        println!("  Layers: {}-{}", node.layer_start, node.layer_end);
        
        // In a real implementation, you would:
        // 1. Connect to this node
        // 2. Send the query/activations
        // 3. Receive processed activations
        // 4. Pass to next node
        
        println!("  → Query: {}", current_query);
        
        // Example: Process on this node (replace with actual QUIC call)
        // For now, just simulate
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        println!("  ✓ Completed\n");
        
        // If this is the exit node, get final answer
        if node.has_output {
            println!("Final node reached - generating output");
            break;
        }
    }
    
    println!("Pipeline processing complete!");
    
    Ok(())
}

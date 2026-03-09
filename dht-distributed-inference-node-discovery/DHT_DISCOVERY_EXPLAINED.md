# DHT Discovery: How Clients See New Nodes

## Current Architecture

### How DHT Lists Work

**Important**: Clients do NOT maintain full DHT lists locally. Instead:

1. **Tracker Maintains DHT**: Central tracker (or distributed DHT) stores peer lists
2. **Clients Query on Demand**: Each client queries DHT when needed
3. **Local Cache**: Clients cache discovered nodes, but refresh periodically

### Discovery Flow

```
┌─────────────┐
│   Node 1    │──┐
│ (Announces) │  │
└─────────────┘  │
                 │
┌─────────────┐  │    ┌──────────────┐    ┌─────────────┐
│   Node 2    │──┼───▶│  DHT/Tracker │◀───│   Client    │
│ (Announces) │  │    │  (Maintains  │    │  (Queries)  │
└─────────────┘  │    │   peer list) │    └─────────────┘
                 │    └──────────────┘           │
┌─────────────┐  │                               │
│   Node 3    │──┘                               │
│ (Announces) │                                  │
└─────────────┘                                  │
                                                 │
                                    Gets list of node IPs
```

## Client Behavior

### Current Implementation

```rust
// Client queries DHT to get current peer list
let client = DistributedInferenceClient::new(
    "llama-3.1-8b-cluster",
    "162.221.207.169",
    7001,
).await?;

// Client discovers nodes ONCE at startup
// Then caches them locally
```

**Problem**: Client won't see new nodes until it refreshes.

### Refresh Mechanisms

#### Option 1: Manual Refresh
```rust
// Refresh node list manually
client.refresh_nodes().await?;
```

#### Option 2: Periodic Auto-Refresh
```rust
// Start background refresh task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if let Err(e) = client.refresh_nodes().await {
            eprintln!("Failed to refresh nodes: {}", e);
        }
    }
});
```

#### Option 3: On-Demand Refresh (Smart)
```rust
// Refresh only when connection fails
let response = match client.process_query(...).await {
    Ok(r) => r,
    Err(_) => {
        // Connection failed, maybe new nodes available
        client.refresh_nodes().await?;
        client.process_query(...).await?
    }
};
```

## Making It Real-Time Aware

### Enhanced Client with Auto-Discovery

```rust
pub struct DistributedInferenceClient {
    discovery: Arc<NodeDiscovery>,
    nodes: Arc<RwLock<HashMap<u32, DiscoveredNode>>>,
    // ... existing fields ...
    auto_refresh: Arc<tokio::sync::RwLock<bool>>,
    refresh_interval: Duration,
}

impl DistributedInferenceClient {
    /// Create client with automatic node discovery
    pub async fn new_with_auto_refresh(
        cluster_info_hash: String,
        tracker_host: &str,
        tracker_port: u16,
        refresh_interval: Duration,
    ) -> Result<Self> {
        let client = Self::new(cluster_info_hash, tracker_host, tracker_port).await?;
        
        // Start background refresh
        let discovery = client.discovery.clone();
        let nodes = client.nodes.clone();
        let entry_exit = Arc::new((client.entry_node_id, client.exit_node_id));
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            
            loop {
                interval.tick().await;
                
                // Discover new nodes
                match discovery.discover_nodes().await {
                    Ok(discovered) => {
                        let mut cached = nodes.write().await;
                        let mut new_nodes = false;
                        
                        // Update cache, detect new nodes
                        for node in discovered {
                            let existing = cached.contains_key(&node.node_id);
                            if !existing {
                                new_nodes = true;
                                println!("[DHT] New node discovered: {} at {}", 
                                         node.node_name, node.addr);
                            }
                            cached.insert(node.node_id, node);
                        }
                        
                        if new_nodes {
                            println!("[DHT] Node list updated - {} total nodes", cached.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("[DHT] Refresh failed: {}", e);
                    }
                }
            }
        });
        
        Ok(client)
    }
}
```

## Real-Time Discovery Strategies

### Strategy 1: Polling (Current)

```rust
// Query DHT every N seconds
refresh_interval: Duration::from_secs(30)
```

**Pros**: Simple, works with any DHT
**Cons**: 0-30 second delay before seeing new nodes

### Strategy 2: Event-Driven (Better)

```rust
// DHT publishes events when peers join/leave
dht_client.subscribe_to_changes(info_hash, |event| {
    match event {
        PeerJoined(peer) => {
            // Immediately update local cache
            client.add_node(peer).await?;
        }
        PeerLeft(peer_id) => {
            // Remove from cache
            client.remove_node(peer_id).await?;
        }
    }
});
```

**Pros**: Instant updates
**Cons**: Requires DHT with pub/sub capability

### Strategy 3: Hybrid (Best)

```rust
// Combination of polling + event-driven
- Poll DHT every 60 seconds (backup)
- Subscribe to peer change events (primary)
- Refresh on connection failures (fallback)
```

## Implementation: Enhanced Client

```rust
use tokio::sync::broadcast;

pub struct DistributedInferenceClient {
    // ... existing fields ...
    node_change_tx: broadcast::Sender<NodeChangeEvent>,
}

#[derive(Debug, Clone)]
pub enum NodeChangeEvent {
    NodeJoined(DiscoveredNode),
    NodeLeft(u32),  // node_id
    NodeUpdated(DiscoveredNode),
}

impl DistributedInferenceClient {
    /// Get channel to receive node change events
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<NodeChangeEvent> {
        self.node_change_tx.subscribe()
    }
    
    /// Start background discovery with real-time updates
    pub async fn start_auto_discovery(&self, refresh_interval: Duration) {
        let discovery = self.discovery.clone();
        let nodes = self.nodes.clone();
        let tx = self.node_change_tx.clone();
        let cluster_hash = discovery.cluster_info_hash.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            let mut last_known_nodes = HashSet::new();
            
            loop {
                interval.tick().await;
                
                match discovery.discover_nodes().await {
                    Ok(current_nodes) => {
                        let current_ids: HashSet<u32> = 
                            current_nodes.iter().map(|n| n.node_id).collect();
                        
                        // Detect new nodes
                        for node in &current_nodes {
                            if !last_known_nodes.contains(&node.node_id) {
                                let _ = tx.send(NodeChangeEvent::NodeJoined(node.clone()));
                                println!("[DHT] ✅ New node joined: {} at {}", 
                                         node.node_name, node.addr);
                            }
                        }
                        
                        // Detect left nodes
                        for node_id in &last_known_nodes {
                            if !current_ids.contains(node_id) {
                                let _ = tx.send(NodeChangeEvent::NodeLeft(*node_id));
                                println!("[DHT] ❌ Node left: {}", node_id);
                            }
                        }
                        
                        // Update cache
                        let mut cached = nodes.write().await;
                        for node in current_nodes {
                            cached.insert(node.node_id, node);
                        }
                        
                        last_known_nodes = current_ids;
                    }
                    Err(e) => {
                        eprintln!("[DHT] Discovery error: {}", e);
                    }
                }
            }
        });
    }
}
```

## Usage: Real-Time Node Awareness

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Create client
    let client = DistributedInferenceClient::new(
        "llama-3.1-8b-cluster".to_string(),
        "162.221.207.169",
        7001,
    ).await?;
    
    // Start auto-discovery (refreshes every 30 seconds)
    client.start_auto_discovery(Duration::from_secs(30)).await;
    
    // Subscribe to node change events
    let mut node_events = client.subscribe_to_changes();
    
    // Monitor node changes
    tokio::spawn(async move {
        while let Ok(event) = node_events.recv().await {
            match event {
                NodeChangeEvent::NodeJoined(node) => {
                    println!("🎉 New node available: {} ({})", 
                             node.node_name, node.addr);
                }
                NodeChangeEvent::NodeLeft(node_id) => {
                    println!("⚠️  Node left: {}", node_id);
                }
                NodeChangeEvent::NodeUpdated(node) => {
                    println!("🔄 Node updated: {}", node.node_name);
                }
            }
        }
    });
    
    // Use client normally - it will automatically use new nodes
    let response = client.process_query("Hello", None, params).await?;
    
    Ok(())
}
```

## Answer to Your Questions

### Q: Are DHT lists on every client?

**Answer**: No. DHT lists are maintained by:
- **Tracker/DHT network** (central or distributed)
- **Client local cache** (temporary, refreshed periodically)

Each client queries the DHT when needed, doesn't maintain full DHT state.

### Q: Will I see new nodes joining?

**Answer**: Depends on refresh strategy:

| Strategy | See New Nodes? | Delay |
|----------|----------------|-------|
| **No refresh** | ❌ Only at startup | Never |
| **Manual refresh** | ✅ Yes, when you call it | On-demand |
| **Periodic refresh** | ✅ Yes, automatically | 30-60 seconds |
| **Event-driven** | ✅ Yes, instantly | < 1 second |

## Recommended Implementation

For production, use **hybrid approach**:

```rust
1. Auto-refresh every 30-60 seconds (backup)
2. Refresh on connection failure (fallback)
3. Event subscription if DHT supports it (primary)
4. Log all node join/leave events (visibility)
```

This ensures:
- ✅ Clients see new nodes within 30-60 seconds
- ✅ Failed connections trigger immediate refresh
- ✅ Real-time updates if DHT supports events
- ✅ Full visibility of cluster changes

## Summary

- **DHT lists**: Stored in tracker/DHT network, not on every client
- **Client cache**: Each client caches discovered nodes locally
- **New nodes**: Visible if you refresh (manual, periodic, or event-driven)
- **Real-time**: Possible with event subscriptions or frequent polling

The system can be as reactive as you want - from manual refresh to real-time event-driven updates.

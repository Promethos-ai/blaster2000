# DHT Discovery: Frequently Asked Questions

## Q: Are DHT lists on every client?

**Answer**: **No, DHT lists are NOT stored on every client.**

### How It Actually Works:

1. **DHT/Tracker Stores Peer Lists**: The tracker (or distributed DHT network) maintains a mapping:
   ```
   info_hash → [peer1_ip:port, peer2_ip:port, peer3_ip:port, ...]
   ```

2. **Clients Query DHT**: Each client queries the DHT when needed:
   ```rust
   // Client asks: "Give me all peers for this info_hash"
   let peers = dht_client.get_peers("llama-3.1-8b-cluster").await?;
   // Returns: [192.168.1.100:7001, 192.168.1.101:7001, ...]
   ```

3. **Client Local Cache**: Clients cache discovered nodes locally for performance:
   ```rust
   // Client maintains a local cache (temporary)
   HashMap<node_id, DiscoveredNode>
   ```

4. **Cache Refresh**: Client refreshes cache periodically or on-demand:
   ```rust
   client.refresh_nodes().await?;  // Re-query DHT
   ```

### Architecture Diagram:

```
┌─────────────────────────────────────────┐
│         DHT/Tracker (Central)           │
│  Maintains: info_hash → [peer_list]     │
│                                         │
│  "llama-3.1-8b-cluster" →              │
│    [192.168.1.100:7001,                │
│     192.168.1.101:7001,                │
│     192.168.1.102:7001]                │
└─────────────────────────────────────────┘
            ▲                    ▲
            │                    │
            │ Query              │ Query
            │                    │
    ┌───────┴────────┐    ┌──────┴────────┐
    │   Client A     │    │   Client B    │
    │                │    │               │
    │ Local Cache:   │    │ Local Cache:  │
    │ - node-00      │    │ - node-01     │
    │ - node-01      │    │ - node-02     │
    └────────────────┘    └───────────────┘
```

**Key Point**: The DHT/tracker is the **source of truth**. Clients only cache for performance.

---

## Q: Will I see new nodes joining?

**Answer**: **Yes, if you enable auto-refresh!**

### Current Behavior (Without Auto-Refresh):

```rust
let client = DistributedInferenceClient::new(...).await?;
// Client discovers nodes ONCE at startup
// Won't see new nodes until you manually refresh
```

**Problem**: New nodes won't be visible until you call `refresh_nodes()`

### Solution: Enable Auto-Refresh

```rust
let client = DistributedInferenceClient::new(...).await?;

// Enable automatic refresh every 30 seconds
client.start_auto_refresh(Duration::from_secs(30));
```

**What Happens:**
- Client queries DHT every 30 seconds
- Detects new nodes joining
- Detects nodes leaving
- Logs changes to console
- Updates local cache automatically

### Example Output:

```
[DHT] ✅ New node discovered: node-04 at 192.168.1.104:7001 (Shard 3)
[DHT] Cluster updated: 5 nodes active (was 4)

[DHT] ❌ Node left: 2
[DHT] Cluster updated: 4 nodes active (was 5)
```

### Refresh Strategies:

| Strategy | See New Nodes? | Delay | Code |
|----------|----------------|-------|------|
| **No refresh** | ❌ Only at startup | Never | Default |
| **Manual refresh** | ✅ Yes, on demand | When you call it | `client.refresh_nodes().await?` |
| **Auto-refresh** | ✅ Yes, automatically | 30-60 seconds | `client.start_auto_refresh(Duration::from_secs(30))` |
| **On-failure refresh** | ✅ Yes, when needed | On connection failure | Refresh if query fails |

### Recommended Setup:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let client = DistributedInferenceClient::new(
        "llama-3.1-8b-cluster".to_string(),
        "162.221.207.169",
        7001,
    ).await?;
    
    // Enable auto-refresh to see new nodes
    client.start_auto_refresh(Duration::from_secs(30));
    
    // Now you'll see new nodes automatically!
    // Console will show:
    // [DHT] ✅ New node discovered: node-05 at ...
    
    // Use client normally
    let response = client.process_query(...).await?;
    
    Ok(())
}
```

---

## How DHT Discovery Works Step-by-Step

### When a New Node Joins:

1. **Node starts up**
   ```rust
   let announcer = NodeAnnouncer::new(...)?;
   announcer.start().await?;  // Announces to DHT
   ```

2. **DHT/Tracker updates**
   ```
   DHT stores: "llama-3.1-8b-cluster" → [node1, node2, node3, NEW_NODE]
   ```

3. **Clients discover (next refresh cycle)**
   ```rust
   // Client queries DHT every 30 seconds
   let peers = dht_client.get_peers("llama-3.1-8b-cluster").await?;
   // Returns: [node1, node2, node3, NEW_NODE]  ← New node appears!
   ```

4. **Client detects new node**
   ```rust
   // Client compares:
   previous_nodes: [node1, node2, node3]
   current_nodes:  [node1, node2, node3, NEW_NODE]
   
   // Detects: NEW_NODE is new!
   println!("[DHT] ✅ New node discovered: NEW_NODE");
   ```

5. **Client updates cache**
   ```rust
   // Client adds NEW_NODE to local cache
   // Future queries can use NEW_NODE
   ```

---

## Summary

### DHT Lists:
- **NOT on every client** - Only in DHT/tracker
- **Clients query** - Get current peer list when needed
- **Local cache** - Temporary, refreshed periodically

### Seeing New Nodes:
- **Without auto-refresh**: ❌ Only at startup
- **With auto-refresh**: ✅ Automatically every 30-60 seconds
- **Manual refresh**: ✅ On-demand with `refresh_nodes()`

### Enable Auto-Refresh:
```rust
client.start_auto_refresh(Duration::from_secs(30));
```

This ensures you'll see new nodes joining automatically within 30 seconds!

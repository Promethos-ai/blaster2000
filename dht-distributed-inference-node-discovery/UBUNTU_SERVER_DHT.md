# Ubuntu Server as DHT Tracker

## Overview

The Ubuntu server (`quic_tracker_server_ai.py`) now functions as a **Distributed Hash Table (DHT)** for node discovery. It maintains a mapping of `info_hash → [peer_list]`, where peers are inference nodes that have announced themselves.

## How It Works

### 1. Node Announcement

When an inference node starts, it announces itself to the DHT:

```rust
// Node announces to DHT
let announcer = NodeAnnouncer::new(
    dht_client,
    "llama-3.1-8b-cluster",  // info_hash
    node_config,
    model_info,
    7001,  // port
)?;

announcer.start().await?;  // Announces every 60 seconds
```

**What happens on the server:**
1. Node sends `TrackerAnnounceRequest` with `info_hash` and its IP:port
2. Server's `TrackerState` stores the peer in `peers[info_hash]`
3. Server logs: `[DHT] ✅ NEW PEER registered: ... at IP:PORT`
4. Server returns list of other peers sharing the same `info_hash`

### 2. Client Discovery

Clients query the DHT to discover nodes:

```rust
// Client queries DHT
let client = DistributedInferenceClient::new(
    "llama-3.1-8b-cluster",  // info_hash
    "162.221.207.169",       // DHT server IP
    7001,                     // DHT server port
).await?;
```

**What happens on the server:**
1. Client sends `TrackerAnnounceRequest` with `event: "none"` (query only)
2. Server's `TrackerState.get_peers(info_hash)` returns peer list
3. Server logs: `[DHT] Query for '...': Returning N peers`
4. Server returns list of active peers

### 3. Peer Expiration

**Automatic cleanup** - Peers that haven't announced in 180 seconds are automatically removed:

- When a node stops announcing → Stale after 180s → Auto-expired
- Server logs: `[DHT] Expired N stale peer(s) for '...'`
- Keeps DHT clean and accurate

## DHT Features

### Peer Expiration

```python
# Peers expire after 180 seconds of inactivity
peer_expiration_seconds = 180

# Automatic cleanup on query
def get_peers(self, info_hash: str):
    self._expire_stale_peers(info_hash)  # Clean up first
    return active_peers
```

### DHT Statistics

The server tracks:
- `total_announcements`: Number of peer announcements
- `total_peer_queries`: Number of DHT lookups
- `total_expired_peers`: Number of stale peers removed
- `total_torrents`: Number of unique info_hashes
- `total_peers`: Total peers across all info_hashes

### Logging

**New peer registration:**
```
[DHT] ✅ NEW PEER registered: abc12345... for info_hash 'llama-3.1-8b-cluster...' at 192.168.1.100:7001 (Total: 5 peers)
```

**Peer update:**
```
[DHT] Updated peer abc12345... for info_hash 'llama-3.1-8b-cluster...' at 192.168.1.100:7001
```

**DHT query:**
```
[DHT] Query for 'llama-3.1-8b-cluster...': Returning 4 peers (excluded: xyz67890...)
```

**Stale peer expiration:**
```
[DHT] Expired 2 stale peer(s) for 'llama-3.1-8b-cluster...' (Remaining: 3 peers)
```

**Peer removal:**
```
[DHT] ❌ Peer removed: abc12345... from info_hash 'llama-3.1-8b-cluster...' (Remaining: 4 peers)
```

## Server Configuration

### Startup

The server starts on port 7001 (or specified port):

```bash
python3 quic_tracker_server.py 7001
```

### Logging

All DHT operations are logged to:
- Console (stdout)
- Log file: `~/quic_tracker_server.log` or `/tmp/quic_server.log`

### Peer Expiration Time

Default: **180 seconds** (3 minutes)

You can modify the expiration time in `TrackerState.__init__()`:

```python
def __init__(self, peer_expiration_seconds: int = 180):
    self.peer_expiration_seconds = peer_expiration_seconds
```

## Protocol

### Announce Request (Node → DHT)

```json
{
  "type": "TrackerAnnounceRequest",
  "info_hash": "llama-3.1-8b-cluster",
  "peer_id": "unique-node-identifier",
  "port": 7001,
  "ip": "192.168.1.100",
  "event": "started"
}
```

**Response:**
```json
{
  "interval": 60,
  "peers": [
    {"ip": "192.168.1.101", "port": 7001},
    {"ip": "192.168.1.102", "port": 7001}
  ],
  "complete": 2,
  "incomplete": 3
}
```

### Query Request (Client → DHT)

```json
{
  "type": "TrackerAnnounceRequest",
  "info_hash": "llama-3.1-8b-cluster",
  "peer_id": "client-identifier",
  "port": 0,
  "event": "none"
}
```

**Response:** Same format as announce response

## DHT State Management

### Data Structure

```python
TrackerState:
  peers: {
    "llama-3.1-8b-cluster": [
      Peer(id="node-1", ip="192.168.1.100", port=7001, last_seen=1234567890),
      Peer(id="node-2", ip="192.168.1.101", port=7001, last_seen=1234567891),
      ...
    ],
    "other-cluster": [...]
  }
```

### Operations

1. **Add/Update Peer**: `add_peer(info_hash, peer)` - Called on announce
2. **Get Peers**: `get_peers(info_hash)` - Called on query (auto-expires stale peers)
3. **Remove Peer**: `remove_peer(info_hash, peer_id)` - Called on "stopped" event
4. **Expire Stale**: `_expire_stale_peers(info_hash)` - Automatic cleanup

## Monitoring

### View DHT Activity

```bash
# Watch server logs
ssh dbertrand@162.221.207.169 'tail -f /tmp/quic_server.log | grep DHT'
```

### Check Active Peers

The server logs periodic statistics:
```
[DHT] Stats - Announcements: 150, Torrents: 1, Total Peers: 5
```

## Summary

✅ **Ubuntu server is now the DHT tracker**
- Maintains `info_hash → [peer_list]` mapping
- Handles node announcements
- Handles client queries
- Auto-expires stale peers (180s)
- Comprehensive logging

✅ **Nodes announce themselves**
- Every 60 seconds
- Server stores their IP:port

✅ **Clients discover nodes**
- Query DHT with `info_hash`
- Get list of active peers
- Auto-refresh every 30 seconds

✅ **Self-healing**
- Stale peers automatically removed
- Only active nodes in responses

The Ubuntu server at `162.221.207.169:7001` is now your **central DHT** for distributed inference node discovery!

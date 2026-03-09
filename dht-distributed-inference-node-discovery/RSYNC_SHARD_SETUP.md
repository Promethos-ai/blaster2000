# Rsync-Based Shard Management

## Overview

The system now supports downloading model shards on-demand from an rsync.net server. This solves the problem of large Llama model files by:

1. **Storing shards remotely** on rsync.net server
2. **Downloading on-demand** when a node needs its assigned shard
3. **Caching locally** to avoid re-downloading
4. **Automatic management** of cache space

## Rsync Server Configuration

### Server Details
- **Host**: `zh5605.rsync.net`
- **Username**: `zh5605`
- **Password**: `3da393f1`
- **Remote Path**: `/home/zh5605/model_shards`

### Environment Variables

Set these environment variables on each node:

```bash
export RSYNC_HOST='zh5605.rsync.net'
export RSYNC_USER='zh5605'
export RSYNC_PASSWORD='3da393f1'
export RSYNC_REMOTE_PATH='/home/zh5605/model_shards'
export RSYNC_LOCAL_CACHE='./shard_cache'  # Optional, defaults to ./shard_cache
```

## Setup Instructions

### Step 1: Install Required Tools

**On Linux/Ubuntu:**
```bash
sudo apt-get update
sudo apt-get install -y rsync sshpass
```

**On Windows (using WSL):**
```bash
wsl sudo apt-get update && sudo apt-get install -y rsync sshpass
```

### Step 2: Create and Upload Shards

Use the PowerShell script (works on Windows with WSL or Linux):

```powershell
.\setup_rsync_shards.ps1 -ModelPath "model.gguf" -NumShards 4
```

Or manually:

```bash
# 1. Create shards
python shard_llama_simple.py model.gguf ./shards --num-shards 4

# 2. Upload each shard
sshpass -p '3da393f1' rsync -avz --progress \
  shards/shard_00.gguf \
  zh5605@zh5605.rsync.net:/home/zh5605/model_shards/

sshpass -p '3da393f1' rsync -avz --progress \
  shards/shard_01.gguf \
  zh5605@zh5605.rsync.net:/home/zh5605/model_shards/
# ... repeat for all shards
```

### Step 3: Configure Nodes

Each node automatically downloads its assigned shard on first use:

```bash
# Node 0
export LLAMA_SHARD_ID=0
export LLAMA_LAYER_START=0
export LLAMA_LAYER_END=8
export RSYNC_PASSWORD='3da393f1'
python3 quic_tracker_server.py 7001

# Node 1
export LLAMA_SHARD_ID=1
export LLAMA_LAYER_START=8
export LLAMA_LAYER_END=16
export RSYNC_PASSWORD='3da393f1'
python3 quic_tracker_server.py 7001
# ... etc
```

## How It Works

### Automatic Download

1. **Node starts** with shard configuration
2. **AI processor initializes** and checks for local shard
3. **If not found**, rsync manager downloads from remote server
4. **Shard is cached** locally for future use
5. **Model loads** from cached shard

### Caching

- Shards are cached in `./shard_cache/` directory
- Cache metadata tracks which shards are downloaded
- Cache can be cleaned up: `manager.cleanup_cache(keep_recent=10)`

### Example Flow

```
1. Node 0 starts with shard_id=0
2. ai_processor.load_model() called
3. Checks: ./shard_cache/shard_00.gguf exists? No
4. Downloads: zh5605@zh5605.rsync.net:/home/zh5605/model_shards/shard_00.gguf
5. Saves to: ./shard_cache/shard_00.gguf
6. Loads model from local cache
7. Future requests use cached shard (no re-download)
```

## API Usage

### Python API

```python
from shard_rsync_manager import get_rsync_manager, RsyncConfig

# Auto-load from environment
manager = get_rsync_manager()

# Or configure manually
config = RsyncConfig(
    host='zh5605.rsync.net',
    username='zh5605',
    password='3da393f1',
    remote_path='/home/zh5605/model_shards',
    local_cache='./shard_cache'
)
manager = ShardRsyncManager(config)

# List available shards
shards = manager.list_available_shards()
print(f"Available shards: {shards}")

# Download a specific shard
shard_path = manager.download_shard('shard_00.gguf')

# Get shard for layer range (auto-downloads if needed)
shard_path = manager.get_shard_for_layer_range(
    shard_id=0,
    layer_start=0,
    layer_end=8
)

# Get cache info
info = manager.get_cache_info()
print(f"Cached: {info['cached_count']} shards, {info['total_size_mb']:.1f} MB")
```

## Security Notes

⚠️ **Important**: The password is currently stored in code/environment. For production:

1. **Use SSH keys** instead of password:
   ```bash
   ssh-keygen -t rsa -b 4096
   ssh-copy-id zh5605@zh5605.rsync.net
   ```
   Then set `use_ssh_key=True` in config

2. **Use environment variables** for password:
   ```bash
   export RSYNC_PASSWORD='your_password'
   ```

3. **Restrict file permissions**:
   ```bash
   chmod 600 ~/.ssh/id_rsa
   ```

## Troubleshooting

### Issue: "sshpass not found"
**Solution**: Install sshpass:
```bash
sudo apt-get install sshpass
# Or on macOS: brew install hudochenkov/sshpass/sshpass
```

### Issue: "Connection refused"
**Solution**: 
- Check host address: `zh5605.rsync.net`
- Verify username: `zh5605`
- Test connection: `ssh zh5605@zh5605.rsync.net`

### Issue: "Permission denied"
**Solution**:
- Check password is correct
- Verify remote path exists and is writable
- Check SSH key permissions if using keys

### Issue: "Shard not found on remote"
**Solution**:
- List remote shards: `manager.list_available_shards()`
- Verify shard naming matches expected pattern
- Upload missing shards using `setup_rsync_shards.ps1`

## Performance

- **Download speed**: Depends on network connection
- **Cache hit**: Instant (uses local file)
- **Cache miss**: Network download time
- **Typical shard size**: 1-2 GB for 8B model split 4 ways

## Cache Management

### Check Cache Status
```python
info = manager.get_cache_info()
print(f"Cached: {info['cached_count']} shards")
print(f"Total size: {info['total_size_mb']:.1f} MB")
```

### Clean Up Old Shards
```python
# Keep only 10 most recent shards
manager.cleanup_cache(keep_recent=10)
```

### Manual Cache Cleanup
```bash
rm -rf ./shard_cache/*
```




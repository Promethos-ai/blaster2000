# How to Shard Llama Model Files

## Quick Start

### If You Have Safetensors Files (Most Common)

**Step 1: Convert to GGUF**
```powershell
cd e:\rust\wireshark-smarty
.\convert_safetensors_to_gguf.ps1 -ModelDir "E:\RUST\MistralShards\models" -OutputType q4_k_m
```

**Step 2: Shard the GGUF File**
```powershell
.\shard_models.ps1 -NumShards 4
```

The shard script will automatically find the newly created GGUF file.

### If You Already Have GGUF Files

**Option 1: Automatic (Recommended)**
```powershell
cd e:\rust\wireshark-smarty
.\shard_models.ps1 -NumShards 4
```

This will:
- Find the largest `.gguf` file in `E:\RUST\MistralShards\models`
- Split it into 4 shards
- Create shard metadata and instructions

**Option 2: Specify Model File**
```powershell
.\shard_models.ps1 -ModelPath "E:\RUST\MistralShards\models\model.gguf" -NumShards 4
```

### Option 3: Use Python Script Directly
```bash
python shard_llama_simple.py "E:\RUST\MistralShards\models\model.gguf" ./shards --num-shards 4
```

## What Gets Created

The sharding process creates:

1. **Shard Metadata** (`shard_metadata.json`)
   - Layer distribution plan
   - Which shard handles which layers
   - File assignments

2. **Instructions** (`SHARDING_INSTRUCTIONS.md`)
   - How to use the shards
   - Pipeline setup instructions
   - Configuration examples

3. **Note**: The sharding tool creates **metadata** and **plans**. Actual file splitting requires:
   - Physical GGUF file manipulation (complex)
   - OR inference-time layer range limiting (recommended)

## Sharding Strategies

### Layer-wise Sharding (Default)
Splits model by transformer layers:
- **Shard 0**: Embeddings + Layers 0-7
- **Shard 1**: Layers 8-15
- **Shard 2**: Layers 16-23
- **Shard 3**: Layers 24-31 + Output Head

**Best for**: Most cases, minimal communication overhead

### Tensor Parallelism
Splits attention heads and FFN dimensions across nodes:
- All nodes process all layers
- Tensors split across nodes
- Requires synchronization at each layer

**Best for**: Fast network connections, many attention heads

### Hybrid
Combines layer and tensor parallelism:
- Groups of nodes handle different layer ranges
- Within groups, nodes split tensors

**Best for**: Large-scale distribution (8+ nodes)

## Example: 4-Node Setup

```powershell
# Step 1: Create shards
.\shard_models.ps1 -NumShards 4

# Step 2: Upload to rsync (if you have physical shard files)
# OR use inference-time sharding (recommended)

# Step 3: Configure each node
# Node 0:
export LLAMA_SHARD_ID=0
export LLAMA_LAYER_START=0
export LLAMA_LAYER_END=8

# Node 1:
export LLAMA_SHARD_ID=1
export LLAMA_LAYER_START=8
export LLAMA_LAYER_END=16

# Node 2:
export LLAMA_SHARD_ID=2
export LLAMA_LAYER_START=16
export LLAMA_LAYER_END=24

# Node 3:
export LLAMA_SHARD_ID=3
export LLAMA_LAYER_START=24
export LLAMA_LAYER_END=32
```

## Important Notes

### Physical vs. Inference-Time Sharding

**Current Implementation**: Creates sharding **metadata** (plan), not physical file splits.

**Two Approaches**:

1. **Inference-Time Sharding** (Recommended)
   - Keep full model on each node
   - Configure to process only assigned layer range
   - Simpler, no file manipulation needed

2. **Physical File Sharding** (Advanced)
   - Actually split GGUF files into separate shard files
   - Requires custom GGUF parser/manipulator
   - More complex but saves disk space

### For Physical Sharding

If you need actual file splitting, you would need to:
1. Parse GGUF format
2. Extract tensor data for specific layers
3. Create new GGUF files with subset of tensors
4. Preserve metadata correctly

This is complex and may require:
- Custom GGUF parser
- Or use existing tools that support this

## Using with Rsync

After creating shard metadata:

1. **Upload full model** (if using inference-time sharding):
   ```powershell
   .\upload_mistral_shards.ps1
   ```

2. **Configure nodes** with shard assignments:
   ```bash
   export LLAMA_SHARD_ID=0
   export LLAMA_LAYER_START=0
   export LLAMA_LAYER_END=8
   export RSYNC_PASSWORD='3da393f1'
   python3 quic_tracker_server.py 7001
   ```

3. **Nodes automatically** download model on first use and process only their assigned layers

## Troubleshooting

**Issue**: "llama-cpp-python not found"
- **Solution**: Install with `pip install llama-cpp-python`
- **Note**: Not required - tool uses heuristics if not available

**Issue**: "No GGUF files found"
- **Solution**: Check model directory path
- **Solution**: Convert model to GGUF format first if needed

**Issue**: "Sharding creates metadata but no files"
- **This is expected!** The tool creates a sharding plan, not physical file splits
- Use the metadata to configure inference-time layer range limiting



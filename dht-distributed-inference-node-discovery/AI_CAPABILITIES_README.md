# AI Capabilities Integration

## Overview

AI processing capabilities have been added to both the Rust client and Python server, with stubs ready for llama model integration.

## Components

### 1. AI Processor Stub (`ai_processor.py`)

**Location:** `wireshark-smarty/ai_processor.py`

**Purpose:** Provides a clean interface for AI processing that can be replaced with actual llama model integration.

**Key Classes:**
- `AiProcessingConfig`: Configuration for AI processing (model name, temperature, max_tokens, etc.)
- `AiProcessor`: Main processing class with stub implementations

**Key Methods:**
- `load_model()`: Load llama model (stub - returns True without loading)
- `process_query()`: Process AI query and generate response (stub - returns simulated response)
- `_generate_stub_response()`: Generate stub responses for testing

**Future Integration Points:**
```python
# In load_model():
from llama_cpp import Llama

self.model = Llama(
    model_path=self.config.model_path,
    n_ctx=self.config.context_window,
    n_gpu_layers=self.config.gpu_layers if self.config.use_gpu else 0,
    verbose=False
)

# In process_query():
result = self.model(
    prompt,
    temperature=temperature or self.config.temperature,
    max_tokens=max_tokens or self.config.max_tokens,
    top_p=top_p or self.config.top_p,
    stop=["</s>", "\n\nHuman:", "\n\nUser:"],
    echo=False
)
```

### 2. Server AI Handling

**Location:** `wireshark-smarty/quic_tracker_server_ai.py` (new file with AI support)

**Changes:**
- Added `handle_ai_request()` method to process AI queries
- Integrated `ai_processor` module
- Added AI request routing in `process_stream()`

**Message Flow:**
1. Client sends `AiRequest` JSON message
2. Server routes to `handle_ai_request()`
3. Server calls `ai_processor.process_query()`
4. Server returns `AiResponse` with answer and metadata

### 3. Client AI Function

**Location:** `quic-torrent-client-server/src/client.rs`

**Function:** `send_ai_query()`

**Usage:**
```rust
use quic_torrent_client_server::client;

let response = client::send_ai_query(
    "162.221.207.169",
    7001,
    "What is the capital of France?",
    None,  // context
    Some(0.7),  // temperature
    Some(100),  // max_tokens
    Some(0.9),  // top_p
).await?;

println!("Answer: {}", response.answer);
```

### 4. Test Integration

**Location:** `quic-torrent-client-server/src/bin/random_json_test.rs`

**Changes:**
- Added AI query test type (test_type = 3)
- Added AI success/fail tracking to `TestStats`
- Randomly selects from predefined queries

## Message Formats

### AiRequest
```json
{
  "query": "What is the capital of France?",
  "context": [
    {
      "role": "user",
      "content": "Previous message"
    },
    {
      "role": "assistant",
      "content": "Previous response"
    }
  ],
  "parameters": {
    "temperature": 0.7,
    "max_tokens": 100,
    "top_p": 0.9
  }
}
```

### AiResponse
```json
{
  "answer": "The capital of France is Paris.",
  "metadata": {
    "input_tokens": 15,
    "output_tokens": 8,
    "total_tokens": 23,
    "processing_time_ms": 150
  }
}
```

## Testing

### Run Random Test (includes AI queries)
```powershell
cd quic-torrent-client-server
.\random_json_test.ps1 -Iterations 20
```

The test will randomly send:
- TrackerAnnounceRequest
- FileRequest
- Custom JSON (unknown types)
- **AiRequest** (new)

### Manual AI Query Test
```rust
// In Rust code
let response = client::send_ai_query(
    "162.221.207.169",
    7001,
    "Hello, how are you?",
    None,
    Some(0.7),
    Some(100),
    Some(0.9),
).await?;
```

## Future Llama Integration

### Step 1: Install llama-cpp-python
```bash
pip install llama-cpp-python
# For GPU support:
CMAKE_ARGS="-DLLAMA_CUBLAS=on" pip install llama-cpp-python
```

### Step 2: Download Model
```bash
# Download a small llama model (e.g., llama-2-7b-chat.gguf)
wget https://huggingface.co/TheBloke/Llama-2-7B-Chat-GGUF/resolve/main/llama-2-7b-chat.Q4_0.gguf
```

### Step 3: Update ai_processor.py
Replace stub implementations with actual llama model calls:

```python
def load_model(self) -> bool:
    from llama_cpp import Llama
    
    if self.config.model_path is None:
        raise ValueError("Model path not specified")
    
    self.model = Llama(
        model_path=self.config.model_path,
        n_ctx=self.config.context_window,
        n_gpu_layers=self.config.gpu_layers if self.config.use_gpu else 0,
        verbose=False
    )
    self.model_loaded = True
    return True

def process_query(self, query: str, ...) -> Tuple[str, Dict]:
    if not self.model_loaded:
        self.load_model()
    
    prompt = self._build_prompt(query, context)
    start_time = time.time()
    
    result = self.model(
        prompt,
        temperature=temperature or self.config.temperature,
        max_tokens=max_tokens or self.config.max_tokens,
        top_p=top_p or self.config.top_p,
        stop=["</s>", "\n\nHuman:", "\n\nUser:"],
        echo=False
    )
    
    processing_time = int((time.time() - start_time) * 1000)
    answer = result['choices'][0]['text'].strip()
    
    metadata = {
        'input_tokens': len(self.model.tokenize(prompt.encode())),
        'output_tokens': len(self.model.tokenize(answer.encode())),
        'total_tokens': result.get('usage', {}).get('total_tokens', 0),
        'processing_time_ms': processing_time,
    }
    
    return answer, metadata
```

### Step 4: Configure Model Path
```python
# In quic_tracker_server_ai.py or main()
config = AiProcessingConfig(
    model_name="llama-2-7b-chat",
    model_path="/path/to/llama-2-7b-chat.Q4_0.gguf",
    use_gpu=True,
    gpu_layers=35,
)
ai_processor = get_ai_processor(config)
```

## Current Status

✅ **Completed:**
- AI processor stub module created
- Server AI request handling added
- Client `send_ai_query()` function added
- Test integration with random AI queries
- Message format definitions
- Documentation

⏳ **Pending:**
- Actual llama model integration (stubs ready)
- Server file needs to be updated on Ubuntu (currently truncated)
- End-to-end testing with real server

## Notes

- The AI processor currently returns stub responses for testing
- All integration points are clearly marked with comments
- The system is designed to be a drop-in replacement when llama is integrated
- Token counting is currently estimated (will be accurate with real model)





# NAT Buster 2000

A UDP NAT hole punching utility written in Rust. This tool helps establish direct peer-to-peer connections through NATs and firewalls by using various techniques including port variation, packet size variation, TTL stepping, and timing jitter.

## Features

- **Interactive CLI**: Configure parameters at runtime
- **Two Operation Modes**: 
  - `Punching`: Actively attempting to establish connection
  - `Connected`: Maintaining an established connection
- **NAT Traversal Techniques**:
  - Port variation (base port ±0, ±1, ±2)
  - Packet size variation
  - TTL stepping (255 → 128 → 64)
  - Timing jitter to avoid NAT pattern detection
- **Automatic Remapping**: Detects and adapts to peer address changes
- **Connection Monitoring**: Automatically restarts punching if connection is lost

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

Or run the compiled binary:
```bash
./target/release/blaster2000
```

The program binds to UDP port 40000 by default.

## Commands

Once running, you can use the following interactive commands:

- `ip <addr:port>` - Set the peer address to connect to (e.g., `ip 192.168.1.100:50000`)
- `size <bytes>` - Set the base packet size (minimum 8 bytes)
- `speed <ms>` - Set the base send interval in milliseconds (minimum 10ms)
- `jitter <ms>` - Set the jitter range in milliseconds (±value)
- `timeout <ms>` - Set the NAT timeout in milliseconds (minimum 5000ms)
- `show` - Display current state and configuration
- `help` - Show command list
- `quit` - Exit the program

## Usage Example

1. Start the program on both peers
2. On peer A, set the target: `ip <peer_b_ip>:<peer_b_port>`
3. On peer B, set the target: `ip <peer_a_ip>:<peer_a_port>`
4. The program will automatically attempt to establish a connection
5. Once connected, you'll see "connection success" message
6. The connection will be maintained automatically

## How It Works

1. **Initial Burst**: Sends 3 bursts of packets to multiple ports with varying sizes
2. **Punching Phase**: Continuously sends packets with jittered timing to establish NAT mapping
3. **Connection Phase**: Once bidirectional communication is detected, switches to keep-alive mode
4. **Adaptation**: Monitors for address changes and remaps automatically
5. **Recovery**: If connection is lost (timeout), automatically returns to punching mode

## Default Settings

- Packet size: 32 bytes
- Base interval: 500ms
- Jitter: ±150ms
- NAT timeout: 30000ms (30 seconds)
- Local bind: 0.0.0.0:40000

## Test Wrapper - Full Spectrum Testing

A comprehensive test wrapper is included that systematically tests different parameter combinations until a connection is established. This is useful for finding optimal parameters for your specific NAT configuration.

### Running the Test Wrapper

```bash
cargo run --release --bin test_wrapper -- <target_ip:port>
```

Example:
```bash
cargo run --release --bin test_wrapper -- 192.168.1.100:50000
```

Or run the compiled binary:
```bash
./target/release/test_wrapper 192.168.1.100:50000
```

### Test Wrapper Features

- **Comprehensive Parameter Sweep**: Tests all combinations of:
  - Packet sizes: 8, 16, 32, 64, 128 bytes
  - Base intervals: 100, 250, 500, 1000, 2000 ms
  - Jitter values: 0, 50, 100, 150, 200 ms
  - NAT timeouts: 10000, 20000, 30000, 60000 ms
- **Full Logging**: Detailed logs for every step including:
  - Test cycle progress
  - Parameter combinations being tested
  - Packet send/receive events
  - Connection state changes
  - TTL changes
  - Remapping events
  - Periodic status updates
- **Automatic Success Detection**: Stops immediately when connection is established
- **Progress Tracking**: Shows current test number and estimated completion time

### Logging Levels

Control logging verbosity with the `RUST_LOG` environment variable:

```bash
# Info level (default) - shows test progress and important events
RUST_LOG=info cargo run --release --bin test_wrapper -- 192.168.1.100:50000

# Debug level - shows detailed packet and state information
RUST_LOG=debug cargo run --release --bin test_wrapper -- 192.168.1.100:50000

# Trace level - shows all events including individual packet sends
RUST_LOG=trace cargo run --release --bin test_wrapper -- 192.168.1.100:50000
```

### Test Wrapper Output

When a successful connection is established, the wrapper displays:
- The test cycle number where success occurred
- The working parameter combination
- Total time taken

Example output:
```
╔════════════════════════════════════════════════════════════╗
║ *** SUCCESS ***
║ Connection established on test cycle #42
║ Working parameters:
║   Packet size: 32 bytes
║   Base interval: 500 ms
║   Jitter: 150 ms
║   NAT timeout: 30000 ms
║ Total time: 630.5 seconds
╚════════════════════════════════════════════════════════════╝
```

## Notes

- The tool uses non-blocking I/O for responsive operation
- Packet sequence numbers are included in the payload for tracking
- The fake session header (0x12 0x34) helps identify valid packets
- TTL values are stepped over time to work with different NAT types
- The test wrapper uses port 40001 to avoid conflicts with the main binary (port 40000)

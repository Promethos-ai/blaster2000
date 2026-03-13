# Bug Report: h3-util "async fn resumed after completion" panic

**Copy the content below to create a GitHub issue at:** https://github.com/youyuanwu/tonic-h3/issues/new

---

## Title

`async fn` resumed after completion panic in h3-util client_conn.rs:136

## Description

### Summary

When using tonic-h3 as a gRPC-over-QUIC **client** to connect to a server and call a streaming RPC (`complete_stream`), the application panics with:

```
thread 'tokio-rt-worker' panicked at ...\h3-util-0.0.5\src\client_conn.rs:136:56:
`async fn` resumed after completion
```

### Environment

- **h3-util**: 0.0.5 (features: quinn)
- **tonic-h3**: 0.0.5 (features: quinn)
- **quinn**: 0.11
- **tonic**: 0.14
- **OS**: Windows 10
- **Rust**: stable

### Reproduction

1. Run a gRPC server with HTTP/3/QUIC (e.g. Feb17 grpc_server with `--quic`).
2. Run a client that uses tonic-h3 to connect via `H3Connection::new` / `H3QuinnConnector`.
3. Call a streaming RPC (e.g. `complete_stream`) that returns a server stream.
4. Panic occurs during or after the stream is consumed.

### Code Path

The client setup:

```rust
let connector = tonic_h3::quinn::H3QuinnConnector::new(uri.clone(), server_name, endpoint.clone());
let channel: tonic_h3::H3Channel<_> = h3_util::client::H3Connection::new(connector, uri);
let client = LlmClient::new(channel);
// ...
client.complete_stream(request).await  // streaming response
```

### Root Cause (Hypothesis)

The panic "`async fn` resumed after completion" indicates a `Future` is being polled after it has already returned `Poll::Ready`, which violates the Future contract. In `h3-util/src/client_conn.rs`, likely culprits include:

1. **`send_request_inner`**: The `body_fut` / `poll_fn` pattern—when `body_fut` returns `Pending`, the outer future completes with `None` and spawns a task that awaits `body_fut`. If `body_fut` is polled from multiple places or the spawn races with completion, a double-poll could occur.

2. **`RequestSender::poll_ready`**: The `make_send_request_fut` future is polled until `Ready`. If the Tower layer or executor polls again after the future is dropped/replaced, or if the spawned driver task interacts incorrectly with the connection future, a completed future could be polled again.

3. **Streaming responses**: With gRPC streaming, the connection stays open and the driver runs in the background. Lifecycle or waker interactions between the driver task and the main connection could cause a future to be resumed after completion.

### Workaround

Using TCP instead of QUIC for the gRPC connection (plain `tonic` with `Channel::from_shared("http://...")`) avoids the panic. The issue is specific to the QUIC/HTTP3 client path.

### Possible Fix (applied locally)

In `poll_ready`, when the connection future returns `Err(e)`, we must clear `make_send_request_fut = None` in the `Err` branch. Otherwise the next `poll_ready` call will poll the completed future again, causing "async fn resumed after completion". The `Ok` branch already cleared it; the `Err` branch did not.

```rust
Err(e) => {
    self.make_send_request_fut = None;  // <-- add this
    Err(e)
}
```

Ember applies this via `[patch.crates-io]` pointing to `vendor-tonic-h3/h3-util`.

### Additional Context

- Occurs with `complete_stream` (server streaming) RPC.
- Does not occur with TCP (non-QUIC) gRPC.
- Reproducible and consistent when using QUIC client.

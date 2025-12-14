use std::env;
use std::net::{UdpSocket, SocketAddr, IpAddr};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum Mode {
    Punching,
    Connected,
}

#[derive(Clone, Debug)]
struct State {
    base_target: Option<SocketAddr>,
    active_target: Option<SocketAddr>,
    packet_size: usize,
    base_interval_ms: u64,
    jitter_ms: u64,
    nat_timeout_ms: u64,
    connected: bool,
    mode: Mode,
    last_inbound: Option<Instant>,
    first_mapping_time: Option<Instant>,
    pending_remap_addr: Option<SocketAddr>,
    pending_remap_hits: u8,
    remote_interval_ms: u64,
}

#[derive(Clone, Debug)]
struct TestParams {
    packet_size: usize,
    base_interval_ms: u64,
    jitter_ms: u64,
    nat_timeout_ms: u64,
}

fn next_rand(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    *state
}

fn build_payload(seq: u64, size: usize) -> Vec<u8> {
    let min_size = if size < 8 { 8 } else { size };
    let mut buf = vec![0u8; min_size];

    // fake session header
    buf[0] = 0x12;
    buf[1] = 0x34;

    // sequence number
    let seq_bytes = seq.to_be_bytes();
    let copy_len = std::cmp::min(8, min_size.saturating_sub(2));
    for i in 0..copy_len {
        buf[2 + i] = seq_bytes[i];
    }

    buf
}

fn send_punches(
    socket: &UdpSocket,
    base_target: SocketAddr,
    packet_size: usize,
    seq: &mut u64,
    rng_state: &mut u64,
) {
    let ip: IpAddr = base_target.ip();
    let port = base_target.port();

    let mut ports: Vec<u16> = Vec::new();
    ports.push(port);
    if port < 65535 {
        ports.push(port + 1);
    }
    if port > 0 {
        ports.push(port - 1);
    }
    if port <= 65533 {
        ports.push(port + 2);
    }
    if port > 1 {
        ports.push(port - 2);
    }

    ports.sort();
    ports.dedup();

    let base = if packet_size < 8 { 8 } else { packet_size };
    let smaller = if base > 12 { base - 4 } else { base };
    let bigger = base + 4;
    let size_pattern = [smaller, base, bigger];

    for (i, p) in ports.iter().enumerate() {
        let size = {
            let mut s = size_pattern[i % size_pattern.len()];
            if s < 8 {
                s = 8;
            }
            s
        };
        let payload = build_payload(*seq, size);
        *seq = seq.wrapping_add(1);
        let addr = SocketAddr::new(ip, *p);
        let _ = socket.send_to(&payload, addr);

        // first packet split trick: small then large
        let small_size = 8;
        let small_payload = build_payload(*seq, small_size);
        *seq = seq.wrapping_add(1);
        let _ = socket.send_to(&small_payload, addr);

        // a little random spacing inside the punch burst
        let r = next_rand(rng_state);
        let pause = 5 + (r % 15);
        thread::sleep(Duration::from_millis(pause));
    }
}

fn run_test_cycle(
    socket: &UdpSocket,
    target: SocketAddr,
    params: &TestParams,
    test_id: usize,
) -> bool {
    log::info!("=== Starting Test Cycle #{} ===", test_id);
    log::info!("Parameters: size={} bytes, speed={} ms, jitter={} ms, timeout={} ms",
               params.packet_size, params.base_interval_ms, params.jitter_ms, params.nat_timeout_ms);
    
    let state = Arc::new(Mutex::new(State {
        base_target: Some(target),
        active_target: Some(target),
        packet_size: params.packet_size,
        base_interval_ms: params.base_interval_ms,
        jitter_ms: params.jitter_ms,
        nat_timeout_ms: params.nat_timeout_ms,
        connected: false,
        mode: Mode::Punching,
        last_inbound: None,
        first_mapping_time: None,
        pending_remap_addr: None,
        pending_remap_hits: 0,
        remote_interval_ms: 0,
    }));

    let start_time = Instant::now();
    let test_duration = Duration::from_secs(30); // 30 seconds per test
    let mut current_ttl: u32 = 255;
    let mut last_send_time = Instant::now();
    let mut next_interval_ms: u64 = params.base_interval_ms;

    let mut seq: u64 = 1;
    let mut rng_state: u64 = 0x1234_5678_9abc_def0;
    let mut burst_done = false;
    let mut packets_sent = 0u64;
    let mut packets_received = 0u64;

    log::debug!("Test cycle #{}: Starting main loop", test_id);

    loop {
        let now = Instant::now();

        // Check if test duration exceeded
        if now.duration_since(start_time) > test_duration {
            log::warn!("Test cycle #{}: Timeout after {} seconds, no connection established", 
                      test_id, test_duration.as_secs());
            return false;
        }

        // TTL stepping
        let elapsed_secs = now.duration_since(start_time).as_secs();
        let desired_ttl: u32 = if elapsed_secs < 3 {
            255
        } else if elapsed_secs < 6 {
            128
        } else {
            64
        };
        if desired_ttl != current_ttl {
            log::debug!("Test cycle #{}: Changing TTL from {} to {}", test_id, current_ttl, desired_ttl);
            let _ = socket.set_ttl(desired_ttl);
            current_ttl = desired_ttl;
        }

        // receive path
        let mut buf = [0u8; 2048];
        if let Ok((len, src)) = socket.recv_from(&mut buf) {
            packets_received += 1;
            let now_in = Instant::now();
            log::info!("Test cycle #{}: Received packet #{} from {} ({} bytes)", 
                      test_id, packets_received, src, len);

            let mut do_open_burst = false;
            let mut connection_established = false;

            {
                let mut s = state.lock().unwrap();
                let prev_last = s.last_inbound;
                s.last_inbound = Some(now_in);

                if let Some(prev) = prev_last {
                    let delta_ms = now_in
                        .duration_since(prev)
                        .as_millis() as u64;
                    if delta_ms > 0 {
                        s.remote_interval_ms = delta_ms;
                        log::debug!("Test cycle #{}: Remote interval detected: {} ms", test_id, delta_ms);
                    }
                }

                if !s.connected {
                    s.connected = true;
                    s.mode = Mode::Connected;
                    s.active_target = Some(src);
                    s.first_mapping_time = Some(now_in);
                    s.pending_remap_addr = None;
                    s.pending_remap_hits = 0;
                    connection_established = true;
                    do_open_burst = true;
                    burst_done = true;
                    log::info!("Test cycle #{}: *** CONNECTION SUCCESS *** Mapped external address: {}", 
                              test_id, src);
                } else {
                    if let Some(active) = s.active_target {
                        if active != src {
                            log::debug!("Test cycle #{}: Address change detected: {} -> {}", 
                                       test_id, active, src);
                            let allow_window = match s.first_mapping_time {
                                Some(t0) => now_in.duration_since(t0).as_secs() >= 5,
                                None => true,
                            };
                            if allow_window {
                                if let Some(cand) = s.pending_remap_addr {
                                    if cand == src {
                                        s.pending_remap_hits = s.pending_remap_hits.saturating_add(1);
                                        log::debug!("Test cycle #{}: Remap candidate hit count: {}", 
                                                   test_id, s.pending_remap_hits);
                                        if s.pending_remap_hits >= 2 {
                                            s.active_target = Some(src);
                                            s.pending_remap_addr = None;
                                            s.pending_remap_hits = 0;
                                            log::info!("Test cycle #{}: Remapped active target to {}", 
                                                      test_id, src);
                                        }
                                    } else {
                                        s.pending_remap_addr = Some(src);
                                        s.pending_remap_hits = 1;
                                        log::debug!("Test cycle #{}: New remap candidate: {}", test_id, src);
                                    }
                                } else {
                                    s.pending_remap_addr = Some(src);
                                    s.pending_remap_hits = 1;
                                    log::debug!("Test cycle #{}: New remap candidate: {}", test_id, src);
                                }
                            }
                        }
                    } else {
                        s.active_target = Some(src);
                    }
                }
            }

            if connection_established {
                return true;
            }

            if do_open_burst {
                let target_opt = {
                    let s = state.lock().unwrap();
                    s.active_target
                };
                if let Some(target) = target_opt {
                    log::debug!("Test cycle #{}: Sending connection burst to {}", test_id, target);
                    for i in 0..4 {
                        let size = {
                            let s = state.lock().unwrap();
                            s.packet_size
                        };
                        let payload = build_payload(seq, size);
                        seq = seq.wrapping_add(1);
                        packets_sent += 1;
                        let _ = socket.send_to(&payload, target);
                        log::trace!("Test cycle #{}: Burst packet {} sent ({} bytes)", test_id, i + 1, size);
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }

            let _ = socket.send_to(&buf[..len], src);
            log::trace!("Test cycle #{}: Echoed packet back to {}", test_id, src);
        }

        // NAT timeout check
        {
            let mut s = state.lock().unwrap();
            if s.connected {
                if let Some(last_in) = s.last_inbound {
                    let silent_ms = now
                        .duration_since(last_in)
                        .as_millis() as u64;
                    if silent_ms > s.nat_timeout_ms {
                        log::warn!("Test cycle #{}: Connection lost (silent for {} ms), restarting punching", 
                                  test_id, silent_ms);
                        s.connected = false;
                        s.mode = Mode::Punching;
                        s.active_target = s.base_target;
                        s.first_mapping_time = None;
                        s.pending_remap_addr = None;
                        s.pending_remap_hits = 0;
                        burst_done = false;
                    }
                }
            }
        }

        // send path
        let snapshot = {
            let s = state.lock().unwrap();
            s.clone()
        };

        match snapshot.mode {
            Mode::Punching => {
                if let Some(base_target) = snapshot.base_target {
                    if !burst_done {
                        log::info!("Test cycle #{}: Sending initial punch bursts", test_id);
                        for burst_num in 0..3 {
                            log::debug!("Test cycle #{}: Punch burst {} of 3", test_id, burst_num + 1);
                            send_punches(
                                &socket,
                                base_target,
                                snapshot.packet_size,
                                &mut seq,
                                &mut rng_state,
                            );
                            packets_sent += 5; // approximate
                        }
                        burst_done = true;
                        last_send_time = now;
                        next_interval_ms = snapshot.base_interval_ms;
                        log::debug!("Test cycle #{}: Initial bursts complete, starting periodic sends", test_id);
                    }

                    let elapsed_ms = now
                        .duration_since(last_send_time)
                        .as_millis() as u64;
                    if elapsed_ms >= next_interval_ms {
                        log::trace!("Test cycle #{}: Sending periodic punch (elapsed: {} ms, interval: {} ms)", 
                                   test_id, elapsed_ms, next_interval_ms);
                        send_punches(
                            &socket,
                            base_target,
                            snapshot.packet_size,
                            &mut seq,
                            &mut rng_state,
                        );
                        packets_sent += 5; // approximate

                        let base = if snapshot.base_interval_ms < 10 {
                            10
                        } else {
                            snapshot.base_interval_ms
                        };
                        let jitter = if snapshot.jitter_ms > base {
                            base
                        } else {
                            snapshot.jitter_ms
                        };
                        let jitter_range = (2 * jitter + 1) as i64;
                        let r = next_rand(&mut rng_state);
                        let offset = if jitter_range > 0 {
                            (r % (jitter_range as u64)) as i64 - jitter as i64
                        } else {
                            0
                        };
                        let mut interval = base as i64 + offset;
                        if interval < 5 {
                            interval = 5;
                        }
                        next_interval_ms = interval as u64;
                        last_send_time = now;
                        log::trace!("Test cycle #{}: Next interval: {} ms (base: {}, jitter: {}, offset: {})", 
                                   test_id, next_interval_ms, base, jitter, offset);
                    }
                }
            }
            Mode::Connected => {
                if let Some(target) = snapshot.active_target {
                    let elapsed_ms = now
                        .duration_since(last_send_time)
                        .as_millis() as u64;
                    let keep_interval = if snapshot.remote_interval_ms > 0 {
                        snapshot.remote_interval_ms.min(snapshot.base_interval_ms)
                    } else {
                        snapshot.base_interval_ms
                    };
                    if elapsed_ms >= keep_interval {
                        let payload = build_payload(seq, snapshot.packet_size);
                        seq = seq.wrapping_add(1);
                        packets_sent += 1;
                        let _ = socket.send_to(&payload, target);
                        log::trace!("Test cycle #{}: Keep-alive sent to {} (interval: {} ms)", 
                                   test_id, target, keep_interval);
                        last_send_time = now;
                    }
                }
            }
        }

        // Periodic status log
        if packets_sent % 50 == 0 && packets_sent > 0 {
            let elapsed = now.duration_since(start_time);
            log::info!("Test cycle #{}: Status - Sent: {}, Received: {}, Elapsed: {:.1}s, Connected: {}", 
                      test_id, packets_sent, packets_received, elapsed.as_secs_f64(), snapshot.connected);
        }

        thread::sleep(Duration::from_millis(5));
    }
}

fn generate_test_combinations() -> Vec<TestParams> {
    let mut combinations = Vec::new();

    // Packet sizes: 8, 16, 32, 64, 128 bytes
    let sizes = vec![8, 16, 32, 64, 128];
    
    // Base intervals: 100, 250, 500, 1000, 2000 ms
    let intervals = vec![100, 250, 500, 1000, 2000];
    
    // Jitter: 0, 50, 100, 150, 200 ms
    let jitters = vec![0, 50, 100, 150, 200];
    
    // Timeouts: 10000, 20000, 30000, 60000 ms
    let timeouts = vec![10000, 20000, 30000, 60000];

    log::info!("Generating test parameter combinations...");
    
    for &size in &sizes {
        for &interval in &intervals {
            for &jitter in &jitters {
                for &timeout in &timeouts {
                    // Skip invalid combinations (jitter shouldn't exceed interval)
                    if jitter <= interval {
                        combinations.push(TestParams {
                            packet_size: size,
                            base_interval_ms: interval,
                            jitter_ms: jitter,
                            nat_timeout_ms: timeout,
                        });
                    }
                }
            }
        }
    }

    log::info!("Generated {} test combinations", combinations.len());
    combinations
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_secs()
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <target_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:50000", args[0]);
        std::process::exit(1);
    }

    let target = match SocketAddr::from_str(&args[1]) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Invalid address format: {}", e);
            eprintln!("Expected format: <ip>:<port>");
            std::process::exit(1);
        }
    };

    log::info!("========================================");
    log::info!("NAT Buster 2000 - Full Spectrum Test");
    log::info!("========================================");
    log::info!("Target: {}", target);
    log::info!("Starting comprehensive parameter sweep...");
    log::info!("");

    let local_bind = "0.0.0.0:40001"; // Different port to avoid conflicts
    let socket = match UdpSocket::bind(local_bind) {
        Ok(s) => {
            log::info!("Bound to {}", local_bind);
            s
        }
        Err(e) => {
            log::error!("Failed to bind to {}: {}", local_bind, e);
            std::process::exit(1);
        }
    };

    socket
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");
    let _ = socket.set_ttl(255);

    let combinations = generate_test_combinations();
    let total_tests = combinations.len();
    log::info!("Total test cycles to run: {}", total_tests);
    log::info!("Estimated time: ~{} minutes (30 seconds per test)", 
               (total_tests * 30) / 60);
    log::info!("");
    log::info!("Press Ctrl+C to stop");
    log::info!("");

    let start_time = Instant::now();

    for (idx, params) in combinations.iter().enumerate() {
        let test_num = idx + 1;
        log::info!("");
        log::info!("╔════════════════════════════════════════════════════════════╗");
        log::info!("║ Test {}/{}", test_num, total_tests);
        log::info!("╚════════════════════════════════════════════════════════════╝");
        
        let connected = run_test_cycle(&socket, target, params, test_num);
        
        if connected {
            let elapsed = Instant::now().duration_since(start_time);
            log::info!("");
            log::info!("╔════════════════════════════════════════════════════════════╗");
            log::info!("║ *** SUCCESS ***");
            log::info!("║ Connection established on test cycle #{}", test_num);
            log::info!("║ Working parameters:");
            log::info!("║   Packet size: {} bytes", params.packet_size);
            log::info!("║   Base interval: {} ms", params.base_interval_ms);
            log::info!("║   Jitter: {} ms", params.jitter_ms);
            log::info!("║   NAT timeout: {} ms", params.nat_timeout_ms);
            log::info!("║ Total time: {:.1} seconds", elapsed.as_secs_f64());
            log::info!("╚════════════════════════════════════════════════════════════╝");
            return;
        } else {
            log::warn!("Test cycle #{} failed to establish connection", test_num);
        }
    }

    let elapsed = Instant::now().duration_since(start_time);
    log::error!("");
    log::error!("╔════════════════════════════════════════════════════════════╗");
    log::error!("║ *** FAILURE ***");
    log::error!("║ All {} test cycles completed without establishing connection", total_tests);
    log::error!("║ Total time: {:.1} seconds", elapsed.as_secs_f64());
    log::error!("╚════════════════════════════════════════════════════════════╝");
}

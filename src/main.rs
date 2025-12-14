use std::io::{BufRead, stdin};
use std::net::{UdpSocket, SocketAddr, IpAddr};
use std::process;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Mode {
    Punching,
    Connected,
}

#[derive(Clone)]
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

fn main() {
    let state = Arc::new(Mutex::new(State {
        base_target: None,
        active_target: None,
        packet_size: 32,
        base_interval_ms: 500,
        jitter_ms: 150,
        nat_timeout_ms: 30000,
        connected: false,
        mode: Mode::Punching,
        last_inbound: None,
        first_mapping_time: None,
        pending_remap_addr: None,
        pending_remap_hits: 0,
        remote_interval_ms: 0,
    }));

    let local_bind = "0.0.0.0:40000";
    let socket = UdpSocket::bind(local_bind).expect("bind failed");
    socket
        .set_nonblocking(true)
        .expect("nonblock failed");
    let _ = socket.set_ttl(255);

    let input_state = state.clone();
    thread::spawn(move || {
        let stdin = stdin();
        let mut lines = stdin.lock().lines();

        loop {
            if let Some(Ok(line)) = lines.next() {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "ip" => {
                        if parts.len() != 2 {
                            println!("usage: ip <addr:port>");
                            continue;
                        }
                        match SocketAddr::from_str(parts[1]) {
                            Ok(addr) => {
                                let mut s = input_state.lock().unwrap();
                                s.base_target = Some(addr);
                                s.active_target = Some(addr);
                                s.connected = false;
                                s.mode = Mode::Punching;
                                s.last_inbound = None;
                                s.first_mapping_time = None;
                                s.pending_remap_addr = None;
                                s.pending_remap_hits = 0;
                                println!("base target set to {}", addr);
                            }
                            Err(_) => {
                                println!("invalid address");
                            }
                        }
                    }
                    "size" => {
                        if parts.len() != 2 {
                            println!("usage: size <bytes>");
                            continue;
                        }
                        if let Ok(v) = parts[1].parse::<usize>() {
                            let mut s = input_state.lock().unwrap();
                            s.packet_size = if v < 8 { 8 } else { v };
                            println!("packet size set to {}", s.packet_size);
                        } else {
                            println!("invalid number");
                        }
                    }
                    "speed" => {
                        if parts.len() != 2 {
                            println!("usage: speed <ms>");
                            continue;
                        }
                        if let Ok(v) = parts[1].parse::<u64>() {
                            let mut s = input_state.lock().unwrap();
                            s.base_interval_ms = if v < 10 { 10 } else { v };
                            println!("base interval set to {} ms", s.base_interval_ms);
                        } else {
                            println!("invalid number");
                        }
                    }
                    "jitter" => {
                        if parts.len() != 2 {
                            println!("usage: jitter <ms>");
                            continue;
                        }
                        if let Ok(v) = parts[1].parse::<u64>() {
                            let mut s = input_state.lock().unwrap();
                            s.jitter_ms = v;
                            println!("jitter range set to ±{} ms", s.jitter_ms);
                        } else {
                            println!("invalid number");
                        }
                    }
                    "timeout" => {
                        if parts.len() != 2 {
                            println!("usage: timeout <ms>");
                            continue;
                        }
                        if let Ok(v) = parts[1].parse::<u64>() {
                            let mut s = input_state.lock().unwrap();
                            s.nat_timeout_ms = if v < 5000 { 5000 } else { v };
                            println!("nat timeout set to {} ms", s.nat_timeout_ms);
                        } else {
                            println!("invalid number");
                        }
                    }
                    "show" => {
                        let s = input_state.lock().unwrap();
                        println!("Nat Buster 2000 state:");
                        println!("base_target: {:?}", s.base_target);
                        println!("active_target: {:?}", s.active_target);
                        println!("packet_size: {}", s.packet_size);
                        println!("base_interval_ms: {}", s.base_interval_ms);
                        println!("jitter_ms: {}", s.jitter_ms);
                        println!("nat_timeout_ms: {}", s.nat_timeout_ms);
                        println!("connected: {}", s.connected);
                        println!("mode: {:?}", s.mode as u8);
                        println!("remote_interval_ms: {}", s.remote_interval_ms);
                    }
                    "help" => {
                        println!("commands:");
                        println!("ip <addr:port>   set peer address");
                        println!("size <bytes>     set base packet size");
                        println!("speed <ms>       set base interval");
                        println!("jitter <ms>      set jitter range");
                        println!("timeout <ms>     set nat timeout");
                        println!("show             show state");
                        println!("help             show this help");
                        println!("quit             exit");
                    }
                    "quit" => {
                        println!("exiting");
                        process::exit(0);
                    }
                    _ => {
                        println!("unknown command, type 'help' for list");
                    }
                }
            }
        }
    });

    println!("Nat Buster 2000 ready");
    println!("Commands: ip <addr:port>, size <bytes>, speed <ms>, jitter <ms>, timeout <ms>, show, help");

    let start_time = Instant::now();
    let mut current_ttl: u32 = 255;
    let mut last_send_time = Instant::now();
    let mut next_interval_ms: u64 = {
        let s = state.lock().unwrap();
        s.base_interval_ms
    };

    let mut seq: u64 = 1;
    let mut rng_state: u64 = 0x1234_5678_9abc_def0;
    let mut burst_done = false;

    loop {
        let now = Instant::now();

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
            let _ = socket.set_ttl(desired_ttl);
            current_ttl = desired_ttl;
        }

        // receive path
        let mut buf = [0u8; 2048];
        if let Ok((len, src)) = socket.recv_from(&mut buf) {
            let now_in = Instant::now();
            println!("inbound from {}", src);

            let mut do_open_burst = false;

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
                    }
                }

                if !s.connected {
                    s.connected = true;
                    s.mode = Mode::Connected;
                    s.active_target = Some(src);
                    s.first_mapping_time = Some(now_in);
                    s.pending_remap_addr = None;
                    s.pending_remap_hits = 0;
                    println!("connection success, mapped external {}", src);
                    do_open_burst = true;
                    burst_done = true;
                } else {
                    if let Some(active) = s.active_target {
                        if active != src {
                            let allow_window = match s.first_mapping_time {
                                Some(t0) => now_in.duration_since(t0).as_secs() >= 5,
                                None => true,
                            };
                            if allow_window {
                                if let Some(cand) = s.pending_remap_addr {
                                    if cand == src {
                                        s.pending_remap_hits = s.pending_remap_hits.saturating_add(1);
                                        if s.pending_remap_hits >= 2 {
                                            s.active_target = Some(src);
                                            s.pending_remap_addr = None;
                                            s.pending_remap_hits = 0;
                                            println!("remapped active target to {}", src);
                                        }
                                    } else {
                                        s.pending_remap_addr = Some(src);
                                        s.pending_remap_hits = 1;
                                    }
                                } else {
                                    s.pending_remap_addr = Some(src);
                                    s.pending_remap_hits = 1;
                                }
                            }
                        }
                    } else {
                        s.active_target = Some(src);
                    }
                }
            }

            if do_open_burst {
                let target_opt = {
                    let s = state.lock().unwrap();
                    s.active_target
                };
                if let Some(target) = target_opt {
                    for _ in 0..4 {
                        let size = {
                            let s = state.lock().unwrap();
                            s.packet_size
                        };
                        let payload = build_payload(seq, size);
                        seq = seq.wrapping_add(1);
                        let _ = socket.send_to(&payload, target);
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }

            let _ = socket.send_to(&buf[..len], src);
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
                        println!("connection lost, restarting punching");
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
                        for _ in 0..3 {
                            send_punches(
                                &socket,
                                base_target,
                                snapshot.packet_size,
                                &mut seq,
                                &mut rng_state,
                            );
                        }
                        burst_done = true;
                        last_send_time = now;
                        next_interval_ms = snapshot.base_interval_ms;
                    }

                    let elapsed_ms = now
                        .duration_since(last_send_time)
                        .as_millis() as u64;
                    if elapsed_ms >= next_interval_ms {
                        send_punches(
                            &socket,
                            base_target,
                            snapshot.packet_size,
                            &mut seq,
                            &mut rng_state,
                        );

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
                        let _ = socket.send_to(&payload, target);
                        last_send_time = now;
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

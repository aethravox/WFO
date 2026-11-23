use super::*;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;

const AUTH_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_AUTH_ATTEMPTS_PER_IP: usize = 5;

#[derive(Default)]
pub(crate) struct AuthRateLimiter {
    attempts: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
}

impl AuthRateLimiter {
    pub(crate) fn allow_attempt_at(&self, ip: IpAddr, now: Instant) -> bool {
        let Ok(mut attempts) = self.attempts.lock() else {
            eprintln!("[wfo-server] authentication rate limiter lock poisoned");
            return false;
        };
        attempts.retain(|_, times| {
            times.retain(|time| now.saturating_duration_since(*time) < AUTH_ATTEMPT_WINDOW);
            !times.is_empty()
        });
        let times = attempts.entry(ip).or_default();
        if times.len() >= MAX_AUTH_ATTEMPTS_PER_IP {
            return false;
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn auth_attempt_limit_is_per_ip_and_expires_after_one_minute() {
                let limiter = AuthRateLimiter::default();
                let now = Instant::now();
                let ip = IpAddr::from([192, 0, 2, 1]);
                for offset in 0..MAX_AUTH_ATTEMPTS_PER_IP {
                    assert!(limiter.allow_attempt_at(
                        ip,
                        now + Duration::from_secs(offset as u64)
                    ));
                }
                assert!(!limiter.allow_attempt_at(ip, now + Duration::from_secs(10)));
                assert!(limiter.allow_attempt_at(
                    IpAddr::from([192, 0, 2, 2]),
                    now + Duration::from_secs(10)
                ));
                assert!(limiter.allow_attempt_at(
                    ip,
                    now + AUTH_ATTEMPT_WINDOW + Duration::from_secs(1)
                ));
            }
        }
        times.push_back(now);
        true
    }
}

pub(crate) fn register_player(state: &Arc<Mutex<WorldState>>, sink: ClientSink) -> String {
    let mut w = state.lock().unwrap();
    let pid = w.gen_id("hero");
    let r1 = w.next_rand();
    let ang = r1 * 6.283185;
    let sx = cos_approx(ang) * 4.0;
    let sz = sin_approx(ang) * 4.0;
    let now = Instant::now();
    w.players.insert(
        pid.clone(),
        ServerPlayer {
            snap: PlayerSnapshot {
                id: pid.clone(),
                name: String::from("Traveler"),
                class_id: ClassId::Vanguard,
                x: sx,
                y: ground_height_at(sx, sz),
                z: sz,
                yaw: 3.14159,
                moving: false,
                hp: ClassId::Vanguard.base_hp(),
                max_hp: ClassId::Vanguard.base_hp(),
                mp: ClassId::Vanguard.base_mp(),
                max_mp: ClassId::Vanguard.base_mp(),
                level: 1,
                xp: 0,
                xp_next: 100,
                gold: 0,
                rank_points: 0,
                attack_tick: 0,
                shield_hp: 0,
                haste: false,
            },
            movement_budget: MovementBudget::new(now),
            account_id: None,
            authenticated: false,
            auth_attempts: 0,
            joined: false,
            in_lobby: false,
            room_id: String::new(),
            duel: None,
            sink,
            skill_cd_until: [now, now, now],
            shield_expires_at: now,
            haste_expires_at: now,
            dodge_cd_until: now,
            jump_velocity: 0.0,
            companion: None,
        },
    );
    pid
}

pub(crate) fn remove_player(state: &Arc<Mutex<WorldState>>, pid: &str) {
    let mut w = state.lock().unwrap();
    w.persist_account(pid);
    let was_joined = w.players.remove(pid).map(|p| p.joined).unwrap_or(false);
    if was_joined {
        w.broadcast(
            &ServerPacket::PlayerLeft {
                id: pid.to_string(),
            },
            None,
        );
    }
}

pub(crate) fn handle_client_packet(
    state: &Arc<Mutex<WorldState>>,
    accounts: &AccountStore,
    auth_rate_limiter: &AuthRateLimiter,
    peer_ip: IpAddr,
    player_id: &str,
    packet: ClientPacket,
) -> bool {
    let auth_request = match packet {
        ClientPacket::RegisterAccount { username, password } => Some((username, password, true)),
        ClientPacket::LoginAccount { username, password } => Some((username, password, false)),
        other => {
            if let Ok(mut world) = state.lock() {
                world.handle_packet(player_id, other);
            }
            return true;
        }
    };

    let Some((username, password, register)) = auth_request else {
        return true;
    };
    let result = if auth_rate_limiter.allow_attempt_at(peer_ip, Instant::now()) {
        accounts.authenticate(&username, &password, register)
    } else {
        Err(AuthError::RateLimited)
    };
    state
        .lock()
        .map(|mut world| world.complete_auth(player_id, result))
        .unwrap_or(false)
}

pub(crate) fn handle_native_tcp_client(
    mut stream: StreamOwned<ServerConnection, TcpStream>,
    state: Arc<Mutex<WorldState>>,
    accounts: Arc<AccountStore>,
    auth_rate_limiter: Arc<AuthRateLimiter>,
) {
    let peer_ip = match stream.get_ref().peer_addr() {
        Ok(address) => address.ip(),
        Err(error) => {
            eprintln!("[wfo-server] failed to read client address: {error}");
            return;
        }
    };
    if let Err(error) = stream
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(50)))
    {
        eprintln!("[wfo-server] failed to set client read timeout: {error}");
        return;
    }
    let (out_tx, out_rx): (Sender<ServerPacket>, Receiver<ServerPacket>) = mpsc::channel();
    let sink = ClientSink(out_tx);
    let pid = register_player(&state, sink);
    if let Ok(world) = state.lock() {
        if let Some(player) = world.players.get(&pid) {
            player.sink.send_packet(&ServerPacket::AuthRequired);
        }
    }
    while let Ok(packet) = out_rx.try_recv() {
        if writeln!(stream, "{}", packet.encode()).is_err() {
            remove_player(&state, &pid);
            return;
        }
    }
    if stream.flush().is_err() {
        remove_player(&state, &pid);
        return;
    }

    let mut pending = Vec::new();
    let mut buf = [0u8; 2048];
    let mut connected = true;
    while connected {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => {
                pending.extend_from_slice(&buf[..read]);
                if pending.len() > 4096 {
                    eprintln!("[wfo-server] Closing oversized client packet from {pid}");
                    break;
                }
                while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                    let line = pending.drain(..=end).collect::<Vec<_>>();
                    let text = String::from_utf8_lossy(&line);
                    if let Some(packet) = ClientPacket::decode(&text) {
                        connected = handle_client_packet(
                            &state,
                            &accounts,
                            &auth_rate_limiter,
                            peer_ip,
                            &pid,
                            packet,
                        );
                        if !connected {
                            break;
                        }
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }

        while let Ok(packet) = out_rx.try_recv() {
            if writeln!(stream, "{}", packet.encode()).is_err() {
                connected = false;
                break;
            }
        }
        if connected && stream.flush().is_err() {
            connected = false;
        }
    }
    remove_player(&state, &pid);
}

pub(crate) fn load_server_tls_config() -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
    let cert_path = std::env::var("WFO_TLS_CERT")
        .map_err(|_| "WFO_TLS_CERT must point to a PEM server certificate")?;
    let key_path =
        std::env::var("WFO_TLS_KEY").map_err(|_| "WFO_TLS_KEY must point to a PEM private key")?;
    let cert_file = fs::File::open(cert_path)?;
    let certs =
        rustls_pemfile::certs(&mut BufReader::new(cert_file)).collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(fs::File::open(key_path)?))?
        .ok_or("WFO_TLS_KEY contains no supported private key")?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    Ok(Arc::new(config))
}

pub(crate) fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".webmanifest") {
        "application/manifest+json; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

pub(crate) fn handle_http_or_ws<S: Read + Write>(
    mut stream: S,
    state: Arc<Mutex<WorldState>>,
    accounts: Arc<AccountStore>,
    auth_rate_limiter: Arc<AuthRateLimiter>,
    peer_ip: IpAddr,
    secure: bool,
) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let raw_path = parts.next().unwrap_or("/");
    let path_only = raw_path.split('?').next().unwrap_or("/");

    let websocket_key = req.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case("sec-websocket-key")
            .then(|| value.trim().to_string())
    });

    if path_only == "/ws" {
        let proxied_https = req.lines().any(|line| {
            line.split_once(':')
                .map(|(k, v)| {
                    k.trim().eq_ignore_ascii_case("x-forwarded-proto")
                        && v.trim().eq_ignore_ascii_case("https")
                })
                .unwrap_or(false)
        });
        if secure || proxied_https || peer_ip.is_loopback() {
            if let Some(key) = websocket_key {
                let accept = ws_accept_key(&key);
                let response = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
                );
                if stream.write_all(response.as_bytes()).is_err() {
                    return;
                }

                let (out_tx, out_rx) = mpsc::channel();
                let pid = register_player(&state, ClientSink(out_tx));
                if let Ok(world) = state.lock() {
                    if let Some(player) = world.players.get(&pid) {
                        player.sink.send_packet(&ServerPacket::AuthRequired);
                    }
                }
                while let Ok(packet) = out_rx.try_recv() {
                    if write_ws_text_frame(&mut stream, &packet.encode()).is_err() {
                        remove_player(&state, &pid);
                        return;
                    }
                }
                if stream.flush().is_err() {
                    remove_player(&state, &pid);
                    return;
                }
                loop {
                    let frame = match read_ws_frame(&mut stream) {
                        Ok(frame) => frame,
                        Err(_) => break,
                    };
                    match frame {
                        WsFrame::Text(text) => {
                            if let Some(packet) = ClientPacket::decode(&text) {
                                if !handle_client_packet(
                                    &state,
                                    &accounts,
                                    &auth_rate_limiter,
                                    peer_ip,
                                    &pid,
                                    packet,
                                ) {
                                    break;
                                }
                            }
                        }
                        WsFrame::Ping(payload) => {
                            if write_ws_pong(&mut stream, &payload).is_err() {
                                break;
                            }
                        }
                        WsFrame::Close => break,
                    }
                    while let Ok(packet) = out_rx.try_recv() {
                        if write_ws_text_frame(&mut stream, &packet.encode()).is_err() {
                            remove_player(&state, &pid);
                            return;
                        }
                    }
                    if stream.flush().is_err() {
                        break;
                    }
                }
                remove_player(&state, &pid);
                return;
            }
        }

        let body = b"WebSocket game connections require TLS.";
        let response = format!(
            "HTTP/1.1 426 Upgrade Required\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(body);
        return;
    }

    if method == "GET" && path_only == "/api/world-info" {
        let (online, alive_mobs, total_mobs, crystals) = {
            let w = state.lock().unwrap();
            (
                w.players.values().filter(|p| p.joined).count(),
                w.mobs.values().filter(|m| m.snap.alive).count(),
                w.mobs.len(),
                w.crystals.values().filter(|c| c.snap.available).count(),
            )
        };
        let body = format!(
            "{{\"world\":\"Wayfarers Online (WFO)\",\"shortName\":\"WFO\",\"playersOnline\":{},\"aliveMobs\":{},\"totalMobs\":{},\"crystalsAvailable\":{},\"tcpPort\":7878}}",
            online, alive_mobs, total_mobs, crystals
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // Serve static web preview from dist/ (or public/)
    let rel = if path_only == "/" {
        "index.html"
    } else {
        path_only.trim_start_matches('/')
    };
    if rel.contains("..") {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
        return;
    }

    let dist_path = Path::new("dist").join(rel);
    let target = if dist_path.is_file() {
        dist_path
    } else {
        let pub_path = Path::new("public").join(rel);
        if pub_path.is_file() {
            pub_path
        } else {
            Path::new("dist").join("index.html")
        }
    };

    match fs::read(&target) {
        Ok(bytes) => {
            let ctype = content_type_for(target.to_str().unwrap_or(""));
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                ctype,
                bytes.len()
            );
            let _ = stream.write_all(hdr.as_bytes());
            let _ = stream.write_all(&bytes);
        }
        Err(_) => {
            let body = b"Wayfarers Online (WFO) Rust Server running. Run `npm run build` for web preview or launch `wfo-player`.";
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(hdr.as_bytes());
            let _ = stream.write_all(body);
        }
    }
}

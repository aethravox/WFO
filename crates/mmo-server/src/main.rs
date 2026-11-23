mod accounts;
mod ws_http;

use accounts::{Account, AccountStore, AuthError};
use mmo_protocol::{
    atan2_approx, cos_approx, ground_height_at, move_with_tree_collision, sanitize_field,
    sin_approx, sqrt_f32, AdventurerRank, ChatEntry, ClassId, ClientPacket, CombatFx,
    CrystalSnapshot, MobKind, MobSnapshot, PlayerSnapshot, ServerPacket, PLAYABLE_RADIUS,
    SAFE_PLAZA_RADIUS,
};
use rand_core::{OsRng, RngCore};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use ws_http::{read_ws_frame, write_ws_pong, write_ws_text_frame, ws_accept_key, WsFrame};

#[derive(Clone)]
pub(crate) struct ClientSink(Sender<ServerPacket>);

impl ClientSink {
    fn send_packet(&self, pkt: &ServerPacket) -> bool {
        self.0.send(pkt.clone()).is_ok()
    }
}

pub(crate) struct ServerPlayer {
    pub(crate) snap: PlayerSnapshot,
    pub(crate) movement_budget: MovementBudget,
    pub(crate) account_id: Option<i64>,
    pub(crate) authenticated: bool,
    pub(crate) auth_attempts: u8,
    pub(crate) joined: bool,
    pub(crate) in_lobby: bool,
    pub(crate) room_id: String,
    pub(crate) duel: Option<DuelState>,
    pub(crate) sink: ClientSink,
    pub(crate) skill_cd_until: [Instant; 3],
    pub(crate) shield_expires_at: Instant,
    pub(crate) haste_expires_at: Instant,
    pub(crate) dodge_cd_until: Instant,
    pub(crate) jump_velocity: f32,
    pub(crate) companion: Option<ActiveCompanion>,
}

#[derive(Clone, Copy)]
enum CompanionKind {
    Spirit,
    Turret,
}

#[derive(Clone, Copy)]
struct ActiveCompanion {
    kind: CompanionKind,
    x: f32,
    z: f32,
    expires_at: Instant,
    next_attack_at: Instant,
}

pub(crate) struct MovementBudget {
    pub(crate) distance: f32,
    pub(crate) updated_at: Instant,
}

impl ServerPlayer {
    fn absorb_shield(&mut self, damage: i32, now: Instant) -> i32 {
        if self.snap.shield_hp <= 0 {
            return damage.max(0);
        }
        if now >= self.shield_expires_at {
            self.snap.shield_hp = 0;
            return damage.max(0);
        }

        let damage = damage.max(0);
        let absorbed = damage.min(self.snap.shield_hp);
        self.snap.shield_hp -= absorbed;
        damage - absorbed
    }
}

impl MovementBudget {
    const BUFFER_SECONDS: f32 = 0.25;

    fn new(now: Instant) -> Self {
        Self {
            distance: 0.0,
            updated_at: now,
        }
    }

    fn reset(&mut self, now: Instant) {
        self.distance = 0.0;
        self.updated_at = now;
    }

    pub(crate) fn move_authoritatively(
        &mut self,
        from_x: f32,
        from_z: f32,
        direction_x: f32,
        direction_z: f32,
        speed: f32,
        now: Instant,
    ) -> (f32, f32) {
        let elapsed = now
            .saturating_duration_since(self.updated_at)
            .as_secs_f32()
            .clamp(0.0, Self::BUFFER_SECONDS);
        self.updated_at = now;

        let magnitude = direction_x.hypot(direction_z);
        if !magnitude.is_finite() || magnitude <= f32::EPSILON {
            return (from_x, from_z);
        }
        let nx = direction_x / magnitude;
        let nz = direction_z / magnitude;
        let distance = speed * elapsed;
        (from_x + nx * distance, from_z + nz * distance)
    }
}

fn generate_room_id() -> Option<String> {
    let mut token = [0u8; 9];
    if let Err(error) = OsRng.try_fill_bytes(&mut token) {
        eprintln!("[wfo-server] failed to generate lobby invite token: {error}");
        return None;
    }

    let token = token
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Some(format!("room_{token}"))
}

fn target_is_in_front(yaw: f32, from_x: f32, from_z: f32, target_x: f32, target_z: f32) -> bool {
    let dx = target_x - from_x;
    let dz = target_z - from_z;
    let distance = dx.hypot(dz);
    distance <= f32::EPSILON || sin_approx(yaw) * dx + cos_approx(yaw) * dz >= distance * 0.5
}

fn terrain_blocks_line(
    from_x: f32,
    from_y: f32,
    from_z: f32,
    to_x: f32,
    to_y: f32,
    to_z: f32,
) -> bool {
    let dx = to_x - from_x;
    let dz = to_z - from_z;
    let steps = dx.hypot(dz).ceil() as usize;
    for step in 1..steps {
        let t = step as f32 / steps as f32;
        let x = from_x + dx * t;
        let z = from_z + dz * t;
        let line_y = from_y + (to_y - from_y) * t;
        if ground_height_at(x, z) > line_y - 0.15 {
            return true;
        }
    }
    false
}

#[derive(Clone)]
pub(crate) struct DuelState {
    pub(crate) opponent_id: String,
    pub(crate) round: i32,
    pub(crate) wins: i32,
    pub(crate) opponent_wins: i32,
    pub(crate) round_active: bool,
    pub(crate) round_winner: String,
    pub(crate) pending_skill: Option<u8>,
}

pub(crate) struct ServerMob {
    pub(crate) snap: MobSnapshot,
    pub(crate) anchor_x: f32,
    pub(crate) anchor_z: f32,
    pub(crate) patrol_phase: f32,
    pub(crate) speed: f32,
    pub(crate) aggro_range: f32,
    pub(crate) attack_range: f32,
    pub(crate) damage: i32,
    pub(crate) xp_reward: i32,
    pub(crate) gold_reward: i32,
    pub(crate) aggro_target: Option<String>,
    pub(crate) last_attack: Instant,
    pub(crate) respawn_at: Instant,
}

pub(crate) struct ServerCrystal {
    pub(crate) snap: CrystalSnapshot,
    pub(crate) respawn_at: Instant,
}

pub(crate) struct WorldState {
    pub(crate) accounts: Arc<AccountStore>,
    pub(crate) next_id: u64,
    pub(crate) rng_seed: u64,
    pub(crate) players: HashMap<String, ServerPlayer>,
    pub(crate) mobs: HashMap<String, ServerMob>,
    pub(crate) crystals: HashMap<String, ServerCrystal>,
    pub(crate) chat: Vec<ChatEntry>,
}

mod network;
mod world;
use network::{
    handle_http_or_ws, handle_native_tcp_client, load_server_tls_config, AuthRateLimiter,
};

fn main() {
    let tls_config = load_server_tls_config().expect("configure TLS for online game sessions");
    let database_path = std::env::var_os("WFO_DB_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("wfo-server.sqlite3"));
    let accounts =
        Arc::new(AccountStore::open(&database_path).expect("initialize WFO account database"));
    let auth_rate_limiter = Arc::new(AuthRateLimiter::default());
    let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));

    // Simulation tick thread (20 Hz)
    let sim_state = Arc::clone(&state);
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(50));
        if let Ok(mut w) = sim_state.lock() {
            w.tick();
        }
    });

    // Native TLS MMO listener (port 7878) for `wfo-player` executables.
    let tcp_addr = std::env::var("WFO_TLS_ADDR").unwrap_or_else(|_| String::from("0.0.0.0:7878"));
    let tcp_listener = TcpListener::bind(&tcp_addr).expect("bind native MMO TLS port");
    println!("[wfo-server] Native TLS MMO listening on {}", tcp_addr);

    let tcp_state = Arc::clone(&state);
    let tcp_accounts = Arc::clone(&accounts);
    let tcp_auth_rate_limiter = Arc::clone(&auth_rate_limiter);
    let tcp_tls_config = Arc::clone(&tls_config);
    thread::spawn(move || {
        for stream in tcp_listener.incoming().flatten() {
            let _ = stream.set_nodelay(true);
            let st = Arc::clone(&tcp_state);
            let account_store = Arc::clone(&tcp_accounts);
            let auth_rate_limiter = Arc::clone(&tcp_auth_rate_limiter);
            let config = Arc::clone(&tcp_tls_config);
            thread::spawn(move || {
                let connection = match ServerConnection::new(config) {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("[wfo-server] TLS session setup failed: {error}");
                        return;
                    }
                };
                handle_native_tcp_client(
                    StreamOwned::new(connection, stream),
                    st,
                    account_store,
                    auth_rate_limiter,
                );
            });
        }
    });

    // Plain HTTP serves only static files and read-only world status (port 3000).
    let web_addr = std::env::var("WFO_WEB_ADDR").unwrap_or_else(|_| String::from("0.0.0.0:3000"));
    let web_listener = TcpListener::bind(&web_addr).expect("bind web preview port");
    println!(
        "[wfo-server] HTTP status/static listener on http://{}",
        web_addr
    );

    let web_state = Arc::clone(&state);
    let web_accounts = Arc::clone(&accounts);
    let web_auth_rate_limiter = Arc::clone(&auth_rate_limiter);
    thread::spawn(move || {
        for stream in web_listener.incoming().flatten() {
            let _ = stream.set_nodelay(true);
            let peer_ip = match stream.peer_addr() {
                Ok(address) => address.ip(),
                Err(error) => {
                    eprintln!("[wfo-server] failed to read web client address: {error}");
                    continue;
                }
            };
            let st = Arc::clone(&web_state);
            let account_store = Arc::clone(&web_accounts);
            let auth_rate_limiter = Arc::clone(&web_auth_rate_limiter);
            thread::spawn(move || {
                handle_http_or_ws(
                    stream,
                    st,
                    account_store,
                    auth_rate_limiter,
                    peer_ip,
                    false,
                )
            });
        }
    });

    // Secure WebSocket endpoint (WSS) shares the server certificate.
    let wss_addr = std::env::var("WFO_WSS_ADDR").unwrap_or_else(|_| String::from("0.0.0.0:3443"));
    let wss_listener = TcpListener::bind(&wss_addr).expect("bind secure WebSocket port");
    println!(
        "[wfo-server] Secure WebSocket listener on wss://{}/ws",
        wss_addr
    );
    for stream in wss_listener.incoming().flatten() {
        let _ = stream.set_nodelay(true);
        let peer_ip = match stream.peer_addr() {
            Ok(address) => address.ip(),
            Err(error) => {
                eprintln!("[wfo-server] failed to read WSS client address: {error}");
                continue;
            }
        };
        let st = Arc::clone(&state);
        let account_store = Arc::clone(&accounts);
        let auth_rate_limiter = Arc::clone(&auth_rate_limiter);
        let config = Arc::clone(&tls_config);
        thread::spawn(move || {
            let connection = match ServerConnection::new(config) {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("[wfo-server] TLS WebSocket setup failed: {error}");
                    return;
                }
            };
            handle_http_or_ws(
                StreamOwned::new(connection, stream),
                st,
                account_store,
                auth_rate_limiter,
                peer_ip,
                true,
            );
        });
    }
}

#[cfg(test)]
mod tests;

mod accounts;
mod ws_http;

use accounts::{Account, AccountStore, AuthError};
use mmo_protocol::{
    atan2_approx, cos_approx, ground_height_at, sanitize_field, sin_approx, sqrt_f32, ChatEntry,
    ClassId, ClientPacket, CombatFx, CrystalSnapshot, MobKind, MobSnapshot, PlayerSnapshot,
    ServerPacket, PLAYABLE_RADIUS, SAFE_PLAZA_RADIUS,
};
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
struct ClientSink(Sender<ServerPacket>);

impl ClientSink {
    fn send_packet(&self, pkt: &ServerPacket) -> bool {
        self.0.send(pkt.clone()).is_ok()
    }
}

struct ServerPlayer {
    snap: PlayerSnapshot,
    account_id: Option<i64>,
    authenticated: bool,
    auth_attempts: u8,
    joined: bool,
    in_lobby: bool,
    room_id: String,
    duel: Option<DuelState>,
    sink: ClientSink,
    skill_cd_until: [Instant; 3],
}

#[derive(Clone)]
struct DuelState {
    opponent_id: String,
    round: i32,
    wins: i32,
    opponent_wins: i32,
    round_active: bool,
    round_winner: String,
    pending_skill: Option<u8>,
}

struct ServerMob {
    snap: MobSnapshot,
    anchor_x: f32,
    anchor_z: f32,
    patrol_phase: f32,
    speed: f32,
    aggro_range: f32,
    attack_range: f32,
    damage: i32,
    xp_reward: i32,
    gold_reward: i32,
    aggro_target: Option<String>,
    last_attack: Instant,
    respawn_at: Instant,
}

struct ServerCrystal {
    snap: CrystalSnapshot,
    respawn_at: Instant,
}

struct WorldState {
    accounts: Arc<AccountStore>,
    next_id: u64,
    rng_seed: u64,
    players: HashMap<String, ServerPlayer>,
    mobs: HashMap<String, ServerMob>,
    crystals: HashMap<String, ServerCrystal>,
    chat: Vec<ChatEntry>,
}

impl WorldState {
    fn new(accounts: Arc<AccountStore>) -> Self {
        let mut ws = Self {
            accounts,
            next_id: 1,
            rng_seed: 0x9E37_79B9_7F4A_7C15,
            players: HashMap::new(),
            mobs: HashMap::new(),
            crystals: HashMap::new(),
            chat: Vec::new(),
        };
        ws.spawn_entities();
        ws
    }

    fn complete_auth(&mut self, player_id: &str, result: Result<Account, AuthError>) -> bool {
        if let Ok(account) = &result {
            let account_in_use = self
                .players
                .iter()
                .any(|(id, player)| id != player_id && player.account_id == Some(account.id));
            if account_in_use {
                if let Some(player) = self.players.get_mut(player_id) {
                    player.auth_attempts = player.auth_attempts.saturating_add(1);
                    player.sink.send_packet(&ServerPacket::AuthFailure {
                        message: String::from("Esta cuenta ya tiene una sesión activa."),
                    });
                    return player.auth_attempts < 5;
                }
                return false;
            }
        }
        let Some(player) = self.players.get_mut(player_id) else {
            return false;
        };
        if player.authenticated {
            return true;
        }

        match result {
            Ok(account) => {
                player.authenticated = true;
                player.account_id = Some(account.id);
                player.auth_attempts = 0;
                player.snap.name = account.username.clone();
                player.snap.class_id = account.class_id;
                player.snap.level = account.level;
                player.snap.xp = account.xp;
                player.snap.xp_next = account.xp_next;
                player.snap.gold = account.gold;
                player.snap.max_hp =
                    account.class_id.base_hp() + (account.level.saturating_sub(1)) * 28;
                player.snap.hp = player.snap.max_hp;
                player.snap.max_mp =
                    account.class_id.base_mp() + (account.level.saturating_sub(1)) * 16;
                player.snap.mp = player.snap.max_mp;
                player.sink.send_packet(&ServerPacket::AuthSuccess {
                    username: account.username,
                    class_id: account.class_id,
                    level: account.level,
                    xp: account.xp,
                    xp_next: account.xp_next,
                    gold: account.gold,
                });
                true
            }
            Err(error) => {
                if let AuthError::Storage(ref reason) = error {
                    eprintln!("[wfo-server] Account storage error: {reason}");
                }
                player.auth_attempts = player.auth_attempts.saturating_add(1);
                let message = match error {
                    AuthError::InvalidInput => "Nombre o contraseña no válidos.",
                    AuthError::InvalidCredentials => "Usuario o contraseña incorrectos.",
                    AuthError::RegistrationFailed => "No se pudo crear la cuenta.",
                    AuthError::Storage(_) => "Error al acceder a las cuentas.",
                };
                player.sink.send_packet(&ServerPacket::AuthFailure {
                    message: message.to_string(),
                });
                player.auth_attempts < 5
            }
        }
    }

    fn persist_account(&self, player_id: &str) {
        let Some(player) = self.players.get(player_id) else {
            return;
        };
        let Some(account_id) = player.account_id else {
            return;
        };
        if let Err(error) = self.accounts.save_progress(
            account_id,
            player.snap.class_id,
            player.snap.level,
            player.snap.xp,
            player.snap.xp_next,
            player.snap.gold,
        ) {
            eprintln!(
                "[wfo-server] Could not persist account {}: {error}",
                player.snap.name
            );
        }
    }

    fn next_rand(&mut self) -> f32 {
        self.rng_seed ^= self.rng_seed << 13;
        self.rng_seed ^= self.rng_seed >> 7;
        self.rng_seed ^= self.rng_seed << 17;
        ((self.rng_seed & 0xFFFF) as f32) / 65535.0
    }

    fn gen_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{}_{}", prefix, self.next_id)
    }

    fn spawn_entities(&mut self) {
        let now = Instant::now();
        let templates = [
            (
                MobKind::Stalker,
                "Rift Stalker",
                -32.0f32,
                18.0f32,
                1,
                90,
                5.0,
                14.0,
                2.6,
                9,
                28,
                12,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -46.0,
                4.0,
                1,
                95,
                5.2,
                14.0,
                2.6,
                10,
                30,
                14,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -36.0,
                -22.0,
                2,
                115,
                5.4,
                15.0,
                2.6,
                12,
                36,
                18,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -16.0,
                38.0,
                1,
                90,
                5.0,
                14.0,
                2.6,
                9,
                28,
                12,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                12.0,
                44.0,
                2,
                115,
                5.3,
                15.0,
                2.6,
                12,
                36,
                18,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                34.0,
                16.0,
                2,
                105,
                4.6,
                15.0,
                6.0,
                11,
                34,
                20,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                50.0,
                4.0,
                2,
                110,
                4.8,
                15.0,
                6.0,
                12,
                36,
                22,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                42.0,
                -18.0,
                3,
                135,
                4.9,
                16.0,
                6.0,
                14,
                45,
                26,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                28.0,
                -32.0,
                3,
                135,
                4.9,
                16.0,
                6.0,
                14,
                45,
                26,
            ),
            (
                MobKind::Sentinel,
                "Obsidian Sentinel",
                -18.0,
                -38.0,
                4,
                210,
                4.0,
                15.0,
                3.2,
                18,
                68,
                38,
            ),
            (
                MobKind::Sentinel,
                "Obsidian Sentinel",
                18.0,
                -40.0,
                4,
                210,
                4.0,
                15.0,
                3.2,
                18,
                68,
                38,
            ),
            (
                MobKind::Colossus,
                "Ancient Colossus",
                0.0,
                -58.0,
                6,
                620,
                3.6,
                18.0,
                4.5,
                28,
                240,
                140,
            ),
        ];

        for (idx, &(kind, name, x, z, level, hp, speed, aggro, atk_r, dmg, xp, gold)) in
            templates.iter().enumerate()
        {
            let id = format!("mob_{}", idx + 1);
            self.mobs.insert(
                id.clone(),
                ServerMob {
                    snap: MobSnapshot {
                        id,
                        kind,
                        name: name.to_string(),
                        x,
                        y: ground_height_at(x, z),
                        z,
                        yaw: 3.14159,
                        hp,
                        max_hp: hp,
                        level,
                        alive: true,
                        attack_tick: 0,
                    },
                    anchor_x: x,
                    anchor_z: z,
                    patrol_phase: (idx as f32) * 1.3,
                    speed,
                    aggro_range: aggro,
                    attack_range: atk_r,
                    damage: dmg,
                    xp_reward: xp,
                    gold_reward: gold,
                    aggro_target: None,
                    last_attack: now,
                    respawn_at: now,
                },
            );
        }

        let crystals = [
            (28.0f32, 8.0f32),
            (40.0, 22.0),
            (54.0, -8.0),
            (38.0, -28.0),
            (-26.0, 26.0),
            (-44.0, -12.0),
            (-12.0, -26.0),
            (16.0, 32.0),
        ];
        for (idx, &(x, z)) in crystals.iter().enumerate() {
            let id = format!("crystal_{}", idx + 1);
            self.crystals.insert(
                id.clone(),
                ServerCrystal {
                    snap: CrystalSnapshot {
                        id,
                        x,
                        y: ground_height_at(x, z),
                        z,
                        available: true,
                    },
                    respawn_at: now,
                },
            );
        }
    }

    fn broadcast(&self, pkt: &ServerPacket, exclude: Option<&str>) {
        for (id, p) in &self.players {
            if !p.joined {
                continue;
            }
            if let Some(ex) = exclude {
                if id == ex {
                    continue;
                }
            }
            p.sink.send_packet(pkt);
        }
    }

    fn broadcast_lobby(&self, room_id: &str, pkt: &ServerPacket, exclude: Option<&str>) {
        for (id, p) in &self.players {
            if !p.in_lobby || p.room_id != room_id {
                continue;
            }
            if let Some(ex) = exclude {
                if id == ex {
                    continue;
                }
            }
            p.sink.send_packet(pkt);
        }
    }

    fn grant_rewards(&mut self, player_id: &str, xp_gain: i32, gold_gain: i32) {
        let mut leveled_to: Option<i32> = None;
        if let Some(p) = self.players.get_mut(player_id) {
            p.snap.xp += xp_gain;
            p.snap.gold += gold_gain;
            while p.snap.xp >= p.snap.xp_next {
                p.snap.xp -= p.snap.xp_next;
                p.snap.level += 1;
                p.snap.xp_next = ((p.snap.xp_next as f32) * 1.45) as i32;
                p.snap.max_hp = p.snap.class_id.base_hp() + (p.snap.level - 1) * 28;
                p.snap.max_mp = p.snap.class_id.base_mp() + (p.snap.level - 1) * 16;
                p.snap.hp = p.snap.max_hp;
                p.snap.mp = p.snap.max_mp;
                leveled_to = Some(p.snap.level);
            }
        }
        if let Some(lvl) = leveled_to {
            self.broadcast(
                &ServerPacket::LevelUp {
                    id: player_id.to_string(),
                    level: lvl,
                },
                None,
            );
        }
        self.persist_account(player_id);
    }

    fn handle_packet(&mut self, player_id: &str, pkt: ClientPacket) {
        if matches!(
            &pkt,
            ClientPacket::RegisterAccount { .. } | ClientPacket::LoginAccount { .. }
        ) {
            return;
        }
        if !self
            .players
            .get(player_id)
            .is_some_and(|player| player.authenticated)
        {
            return;
        }

        let now = Instant::now();
        match pkt {
            ClientPacket::RegisterAccount { .. } | ClientPacket::LoginAccount { .. } => return,
            ClientPacket::DuelChallenge { target_id } => {
                let target_joined = self
                    .players
                    .get(&target_id)
                    .map(|p| p.joined)
                    .unwrap_or(false);
                if !target_joined {
                    return;
                }
                let challenger_joined = self
                    .players
                    .get(player_id)
                    .map(|p| p.joined)
                    .unwrap_or(false);
                if !challenger_joined {
                    return;
                }
                let duel = DuelState {
                    opponent_id: target_id.clone(),
                    round: 1,
                    wins: 0,
                    opponent_wins: 0,
                    round_active: true,
                    round_winner: String::new(),
                    pending_skill: None,
                };
                if let Some(p) = self.players.get_mut(player_id) {
                    p.duel = Some(duel.clone());
                }
                if let Some(p) = self.players.get_mut(&target_id) {
                    p.duel = Some(DuelState {
                        opponent_id: player_id.to_string(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                        pending_skill: None,
                    });
                }
                if let (Some(a), Some(b)) =
                    (self.players.get(player_id), self.players.get(&target_id))
                {
                    let state_a = ServerPacket::DuelState {
                        opponent_id: b.snap.id.clone(),
                        opponent_name: b.snap.name.clone(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                    };
                    let state_b = ServerPacket::DuelState {
                        opponent_id: a.snap.id.clone(),
                        opponent_name: a.snap.name.clone(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                    };
                    a.sink.send_packet(&state_a);
                    b.sink.send_packet(&state_b);
                }
            }
            ClientPacket::DuelAction { target_id, skill } => {
                if target_id.is_empty() {
                    return;
                }
                let skill = skill.min(2);
                if let Some(p) = self.players.get_mut(player_id) {
                    if let Some(ref mut duel) = p.duel {
                        if duel.opponent_id != target_id || !duel.round_active {
                            return;
                        }
                        duel.pending_skill = Some(skill);
                    } else {
                        return;
                    }
                }

                let opponent_ready = self
                    .players
                    .get(&target_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill)
                    .is_some();
                let own_skill = self
                    .players
                    .get(player_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill);
                if !opponent_ready {
                    return;
                }
                let opponent_skill = self
                    .players
                    .get(&target_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill)
                    .unwrap_or(0);
                let own_skill = own_skill.unwrap_or(0);

                let round_winner = if own_skill == opponent_skill {
                    String::from("EMPATE")
                } else if (own_skill == 0 && opponent_skill == 2)
                    || (own_skill == 1 && opponent_skill == 0)
                    || (own_skill == 2 && opponent_skill == 1)
                {
                    let name = self
                        .players
                        .get(player_id)
                        .map(|p| p.snap.name.clone())
                        .unwrap_or_default();
                    if let Some(p) = self.players.get_mut(player_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.wins += 1;
                        }
                    }
                    if let Some(p) = self.players.get_mut(&target_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.opponent_wins += 1;
                        }
                    }
                    name
                } else {
                    let name = self
                        .players
                        .get(&target_id)
                        .map(|p| p.snap.name.clone())
                        .unwrap_or_default();
                    if let Some(p) = self.players.get_mut(&target_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.wins += 1;
                        }
                    }
                    if let Some(p) = self.players.get_mut(player_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.opponent_wins += 1;
                        }
                    }
                    name
                };

                for pid in [player_id, target_id.as_str()] {
                    if let Some(p) = self.players.get_mut(pid) {
                        if let Some(ref mut duel) = p.duel {
                            duel.round_winner = round_winner.clone();
                            duel.round_active = duel.wins < 3 && duel.opponent_wins < 3;
                            if duel.round_active {
                                duel.round = (duel.round + 1).min(5);
                            } else {
                                duel.round = 5;
                            }
                            duel.pending_skill = None;
                        }
                    }
                }

                for pid in [player_id, target_id.as_str()] {
                    if let (Some(p), Some(d)) = (
                        self.players.get(pid),
                        self.players.get(pid).and_then(|p| p.duel.clone()),
                    ) {
                        let opponent = self.players.get(&d.opponent_id);
                        if let Some(opponent) = opponent {
                            p.sink.send_packet(&ServerPacket::DuelState {
                                opponent_id: opponent.snap.id.clone(),
                                opponent_name: opponent.snap.name.clone(),
                                round: d.round,
                                wins: d.wins,
                                opponent_wins: d.opponent_wins,
                                round_active: d.round_active,
                                round_winner: d.round_winner,
                            });
                        }
                    }
                }
            }
            ClientPacket::DuelSurrender => {
                if let Some(p) = self.players.get_mut(player_id) {
                    p.duel = None;
                }
            }
            ClientPacket::EnterLobby { name } => {
                let clean = sanitize_field(name.trim(), 18);
                let clean_name = if clean.is_empty() {
                    String::from("Traveler")
                } else {
                    clean
                };
                let room_id = format!("room_{}", player_id);
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    p.snap.name = clean_name;
                    p.snap.x = 0.0;
                    p.snap.y = 0.0;
                    p.snap.z = 4.0;
                    p.joined = false;
                    p.in_lobby = true;
                    p.room_id = room_id.clone();
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::LobbyInit {
                    self_id: player_id.to_string(),
                    room_id,
                });
            }
            ClientPacket::CreateInvite => {
                let sink = {
                    let Some(p) = self.players.get(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.sink.clone()
                };
                let room_id = self
                    .players
                    .get(player_id)
                    .map(|p| p.room_id.clone())
                    .unwrap_or_default();
                if !room_id.is_empty() {
                    sink.send_packet(&ServerPacket::LobbyInvite { room_id });
                }
            }
            ClientPacket::JoinLobbyRoom { room_id } => {
                let clean_room = sanitize_field(room_id.trim(), 24);
                let exists = self
                    .players
                    .values()
                    .any(|p| p.in_lobby && p.room_id == clean_room);
                if !exists {
                    return;
                }
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.room_id = clean_room.clone();
                    p.snap.x = 0.0;
                    p.snap.z = 4.0;
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::LobbyJoined {
                    room_id: clean_room.clone(),
                });
                for (id, p) in &self.players {
                    if p.in_lobby && p.room_id == clean_room {
                        sink.send_packet(&ServerPacket::Player(p.snap.clone()));
                        if id != player_id {
                            p.sink.send_packet(&ServerPacket::Player(
                                self.players
                                    .get(player_id)
                                    .map(|p| p.snap.clone())
                                    .unwrap_or_else(|| p.snap.clone()),
                            ));
                        }
                    }
                }
            }
            ClientPacket::RandomClass => {
                let seed = self.next_rand();
                let class_id = match (seed * 3.0) as u8 % 3 {
                    0 => ClassId::Vanguard,
                    1 => ClassId::Arcanist,
                    _ => ClassId::Ranger,
                };
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.snap.class_id = class_id;
                    p.snap.max_hp = class_id.base_hp();
                    p.snap.hp = class_id.base_hp();
                    p.snap.max_mp = class_id.base_mp();
                    p.snap.mp = class_id.base_mp();
                    p.sink.clone()
                };
                self.persist_account(player_id);
                sink.send_packet(&ServerPacket::LobbyClass { class_id });
            }
            ClientPacket::EnterWorld => {
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.in_lobby = false;
                    p.joined = true;
                    p.snap.x = 0.0;
                    p.snap.y = ground_height_at(0.0, 4.0);
                    p.snap.z = 4.0;
                    p.snap.moving = false;
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::WorldEntered);
                sink.send_packet(&ServerPacket::Init {
                    self_id: player_id.to_string(),
                });
                let world_snap = self.players.get(player_id).map(|p| p.snap.clone());
                if let Some(snap) = world_snap {
                    for other in self.players.values() {
                        if other.joined {
                            sink.send_packet(&ServerPacket::Player(other.snap.clone()));
                        }
                    }
                    for m in self.mobs.values() {
                        sink.send_packet(&ServerPacket::Mob(m.snap.clone()));
                    }
                    for c in self.crystals.values() {
                        sink.send_packet(&ServerPacket::Crystal(c.snap.clone()));
                    }
                    sink.send_packet(&ServerPacket::SnapshotEnd);
                    self.broadcast(&ServerPacket::Player(snap), Some(player_id));
                }
            }
            ClientPacket::Join { name, class_id } => {
                let clean = sanitize_field(name.trim(), 18);
                let clean_name = if clean.is_empty() {
                    String::from("Hero")
                } else {
                    clean
                };
                if let Some(p) = self.players.get_mut(player_id) {
                    p.snap.name = clean_name;
                    p.snap.class_id = class_id;
                    p.snap.max_hp = class_id.base_hp();
                    p.snap.hp = class_id.base_hp();
                    p.snap.max_mp = class_id.base_mp();
                    p.snap.mp = class_id.base_mp();
                    p.joined = true;

                    let sink = p.sink.clone();
                    let self_snap = p.snap.clone();
                    sink.send_packet(&ServerPacket::Init {
                        self_id: player_id.to_string(),
                    });
                    for other in self.players.values() {
                        if other.joined {
                            sink.send_packet(&ServerPacket::Player(other.snap.clone()));
                        }
                    }
                    for m in self.mobs.values() {
                        sink.send_packet(&ServerPacket::Mob(m.snap.clone()));
                    }
                    for c in self.crystals.values() {
                        sink.send_packet(&ServerPacket::Crystal(c.snap.clone()));
                    }
                    for msg in self.chat.iter().rev().take(20).rev() {
                        sink.send_packet(&ServerPacket::Chat(msg.clone()));
                    }
                    sink.send_packet(&ServerPacket::SnapshotEnd);

                    self.broadcast(&ServerPacket::Player(self_snap), Some(player_id));
                }
            }
            ClientPacket::Move {
                x,
                y,
                z,
                yaw,
                moving,
            } => {
                if !x.is_finite() || !z.is_finite() || !yaw.is_finite() {
                    return;
                }
                if let Some(p) = self.players.get(player_id) {
                    if p.in_lobby {
                        let cx = x.clamp(-11.5, 11.5);
                        let cz = z.clamp(-9.0, 9.0);
                        if let Some(player) = self.players.get_mut(player_id) {
                            player.snap.x = cx;
                            player.snap.y = 0.0;
                            player.snap.z = cz;
                            player.snap.yaw = yaw;
                            player.snap.moving = moving;
                        }
                        let room_id = self
                            .players
                            .get(player_id)
                            .map(|p| p.room_id.clone())
                            .unwrap_or_default();
                        let snap = self.players.get(player_id).map(|p| p.snap.clone());
                        if let (false, Some(snap)) = (room_id.is_empty(), snap) {
                            self.broadcast_lobby(&room_id, &ServerPacket::Player(snap), None);
                        }
                        return;
                    }
                    if !p.joined {
                        return;
                    }
                }
                let dist = sqrt_f32(x * x + z * z);
                let (cx, cz) = if dist > PLAYABLE_RADIUS {
                    (x * PLAYABLE_RADIUS / dist, z * PLAYABLE_RADIUS / dist)
                } else {
                    (x, z)
                };
                let gy = ground_height_at(cx, cz);
                let sy = if y.is_finite() {
                    y.clamp(gy, gy + 4.5)
                } else {
                    gy
                };
                if let Some(p) = self.players.get_mut(player_id) {
                    if !p.joined {
                        return;
                    }
                    p.snap.x = cx;
                    p.snap.y = sy;
                    p.snap.z = cz;
                    p.snap.yaw = yaw;
                    p.snap.moving = moving;
                }
            }
            ClientPacket::Attack {
                skill,
                target_id,
                yaw,
            } => {
                let s_idx = match skill {
                    1 => 1,
                    2 => 2,
                    _ => 0,
                };
                let (px, py, pz, class_id, level, mp_ok, cd_ok) = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.joined {
                        return;
                    }
                    let mp_cost = match s_idx {
                        1 => 22,
                        2 => 28,
                        _ => 0,
                    };
                    let cd_dur = match s_idx {
                        1 => Duration::from_millis(3000),
                        2 => Duration::from_millis(5000),
                        _ => Duration::from_millis(520),
                    };
                    let cd_ok = now >= p.skill_cd_until[s_idx];
                    let mp_ok = p.snap.mp >= mp_cost;
                    if cd_ok && mp_ok {
                        p.skill_cd_until[s_idx] = now + cd_dur;
                        p.snap.mp -= mp_cost;
                        p.snap.yaw = if yaw.is_finite() { yaw } else { p.snap.yaw };
                        p.snap.attack_tick = p.snap.attack_tick.wrapping_add(1);
                    }
                    (
                        p.snap.x,
                        p.snap.y,
                        p.snap.z,
                        p.snap.class_id,
                        p.snap.level,
                        mp_ok,
                        cd_ok,
                    )
                };
                if !cd_ok || !mp_ok {
                    return;
                }

                // Skill 2: Heal / Mend
                if s_idx == 2 {
                    let heal = {
                        let p = self.players.get_mut(player_id).unwrap();
                        let h = ((p.snap.max_hp as f32) * 0.34) as i32;
                        p.snap.hp = (p.snap.hp + h).min(p.snap.max_hp);
                        h
                    };
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: px,
                            y: py + 1.4,
                            z: pz,
                            skill: 2,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal,
                            xp_gained: 0,
                            gold_gained: 0,
                        }),
                        None,
                    );
                    return;
                }

                let is_aoe = s_idx == 1;
                let range = class_id.attack_range() + if is_aoe { 3.0 } else { 0.0 };
                let dmg_mult = if is_aoe { 1.75 } else { 1.0 };

                let mut hit_ids: Vec<String> = Vec::new();
                if !is_aoe && !target_id.is_empty() {
                    if let Some(m) = self.mobs.get(&target_id) {
                        if m.snap.alive {
                            let d = sqrt_f32((m.snap.x - px).powi(2) + (m.snap.z - pz).powi(2));
                            if d <= range + 1.5 {
                                hit_ids.push(m.snap.id.clone());
                            }
                        }
                    }
                }
                if hit_ids.is_empty() {
                    let mut best: Option<(String, f32)> = None;
                    for (mid, m) in &self.mobs {
                        if !m.snap.alive {
                            continue;
                        }
                        let d = sqrt_f32((m.snap.x - px).powi(2) + (m.snap.z - pz).powi(2));
                        if d <= range {
                            if is_aoe {
                                hit_ids.push(mid.clone());
                            } else if best.as_ref().map(|b| d < b.1).unwrap_or(true) {
                                best = Some((mid.clone(), d));
                            }
                        }
                    }
                    if !is_aoe {
                        if let Some((bid, _)) = best {
                            hit_ids.push(bid);
                        }
                    }
                }

                if hit_ids.is_empty() {
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: px + sin_approx(yaw) * 2.2,
                            y: py + 1.2,
                            z: pz + cos_approx(yaw) * 2.2,
                            skill: s_idx as u8,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal: 0,
                            xp_gained: 0,
                            gold_gained: 0,
                        }),
                        None,
                    );
                    return;
                }

                let mut total_xp = 0;
                let mut total_gold = 0;
                let mut fx_list = Vec::new();

                for mid in hit_ids.into_iter().take(if is_aoe { 4 } else { 1 }) {
                    let r1 = self.next_rand();
                    let r2 = self.next_rand();
                    let crit = r1
                        < if class_id == ClassId::Ranger {
                            0.28
                        } else {
                            0.18
                        };
                    let lvl_bonus = 1.0 + ((level - 1) as f32) * 0.16;
                    let raw = (class_id.base_damage() as f32)
                        * lvl_bonus
                        * dmg_mult
                        * (0.9 + r2 * 0.22)
                        * if crit { 1.65 } else { 1.0 };
                    let dmg = (raw as i32).max(4);

                    if let Some(m) = self.mobs.get_mut(&mid) {
                        m.snap.hp = (m.snap.hp - dmg).max(0);
                        m.aggro_target = Some(player_id.to_string());
                        let mut xp_gained = 0;
                        let mut gold_gained = 0;
                        if m.snap.hp == 0 {
                            m.snap.alive = false;
                            m.aggro_target = None;
                            let respawn_secs = if m.snap.kind == MobKind::Colossus {
                                25
                            } else {
                                12
                            };
                            m.respawn_at = now + Duration::from_secs(respawn_secs);
                            xp_gained = m.xp_reward;
                            gold_gained = m.gold_reward;
                            total_xp += xp_gained;
                            total_gold += gold_gained;
                        }
                        fx_list.push(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: m.snap.x,
                            y: m.snap.y
                                + if m.snap.kind == MobKind::Colossus {
                                    3.2
                                } else {
                                    1.7
                                },
                            z: m.snap.z,
                            skill: s_idx as u8,
                            class_id,
                            damage: dmg,
                            crit,
                            heal: 0,
                            xp_gained,
                            gold_gained,
                        });
                    }
                }

                if total_xp > 0 || total_gold > 0 {
                    self.grant_rewards(player_id, total_xp, total_gold);
                }
                for fx in fx_list {
                    self.broadcast(&ServerPacket::CombatFx(fx), None);
                }
            }
            ClientPacket::Gather { crystal_id } => {
                let Some(p) = self.players.get(player_id) else {
                    return;
                };
                if !p.joined {
                    return;
                }
                let (px, pz) = (p.snap.x, p.snap.z);
                let class_id = p.snap.class_id;

                let gathered = if let Some(c) = self.crystals.get_mut(&crystal_id) {
                    if c.snap.available
                        && sqrt_f32((c.snap.x - px).powi(2) + (c.snap.z - pz).powi(2)) <= 6.5
                    {
                        c.snap.available = false;
                        c.respawn_at = now + Duration::from_secs(16);
                        Some(c.snap.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(cs) = gathered {
                    self.grant_rewards(player_id, 22, 18);
                    self.broadcast(&ServerPacket::Crystal(cs.clone()), None);
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: cs.x,
                            y: cs.y + 1.5,
                            z: cs.z,
                            skill: 3,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal: 0,
                            xp_gained: 22,
                            gold_gained: 18,
                        }),
                        None,
                    );
                }
            }
            ClientPacket::Chat { text } => {
                let clean = sanitize_field(text.trim(), 140);
                if clean.is_empty() {
                    return;
                }
                let Some(p) = self.players.get(player_id) else {
                    return;
                };
                if !p.joined {
                    return;
                }
                let entry = ChatEntry {
                    sender_id: player_id.to_string(),
                    sender_name: p.snap.name.clone(),
                    class_id: p.snap.class_id,
                    text: clean,
                };
                self.chat.push(entry.clone());
                if self.chat.len() > 40 {
                    self.chat.remove(0);
                }
                self.broadcast(&ServerPacket::Chat(entry), None);
            }
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = 0.05f32;

        // Respawn Crystals
        let mut crystal_updates = Vec::new();
        for c in self.crystals.values_mut() {
            if !c.snap.available && now >= c.respawn_at {
                c.snap.available = true;
                crystal_updates.push(c.snap.clone());
            }
        }
        for cs in crystal_updates {
            self.broadcast(&ServerPacket::Crystal(cs), None);
        }

        // Passive Player Regen (faster in Safe Sanctuary Plaza)
        for p in self.players.values_mut() {
            if !p.joined {
                continue;
            }
            let in_plaza = sqrt_f32(p.snap.x * p.snap.x + p.snap.z * p.snap.z) < SAFE_PLAZA_RADIUS;
            if in_plaza && p.snap.hp < p.snap.max_hp {
                p.snap.hp = (p.snap.hp + 2).min(p.snap.max_hp);
            }
            if p.snap.mp < p.snap.max_mp {
                p.snap.mp = (p.snap.mp + if in_plaza { 2 } else { 1 }).min(p.snap.max_mp);
            }
        }

        // Mob AI & Attacks
        let mob_ids: Vec<String> = self.mobs.keys().cloned().collect();
        let mut mob_fx: Vec<CombatFx> = Vec::new();

        for mid in mob_ids {
            let rand_hit = self.next_rand();
            let mob_alive = self.mobs.get(&mid).map(|m| m.snap.alive).unwrap_or(false);
            if !mob_alive {
                let m = self.mobs.get_mut(&mid).unwrap();
                if now >= m.respawn_at {
                    m.snap.alive = true;
                    m.snap.hp = m.snap.max_hp;
                    m.snap.x = m.anchor_x;
                    m.snap.z = m.anchor_z;
                    m.snap.y = ground_height_at(m.anchor_x, m.anchor_z);
                    m.aggro_target = None;
                }
                continue;
            }

            let (mx, mz, anchor_x, anchor_z, aggro_range, current_target) = {
                let m = self.mobs.get(&mid).unwrap();
                (
                    m.snap.x,
                    m.snap.z,
                    m.anchor_x,
                    m.anchor_z,
                    m.aggro_range,
                    m.aggro_target.clone(),
                )
            };

            let mut target_pos: Option<(String, f32, f32, ClassId)> = None;
            if let Some(tid) = current_target {
                if let Some(tp) = self.players.get(&tid) {
                    let in_plaza =
                        sqrt_f32(tp.snap.x * tp.snap.x + tp.snap.z * tp.snap.z) < SAFE_PLAZA_RADIUS;
                    let leash_d =
                        sqrt_f32((tp.snap.x - anchor_x).powi(2) + (tp.snap.z - anchor_z).powi(2));
                    if tp.joined && !in_plaza && leash_d < 28.0 {
                        target_pos = Some((tid, tp.snap.x, tp.snap.z, tp.snap.class_id));
                    }
                }
            }

            if target_pos.is_none() {
                let mut best_d = aggro_range;
                for (pid, tp) in &self.players {
                    if !tp.joined {
                        continue;
                    }
                    if sqrt_f32(tp.snap.x * tp.snap.x + tp.snap.z * tp.snap.z) < SAFE_PLAZA_RADIUS {
                        continue;
                    }
                    let d = sqrt_f32((tp.snap.x - mx).powi(2) + (tp.snap.z - mz).powi(2));
                    if d < best_d {
                        best_d = d;
                        target_pos = Some((pid.clone(), tp.snap.x, tp.snap.z, tp.snap.class_id));
                    }
                }
            }

            let mut dealt_damage: Option<(String, i32, f32, f32, f32, ClassId)> = None;
            {
                let m = self.mobs.get_mut(&mid).unwrap();
                if let Some((tid, tx, tz, t_class)) = target_pos {
                    m.aggro_target = Some(tid.clone());
                    let dx = tx - m.snap.x;
                    let dz = tz - m.snap.z;
                    let d = sqrt_f32(dx * dx + dz * dz);
                    m.snap.yaw = atan2_approx(dx, dz);
                    if d > m.attack_range * 0.85 {
                        let step = (d - m.attack_range * 0.75).min(m.speed * dt);
                        m.snap.x += (dx / d) * step;
                        m.snap.z += (dz / d) * step;
                        m.snap.y = ground_height_at(m.snap.x, m.snap.z);
                    } else {
                        let cd = if m.snap.kind == MobKind::Colossus {
                            Duration::from_millis(1900)
                        } else {
                            Duration::from_millis(1450)
                        };
                        if now.duration_since(m.last_attack) >= cd {
                            m.last_attack = now;
                            m.snap.attack_tick = m.snap.attack_tick.wrapping_add(1);
                            let dmg = ((m.damage as f32) * (0.85 + rand_hit * 0.3)) as i32;
                            dealt_damage =
                                Some((tid, dmg, tx, ground_height_at(tx, tz), tz, t_class));
                        }
                    }
                } else {
                    m.aggro_target = None;
                    m.patrol_phase += dt * 0.65;
                    let px = m.anchor_x + cos_approx(m.patrol_phase) * 3.2;
                    let pz = m.anchor_z + sin_approx(m.patrol_phase * 0.8) * 3.2;
                    let dx = px - m.snap.x;
                    let dz = pz - m.snap.z;
                    let d = sqrt_f32(dx * dx + dz * dz);
                    if d > 0.15 {
                        m.snap.yaw = atan2_approx(dx, dz);
                        let step = d.min(m.speed * 0.45 * dt);
                        m.snap.x += (dx / d) * step;
                        m.snap.z += (dz / d) * step;
                        m.snap.y = ground_height_at(m.snap.x, m.snap.z);
                    }
                }
            }

            if let Some((tid, dmg, tx, ty, tz, t_class)) = dealt_damage {
                let mut reset_aggro = false;
                if let Some(tp) = self.players.get_mut(&tid) {
                    tp.snap.hp = (tp.snap.hp - dmg).max(0);
                    if tp.snap.hp == 0 {
                        tp.snap.hp = tp.snap.max_hp;
                        tp.snap.mp = tp.snap.max_mp;
                        tp.snap.x = 0.0;
                        tp.snap.z = 3.5;
                        tp.snap.y = ground_height_at(0.0, 3.5);
                        reset_aggro = true;
                    }
                }
                if reset_aggro {
                    if let Some(m) = self.mobs.get_mut(&mid) {
                        m.aggro_target = None;
                    }
                }
                mob_fx.push(CombatFx {
                    attacker_id: mid,
                    x: tx,
                    y: ty + 1.5,
                    z: tz,
                    skill: 0,
                    class_id: t_class,
                    damage: dmg,
                    crit: false,
                    heal: 0,
                    xp_gained: 0,
                    gold_gained: 0,
                });
            }
        }

        for fx in mob_fx {
            self.broadcast(&ServerPacket::CombatFx(fx), None);
        }

        // Broadcast authoritative snapshot
        let any_joined = self.players.values().any(|p| p.joined);
        if !any_joined {
            return;
        }
        for p in self.players.values() {
            if p.joined {
                self.broadcast(&ServerPacket::Player(p.snap.clone()), None);
            }
        }
        for m in self.mobs.values() {
            self.broadcast(&ServerPacket::Mob(m.snap.clone()), None);
        }
        self.broadcast(&ServerPacket::SnapshotEnd, None);
    }
}

fn register_player(state: &Arc<Mutex<WorldState>>, sink: ClientSink) -> String {
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
                attack_tick: 0,
            },
            account_id: None,
            authenticated: false,
            auth_attempts: 0,
            joined: false,
            in_lobby: false,
            room_id: String::new(),
            duel: None,
            sink,
            skill_cd_until: [now, now, now],
        },
    );
    pid
}

fn remove_player(state: &Arc<Mutex<WorldState>>, pid: &str) {
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

fn handle_client_packet(
    state: &Arc<Mutex<WorldState>>,
    accounts: &AccountStore,
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
    let result = accounts.authenticate(&username, &password, register);
    state
        .lock()
        .map(|mut world| world.complete_auth(player_id, result))
        .unwrap_or(false)
}

fn handle_native_tcp_client(
    mut stream: StreamOwned<ServerConnection, TcpStream>,
    state: Arc<Mutex<WorldState>>,
    accounts: Arc<AccountStore>,
) {
    let (out_tx, out_rx): (Sender<ServerPacket>, Receiver<ServerPacket>) = mpsc::channel();
    let sink = ClientSink(out_tx);
    let pid = register_player(&state, sink);
    if let Ok(world) = state.lock() {
        if let Some(player) = world.players.get(&pid) {
            player.sink.send_packet(&ServerPacket::AuthRequired);
        }
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
                        connected = handle_client_packet(&state, &accounts, &pid, packet);
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

fn load_server_tls_config() -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
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

fn content_type_for(path: &str) -> &'static str {
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

fn handle_http_or_ws<S: Read + Write>(
    mut stream: S,
    state: Arc<Mutex<WorldState>>,
    accounts: Arc<AccountStore>,
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
        if secure {
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
                                if !handle_client_packet(&state, &accounts, &pid, packet) {
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

fn main() {
    let tls_config = load_server_tls_config().expect("configure TLS for online game sessions");
    let database_path = std::env::var_os("WFO_DB_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("wfo-server.sqlite3"));
    let accounts =
        Arc::new(AccountStore::open(&database_path).expect("initialize WFO account database"));
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
    let tcp_tls_config = Arc::clone(&tls_config);
    thread::spawn(move || {
        for stream in tcp_listener.incoming().flatten() {
            let _ = stream.set_nodelay(true);
            let st = Arc::clone(&tcp_state);
            let account_store = Arc::clone(&tcp_accounts);
            let config = Arc::clone(&tcp_tls_config);
            thread::spawn(move || {
                let connection = match ServerConnection::new(config) {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("[wfo-server] TLS session setup failed: {error}");
                        return;
                    }
                };
                handle_native_tcp_client(StreamOwned::new(connection, stream), st, account_store);
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
    thread::spawn(move || {
        for stream in web_listener.incoming().flatten() {
            let _ = stream.set_nodelay(true);
            let st = Arc::clone(&web_state);
            let account_store = Arc::clone(&web_accounts);
            thread::spawn(move || handle_http_or_ws(stream, st, account_store, false));
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
        let st = Arc::clone(&state);
        let account_store = Arc::clone(&accounts);
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
                true,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn account_cannot_be_authenticated_by_two_players() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-sessions-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let accounts = Arc::new(AccountStore::open(&path).expect("open test database"));
        let account = accounts
            .authenticate("Session_User", "correct horse battery", true)
            .expect("register test account");
        let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));
        let (first_tx, _first_rx) = mpsc::channel();
        let first = register_player(&state, ClientSink(first_tx));
        let (second_tx, second_rx) = mpsc::channel();
        let second = register_player(&state, ClientSink(second_tx));

        {
            let mut world = state.lock().expect("lock world");
            assert!(world.complete_auth(&first, Ok(account.clone())));
            assert!(world.complete_auth(&second, Ok(account.clone())));
            assert!(world.players[&first].authenticated);
            assert!(!world.players[&second].authenticated);
        }
        let response = second_rx.try_iter().last().expect("authentication failure");
        assert!(matches!(response, ServerPacket::AuthFailure { .. }));

        drop(accounts);
        std::fs::remove_file(path).expect("remove test database");
    }
}

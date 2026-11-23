#![allow(clippy::all)]
use super::*;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
pub fn sanitize_field(s: &str, max_len: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == '|' || c == '\n' || c == '\r' {
            continue;
        }
        if out.len() + c.len_utf8() > max_len {
            break;
        }
        out.push(c);
    }
    out
}

impl LanPacket {
    pub fn encode(&self) -> String {
        match self {
            Self::Announce {
                peer_id,
                name,
                udp_port,
                address,
            } => format!(
                "LAN|ANN|{}|{}|{}|{}",
                sanitize_field(peer_id, 24),
                sanitize_field(name, 18),
                udp_port,
                sanitize_field(address, 48)
            ),
            Self::Challenge { from_id, from_name } => format!(
                "LAN|CHAL|{}|{}",
                sanitize_field(from_id, 24),
                sanitize_field(from_name, 18)
            ),
            Self::Accept { from_id } => format!("LAN|ACC|{}", sanitize_field(from_id, 24)),
            Self::Action {
                from_id,
                round,
                skill,
            } => format!(
                "LAN|ACT|{}|{}|{}",
                sanitize_field(from_id, 24),
                round,
                skill
            ),
            Self::Surrender { from_id } => format!("LAN|SUR|{}", sanitize_field(from_id, 24)),
        }
    }

    pub fn decode(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.first().copied()? != "LAN" {
            return None;
        }
        match parts.get(1).copied()? {
            "ANN" if parts.len() >= 6 => Some(Self::Announce {
                peer_id: sanitize_field(parts[2], 24),
                name: sanitize_field(parts[3], 18),
                udp_port: parts[4].parse().ok()?,
                address: sanitize_field(parts[5], 48),
            }),
            "CHAL" if parts.len() >= 4 => Some(Self::Challenge {
                from_id: sanitize_field(parts[2], 24),
                from_name: sanitize_field(parts[3], 18),
            }),
            "ACC" if parts.len() >= 3 => Some(Self::Accept {
                from_id: sanitize_field(parts[2], 24),
            }),
            "ACT" if parts.len() >= 5 => Some(Self::Action {
                from_id: sanitize_field(parts[2], 24),
                round: parts[3].parse().ok()?,
                skill: parts[4].parse().ok()?,
            }),
            "SUR" if parts.len() >= 3 => Some(Self::Surrender {
                from_id: sanitize_field(parts[2], 24),
            }),
            _ => None,
        }
    }
}

impl ClientPacket {
    pub fn encode(&self) -> String {
        match self {
            Self::RegisterAccount { username, password } => format!(
                "REGISTER|{}|{}",
                sanitize_field(username, 18),
                sanitize_field(password, 128)
            ),
            Self::LoginAccount { username, password } => format!(
                "LOGIN|{}|{}",
                sanitize_field(username, 18),
                sanitize_field(password, 128)
            ),
            Self::EnterLobby { name } => {
                format!("LOBBY|{}", sanitize_field(name, 18))
            }
            Self::CreateInvite => String::from("INVITE"),
            Self::JoinLobbyRoom { room_id } => {
                format!("JOINROOM|{}", sanitize_field(room_id, 24))
            }
            Self::RandomClass => String::from("RANDOM"),
            Self::EnterWorld => String::from("WORLD"),
            Self::DuelChallenge { target_id } => {
                format!("DUELCHAL|{}", sanitize_field(target_id, 24))
            }
            Self::DuelAction { target_id, skill } => {
                format!("DUELACT|{}|{}", sanitize_field(target_id, 24), skill)
            }
            Self::DuelSurrender => String::from("DUELSURRENDER"),
            Self::Join { name, class_id } => {
                format!("JOIN|{}|{}", sanitize_field(name, 18), class_id.as_code())
            }
            Self::Move {
                dx,
                dz,
                yaw,
                moving,
            } => {
                let m = if *moving { 1 } else { 0 };
                format!("MOVE|{:.3}|{:.3}|{:.3}|{}", dx, dz, yaw, m)
            }
            Self::Dodge => String::from("DODGE"),
            Self::Jump => String::from("JUMP"),
            Self::Attack {
                skill,
                target_id,
                yaw,
            } => {
                format!("ATK|{}|{}|{:.3}", skill, sanitize_field(target_id, 24), yaw)
            }
            Self::Gather { crystal_id } => {
                format!("GATH|{}", sanitize_field(crystal_id, 24))
            }
            Self::Emote { emote } => format!("EMOTE|{}", emote.as_code()),
            Self::Chat { text } => {
                format!("CHAT|{}", sanitize_field(text, 140))
            }
        }
    }

    pub fn decode(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.trim().split('|').collect();
        match parts.first().copied()? {
            "REGISTER" if parts.len() == 3 => Some(Self::RegisterAccount {
                username: sanitize_field(parts[1], 18),
                password: sanitize_field(parts[2], 128),
            }),
            "LOGIN" if parts.len() == 3 => Some(Self::LoginAccount {
                username: sanitize_field(parts[1], 18),
                password: sanitize_field(parts[2], 128),
            }),
            "LOBBY" if parts.len() >= 2 => Some(Self::EnterLobby {
                name: sanitize_field(parts[1], 18),
            }),
            "INVITE" => Some(Self::CreateInvite),
            "JOINROOM" if parts.len() >= 2 => Some(Self::JoinLobbyRoom {
                room_id: sanitize_field(parts[1], 24),
            }),
            "RANDOM" => Some(Self::RandomClass),
            "WORLD" => Some(Self::EnterWorld),
            "DUELCHAL" if parts.len() >= 2 => Some(Self::DuelChallenge {
                target_id: sanitize_field(parts[1], 24),
            }),
            "DUELACT" if parts.len() >= 3 => Some(Self::DuelAction {
                target_id: sanitize_field(parts[1], 24),
                skill: parts[2].parse().ok()?,
            }),
            "DUELSURRENDER" => Some(Self::DuelSurrender),
            "JOIN" if parts.len() >= 3 => Some(Self::Join {
                name: sanitize_field(parts[1], 18),
                class_id: ClassId::from_str_code(parts[2]),
            }),
            "MOVE" if parts.len() >= 5 => Some(Self::Move {
                dx: parts[1].parse().ok()?,
                dz: parts[2].parse().ok()?,
                yaw: parts[3].parse().ok()?,
                moving: parts[4] == "1",
            }),
            "DODGE" => Some(Self::Dodge),
            "JUMP" => Some(Self::Jump),
            "ATK" if parts.len() >= 4 => Some(Self::Attack {
                skill: parts[1].parse().ok()?,
                target_id: parts[2].to_string(),
                yaw: parts[3].parse().ok()?,
            }),
            "GATH" if parts.len() >= 2 => Some(Self::Gather {
                crystal_id: parts[1].to_string(),
            }),
            "EMOTE" if parts.len() == 2 => Some(Self::Emote {
                emote: EmoteId::from_code(parts[1])?,
            }),
            "CHAT" if parts.len() >= 2 => Some(Self::Chat {
                text: sanitize_field(parts[1], 140),
            }),
            _ => None,
        }
    }
}

impl ServerPacket {
    pub fn encode(&self) -> String {
        match self {
            Self::AuthRequired => String::from("AUTHREQ"),
            Self::ConnectionStatus { connected } => {
                format!("NET|{}", if *connected { 1 } else { 0 })
            }
            Self::AuthSuccess {
                username,
                class_id,
                level,
                xp,
                xp_next,
                gold,
            } => format!(
                "AUTHOK|{}|{}|{}|{}|{}|{}",
                sanitize_field(username, 18),
                class_id.as_code(),
                level,
                xp,
                xp_next,
                gold
            ),
            Self::AuthFailure { message } => {
                format!("AUTHERR|{}", sanitize_field(message, 96))
            }
            Self::LobbyInit { self_id, room_id } => {
                format!(
                    "LOBBYINIT|{}|{}",
                    sanitize_field(self_id, 24),
                    sanitize_field(room_id, 24)
                )
            }
            Self::LobbyInvite { room_id } => {
                format!("LOBBYINVITE|{}", sanitize_field(room_id, 24))
            }
            Self::LobbyJoined { room_id } => {
                format!("LOBBYJOINED|{}", sanitize_field(room_id, 24))
            }
            Self::LobbyClass { class_id } => {
                format!("LOBBYCLASS|{}", class_id.as_code())
            }
            Self::WorldEntered => String::from("WORLDENTERED"),
            Self::DuelState {
                opponent_id,
                opponent_name,
                round,
                wins,
                opponent_wins,
                round_active,
                round_winner,
            } => format!(
                "DUELSTATE|{}|{}|{}|{}|{}|{}|{}",
                sanitize_field(opponent_id, 24),
                sanitize_field(opponent_name, 18),
                round,
                wins,
                opponent_wins,
                if *round_active { 1 } else { 0 },
                sanitize_field(round_winner, 18)
            ),
            Self::Init { self_id } => format!("INIT|{}", self_id),
            Self::Player(p) => format!(
                "PLR|{}|{}|{}|{:.3}|{:.3}|{:.3}|{:.3}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                p.id,
                sanitize_field(&p.name, 18),
                p.class_id.as_code(),
                p.x,
                p.y,
                p.z,
                p.yaw,
                if p.moving { 1 } else { 0 },
                p.hp,
                p.max_hp,
                p.mp,
                p.max_mp,
                p.level,
                p.xp,
                p.xp_next,
                p.gold,
                p.attack_tick,
                p.rank_points,
                p.shield_hp,
                if p.haste { 1 } else { 0 }
            ),
            Self::Mob(m) => format!(
                "MOB|{}|{}|{}|{:.3}|{:.3}|{:.3}|{:.3}|{}|{}|{}|{}|{}",
                m.id,
                m.kind.as_code(),
                m.name,
                m.x,
                m.y,
                m.z,
                m.yaw,
                m.hp,
                m.max_hp,
                m.level,
                if m.alive { 1 } else { 0 },
                m.attack_tick
            ),
            Self::Crystal(c) => format!(
                "CRYS|{}|{:.3}|{:.3}|{:.3}|{}",
                c.id,
                c.x,
                c.y,
                c.z,
                if c.available { 1 } else { 0 }
            ),
            Self::PlayerLeft { id } => format!("LEFT|{}", id),
            Self::CombatFx(fx) => format!(
                "FX|{}|{:.3}|{:.3}|{:.3}|{}|{}|{}|{}|{}|{}|{}",
                fx.attacker_id,
                fx.x,
                fx.y,
                fx.z,
                fx.skill,
                fx.class_id.as_code(),
                fx.damage,
                if fx.crit { 1 } else { 0 },
                fx.heal,
                fx.xp_gained,
                fx.gold_gained
            ),
            Self::LevelUp { id, level } => format!("LVL|{}|{}", id, level),
            Self::RankUp { id, rank } => format!("RANK|{}|{}", id, rank.as_code()),
            Self::Emote { player_id, emote } => {
                format!(
                    "EMOTE|{}|{}",
                    sanitize_field(player_id, 24),
                    emote.as_code()
                )
            }
            Self::Chat(c) => format!(
                "CHAT|{}|{}|{}|{}",
                c.sender_id,
                sanitize_field(&c.sender_name, 18),
                c.class_id.as_code(),
                sanitize_field(&c.text, 140)
            ),
            Self::SnapshotEnd => String::from("SNAP_END"),
        }
    }

    pub fn decode(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.trim().split('|').collect();
        match parts.first().copied()? {
            "AUTHREQ" => Some(Self::AuthRequired),
            "NET" if parts.len() >= 2 => Some(Self::ConnectionStatus {
                connected: parts[1] == "1",
            }),
            "AUTHOK" if parts.len() >= 7 => Some(Self::AuthSuccess {
                username: sanitize_field(parts[1], 18),
                class_id: ClassId::from_str_code(parts[2]),
                level: parts[3].parse().ok()?,
                xp: parts[4].parse().ok()?,
                xp_next: parts[5].parse().ok()?,
                gold: parts[6].parse().ok()?,
            }),
            "AUTHERR" if parts.len() >= 2 => Some(Self::AuthFailure {
                message: sanitize_field(parts[1], 96),
            }),
            "LOBBYINIT" if parts.len() >= 3 => Some(Self::LobbyInit {
                self_id: parts[1].to_string(),
                room_id: parts[2].to_string(),
            }),
            "LOBBYINVITE" if parts.len() >= 2 => Some(Self::LobbyInvite {
                room_id: parts[1].to_string(),
            }),
            "LOBBYJOINED" if parts.len() >= 2 => Some(Self::LobbyJoined {
                room_id: parts[1].to_string(),
            }),
            "LOBBYCLASS" if parts.len() >= 2 => Some(Self::LobbyClass {
                class_id: ClassId::from_str_code(parts[1]),
            }),
            "WORLDENTERED" => Some(Self::WorldEntered),
            "DUELSTATE" if parts.len() >= 8 => Some(Self::DuelState {
                opponent_id: parts[1].to_string(),
                opponent_name: parts[2].to_string(),
                round: parts[3].parse().ok()?,
                wins: parts[4].parse().ok()?,
                opponent_wins: parts[5].parse().ok()?,
                round_active: parts[6] == "1",
                round_winner: parts[7].to_string(),
            }),
            "INIT" if parts.len() >= 2 => Some(Self::Init {
                self_id: parts[1].to_string(),
            }),
            "PLR" if parts.len() >= 18 => Some(Self::Player(PlayerSnapshot {
                id: parts[1].to_string(),
                name: parts[2].to_string(),
                class_id: ClassId::from_str_code(parts[3]),
                x: parts[4].parse().ok()?,
                y: parts[5].parse().ok()?,
                z: parts[6].parse().ok()?,
                yaw: parts[7].parse().ok()?,
                moving: parts[8] == "1",
                hp: parts[9].parse().ok()?,
                max_hp: parts[10].parse().ok()?,
                mp: parts[11].parse().ok()?,
                max_mp: parts[12].parse().ok()?,
                level: parts[13].parse().ok()?,
                xp: parts[14].parse().ok()?,
                xp_next: parts[15].parse().ok()?,
                gold: parts[16].parse().ok()?,
                rank_points: if parts.len() >= 19 {
                    parts[18].parse().ok()?
                } else {
                    0
                },
                attack_tick: parts[17].parse().ok()?,
                shield_hp: if parts.len() >= 20 {
                    parts[19].parse().ok()?
                } else {
                    0
                },
                haste: parts.get(20).is_some_and(|value| *value == "1"),
            })),
            "MOB" if parts.len() >= 13 => Some(Self::Mob(MobSnapshot {
                id: parts[1].to_string(),
                kind: MobKind::from_str_code(parts[2]),
                name: parts[3].to_string(),
                x: parts[4].parse().ok()?,
                y: parts[5].parse().ok()?,
                z: parts[6].parse().ok()?,
                yaw: parts[7].parse().ok()?,
                hp: parts[8].parse().ok()?,
                max_hp: parts[9].parse().ok()?,
                level: parts[10].parse().ok()?,
                alive: parts[11] == "1",
                attack_tick: parts[12].parse().ok()?,
            })),
            "CRYS" if parts.len() >= 6 => Some(Self::Crystal(CrystalSnapshot {
                id: parts[1].to_string(),
                x: parts[2].parse().ok()?,
                y: parts[3].parse().ok()?,
                z: parts[4].parse().ok()?,
                available: parts[5] == "1",
            })),
            "LEFT" if parts.len() >= 2 => Some(Self::PlayerLeft {
                id: parts[1].to_string(),
            }),
            "FX" if parts.len() >= 12 => Some(Self::CombatFx(CombatFx {
                attacker_id: parts[1].to_string(),
                x: parts[2].parse().ok()?,
                y: parts[3].parse().ok()?,
                z: parts[4].parse().ok()?,
                skill: parts[5].parse().ok()?,
                class_id: ClassId::from_str_code(parts[6]),
                damage: parts[7].parse().ok()?,
                crit: parts[8] == "1",
                heal: parts[9].parse().ok()?,
                xp_gained: parts[10].parse().ok()?,
                gold_gained: parts[11].parse().ok()?,
            })),
            "LVL" if parts.len() >= 3 => Some(Self::LevelUp {
                id: parts[1].to_string(),
                level: parts[2].parse().ok()?,
            }),
            "RANK" if parts.len() >= 3 => Some(Self::RankUp {
                id: parts[1].to_string(),
                rank: AdventurerRank::from_code(parts[2])?,
            }),
            "EMOTE" if parts.len() >= 3 => Some(Self::Emote {
                player_id: sanitize_field(parts[1], 24),
                emote: EmoteId::from_code(parts[2])?,
            }),
            "CHAT" if parts.len() >= 5 => Some(Self::Chat(ChatEntry {
                sender_id: parts[1].to_string(),
                sender_name: parts[2].to_string(),
                class_id: ClassId::from_str_code(parts[3]),
                text: parts[4].to_string(),
            })),
            "SNAP_END" => Some(Self::SnapshotEnd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServerPacket {
    AuthRequired,
    ConnectionStatus {
        connected: bool,
    },
    AuthSuccess {
        username: String,
        class_id: ClassId,
        level: i32,
        xp: i32,
        xp_next: i32,
        gold: i32,
    },
    AuthFailure {
        message: String,
    },
    LobbyInit {
        self_id: String,
        room_id: String,
    },
    LobbyInvite {
        room_id: String,
    },
    LobbyJoined {
        room_id: String,
    },
    LobbyClass {
        class_id: ClassId,
    },
    WorldEntered,
    DuelState {
        opponent_id: String,
        opponent_name: String,
        round: i32,
        wins: i32,
        opponent_wins: i32,
        round_active: bool,
        round_winner: String,
    },
    Init {
        self_id: String,
    },
    Player(PlayerSnapshot),
    Mob(MobSnapshot),
    Crystal(CrystalSnapshot),
    PlayerLeft {
        id: String,
    },
    CombatFx(CombatFx),
    LevelUp {
        id: String,
        level: i32,
    },
    RankUp {
        id: String,
        rank: AdventurerRank,
    },
    Emote {
        player_id: String,
        emote: EmoteId,
    },
    Chat(ChatEntry),
    SnapshotEnd,
}

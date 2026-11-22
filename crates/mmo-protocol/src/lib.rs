#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lang::ui::{
    ButtonMode, ButtonView, LabelStyle, LangButton, LangHost, Rect, UiEvent,
};
use lang::{CatalogEntry, LangCatalog, Language};

pub const PLAYABLE_RADIUS: f32 = 84.0;
pub const SAFE_PLAZA_RADIUS: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClassId {
    Vanguard = 0,
    Arcanist = 1,
    Ranger = 2,
}

impl ClassId {
    pub fn from_str_code(s: &str) -> Self {
        match s {
            "arcanist" | "1" => Self::Arcanist,
            "ranger" | "2" => Self::Ranger,
            _ => Self::Vanguard,
        }
    }

    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Vanguard => "vanguard",
            Self::Arcanist => "arcanist",
            Self::Ranger => "ranger",
        }
    }

    pub const fn base_hp(self) -> i32 {
        match self {
            Self::Vanguard => 160,
            Self::Arcanist => 110,
            Self::Ranger => 130,
        }
    }

    pub const fn base_mp(self) -> i32 {
        match self {
            Self::Vanguard => 70,
            Self::Arcanist => 140,
            Self::Ranger => 95,
        }
    }

    pub const fn move_speed(self) -> f32 {
        match self {
            Self::Vanguard => 9.2,
            Self::Arcanist => 8.8,
            Self::Ranger => 10.4,
        }
    }

    pub const fn attack_range(self) -> f32 {
        match self {
            Self::Vanguard => 4.6,
            Self::Arcanist => 14.5,
            Self::Ranger => 13.0,
        }
    }

    pub const fn base_damage(self) -> i32 {
        match self {
            Self::Vanguard => 24,
            Self::Arcanist => 26,
            Self::Ranger => 22,
        }
    }

    pub const fn accent_rgb(self) -> u32 {
        match self {
            Self::Vanguard => 0xF59E0B,
            Self::Arcanist => 0x38BDF8,
            Self::Ranger => 0x10B981,
        }
    }

    pub const fn primary_rgb(self) -> u32 {
        match self {
            Self::Vanguard => 0xC27824,
            Self::Arcanist => 0x1D4ED8,
            Self::Ranger => 0x15803D,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MobKind {
    Stalker = 0,
    Wisp = 1,
    Sentinel = 2,
    Colossus = 3,
}

impl MobKind {
    pub fn from_str_code(s: &str) -> Self {
        match s {
            "wisp" | "1" => Self::Wisp,
            "sentinel" | "2" => Self::Sentinel,
            "colossus" | "3" => Self::Colossus,
            _ => Self::Stalker,
        }
    }

    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Stalker => "stalker",
            Self::Wisp => "wisp",
            Self::Sentinel => "sentinel",
            Self::Colossus => "colossus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneId {
    Plaza,
    Woods,
    Citadel,
    Veins,
}

pub fn zone_at(x: f32, z: f32) -> ZoneId {
    let dist = sqrt_f32(x * x + z * z);
    if dist < 18.0 {
        ZoneId::Plaza
    } else if z < -34.0 && abs_f32(x) < 34.0 {
        ZoneId::Citadel
    } else if x > 20.0 {
        ZoneId::Veins
    } else {
        ZoneId::Woods
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub id: String,
    pub name: String,
    pub class_id: ClassId,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub moving: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub level: i32,
    pub xp: i32,
    pub xp_next: i32,
    pub gold: i32,
    pub attack_tick: u32,
}

#[derive(Debug, Clone)]
pub struct MobSnapshot {
    pub id: String,
    pub kind: MobKind,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub alive: bool,
    pub attack_tick: u32,
}

#[derive(Debug, Clone)]
pub struct CrystalSnapshot {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct CombatFx {
    pub attacker_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub skill: u8,
    pub class_id: ClassId,
    pub damage: i32,
    pub crit: bool,
    pub heal: i32,
    pub xp_gained: i32,
    pub gold_gained: i32,
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub sender_id: String,
    pub sender_name: String,
    pub class_id: ClassId,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum LanPacket {
    Announce {
        peer_id: String,
        name: String,
        udp_port: u16,
        address: String,
    },
    Challenge {
        from_id: String,
        from_name: String,
    },
    Accept {
        from_id: String,
    },
    Action {
        from_id: String,
        round: i32,
        skill: u8,
    },
    Surrender {
        from_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum ClientPacket {
    EnterLobby {
        name: String,
    },
    CreateInvite,
    JoinLobbyRoom {
        room_id: String,
    },
    RandomClass,
    EnterWorld,
    DuelChallenge {
        target_id: String,
    },
    DuelAction {
        target_id: String,
        skill: u8,
    },
    DuelSurrender,
    Join {
        name: String,
        class_id: ClassId,
    },
    Move {
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        moving: bool,
    },
    Attack {
        skill: u8,
        target_id: String,
        yaw: f32,
    },
    Gather {
        crystal_id: String,
    },
    Chat {
        text: String,
    },
}

#[derive(Debug, Clone)]
pub enum ServerPacket {
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
    Init { self_id: String },
    Player(PlayerSnapshot),
    Mob(MobSnapshot),
    Crystal(CrystalSnapshot),
    PlayerLeft { id: String },
    CombatFx(CombatFx),
    LevelUp { id: String, level: i32 },
    Chat(ChatEntry),
    SnapshotEnd,
}

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
            Self::Announce { peer_id, name, udp_port, address } => format!(
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
            Self::Accept { from_id } => format!(
                "LAN|ACC|{}",
                sanitize_field(from_id, 24)
            ),
            Self::Action { from_id, round, skill } => format!(
                "LAN|ACT|{}|{}|{}",
                sanitize_field(from_id, 24),
                round,
                skill
            ),
            Self::Surrender { from_id } => format!(
                "LAN|SUR|{}",
                sanitize_field(from_id, 24)
            ),
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
                x,
                y,
                z,
                yaw,
                moving,
            } => {
                let m = if *moving { 1 } else { 0 };
                format!("MOVE|{:.3}|{:.3}|{:.3}|{:.3}|{}", x, y, z, yaw, m)
            }
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
            Self::Chat { text } => {
                format!("CHAT|{}", sanitize_field(text, 140))
            }
        }
    }

    pub fn decode(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.trim().split('|').collect();
        match parts.first().copied()? {
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
            "MOVE" if parts.len() >= 6 => Some(Self::Move {
                x: parts[1].parse().ok()?,
                y: parts[2].parse().ok()?,
                z: parts[3].parse().ok()?,
                yaw: parts[4].parse().ok()?,
                moving: parts[5] == "1",
            }),
            "ATK" if parts.len() >= 4 => Some(Self::Attack {
                skill: parts[1].parse().ok()?,
                target_id: parts[2].to_string(),
                yaw: parts[3].parse().ok()?,
            }),
            "GATH" if parts.len() >= 2 => Some(Self::Gather {
                crystal_id: parts[1].to_string(),
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
            Self::LobbyInit { self_id, room_id } => {
                format!("LOBBYINIT|{}|{}", sanitize_field(self_id, 24), sanitize_field(room_id, 24))
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
                "PLR|{}|{}|{}|{:.3}|{:.3}|{:.3}|{:.3}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
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
                p.attack_tick
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
                attack_tick: parts[17].parse().ok()?,
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

/// Deterministic `#![no_std]` terrain height shared by Rust server, native player, and WASM preview.
pub fn ground_height_at(x: f32, z: f32) -> f32 {
    let dist_sq = x * x + z * z;
    if dist_sq < 225.0 {
        return 0.0;
    }
    let dist = sqrt_f32(dist_sq);
    let blend = clamp_f32((dist - 15.0) / 14.0, 0.0, 1.0);
    let hill = sin_approx(x * 0.055) * cos_approx(z * 0.055) * 2.4
        + sin_approx((x + z) * 0.09) * 0.8;
    let rim = if dist > 82.0 {
        let t = (dist - 82.0) / 16.0;
        t * t * 9.0
    } else {
        0.0
    };
    hill * blend + rim
}

#[inline]
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn abs_f32(v: f32) -> f32 {
    if v < 0.0 { -v } else { v }
}

#[inline]
pub fn floor_f32(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) > x { i - 1 } else { i }
}

#[inline]
pub fn ceil_f32(x: f32) -> i32 {
    let i = x as i32;
    if (i as f32) < x { i + 1 } else { i }
}

#[inline]
pub fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut guess = x * 0.5 + 0.5;
    let mut i = 0;
    while i < 7 {
        guess = 0.5 * (guess + x / guess);
        i += 1;
    }
    guess
}

#[inline]
pub fn sin_approx(mut x: f32) -> f32 {
    const PI: f32 = 3.14159265;
    const TWO_PI: f32 = 6.2831853;
    while x > PI {
        x -= TWO_PI;
    }
    while x < -PI {
        x += TWO_PI;
    }
    let b = 4.0 / PI;
    let c = -4.0 / (PI * PI);
    let y = b * x + c * x * abs_f32(x);
    0.225 * (y * abs_f32(y) - y) + y
}

#[inline]
pub fn cos_approx(x: f32) -> f32 {
    sin_approx(x + 1.5707963)
}

pub fn atan2_approx(y: f32, x: f32) -> f32 {
    const PI: f32 = 3.14159265;
    if x == 0.0 {
        if y > 0.0 {
            return PI * 0.5;
        }
        if y < 0.0 {
            return -PI * 0.5;
        }
        return 0.0;
    }
    let z = y / x;
    let az = abs_f32(z);
    let atan = if az < 1.0 {
        z / (1.0 + 0.28 * z * z)
    } else {
        let sign = if z > 0.0 { 1.0 } else { -1.0 };
        sign * (PI * 0.5) - z / (z * z + 0.28)
    };
    if x < 0.0 {
        if y >= 0.0 {
            atan + PI
        } else {
            atan - PI
        }
    } else {
        atan
    }
}

// =========================================================================
// aethravox/Lang Integration (6-Language Offline MMO UI + LangButton Host)
// =========================================================================

pub static MMO_CATALOG: LangCatalog = LangCatalog::new(&[
    CatalogEntry::new(
        lang::fnv1a_64("WAYFARERS ONLINE (WFO)".as_bytes()),
        "WAYFARERS ONLINE (WFO)",
        [
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
            "WAYFARERS ONLINE (WFO)",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("ENTRAR AL MUNDO".as_bytes()),
        "ENTRAR AL MUNDO",
        [
            "ENTRAR AL MUNDO",
            "ENTER WORLD",
            "ENTRAR NO MUNDO",
            "ENTRER DANS LE MONDE",
            "WELT BETRETEN",
            "ENTRA NEL MONDO",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("NOMBRE".as_bytes()),
        "NOMBRE",
        ["NOMBRE", "NAME", "NOME", "NOM", "NAME", "NOME"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("VANGUARDIA".as_bytes()),
        "VANGUARDIA",
        [
            "VANGUARDIA",
            "VANGUARD",
            "VANGUARDA",
            "AVANT-GARDE",
            "VORHUT",
            "AVANGUARDIA",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("ARCANISTA".as_bytes()),
        "ARCANISTA",
        [
            "ARCANISTA",
            "ARCANIST",
            "ARCANISTA",
            "ARCANISTE",
            "ARKANIST",
            "ARCANISTA",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("MONTARAZ".as_bytes()),
        "MONTARAZ",
        [
            "MONTARAZ",
            "RANGER",
            "PATRULHEIRO",
            "RODEUR",
            "WALDLAUFER",
            "GUARDABOSCHI",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("GOLPE".as_bytes()),
        "GOLPE",
        ["GOLPE", "STRIKE", "GOLPE", "FRAPPE", "SCHLAG", "COLPO"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("NOVA".as_bytes()),
        "NOVA",
        ["NOVA", "SKILL", "SKILL", "SORT", "SKILL", "SKILL"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("CURAR".as_bytes()),
        "CURAR",
        ["CURAR", "MEND", "CURAR", "SOIN", "HEILEN", "CURA"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("MINAR".as_bytes()),
        "MINAR",
        ["MINAR", "GATHER", "COLETAR", "MINER", "SAMMELN", "MINARE"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("PLAZA SANTUARIO".as_bytes()),
        "PLAZA SANTUARIO",
        [
            "PLAZA SANTUARIO",
            "SANCTUARY PLAZA",
            "PRACA SANTUARIO",
            "PLACE SANCTUAIRE",
            "ZUFLUCHTSPLATZ",
            "PIAZZA SANTUARIO",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("BOSQUE ESMERALDA".as_bytes()),
        "BOSQUE ESMERALDA",
        [
            "BOSQUE ESMERALDA",
            "EMERALD WOODS",
            "BOSQUE ESMERALDA",
            "BOIS EMERAUDE",
            "SMARAGDWALD",
            "BOSCO SMERALDO",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("CIUDADELA".as_bytes()),
        "CIUDADELA",
        [
            "CIUDADELA",
            "CITADEL",
            "CIDADELA",
            "CITADELLE",
            "ZITADELLE",
            "CITTADELLA",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("VETAS CRISTAL".as_bytes()),
        "VETAS CRISTAL",
        [
            "VETAS CRISTAL",
            "CRYSTAL VEINS",
            "VEIOS CRISTAL",
            "VEINES CRISTAL",
            "KRISTALLADERN",
            "VENE CRISTALLO",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("EN LINEA".as_bytes()),
        "EN LINEA",
        [
            "EN LINEA",
            "ONLINE",
            "ONLINE",
            "EN LIGNE",
            "ONLINE",
            "ONLINE",
        ],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("ORO".as_bytes()),
        "ORO",
        ["ORO", "GOLD", "OURO", "OR", "GOLD", "ORO"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("CHAT".as_bytes()),
        "CHAT",
        ["CHAT", "CHAT", "CHAT", "CHAT", "CHAT", "CHAT"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("SALTAR".as_bytes()),
        "SALTAR",
        ["SALTAR", "JUMP", "PULAR", "SAUTER", "SPRINGEN", "SALTA"],
    ),
    CatalogEntry::new(
        lang::fnv1a_64("TACTIL".as_bytes()),
        "TACTIL",
        ["TACTIL", "TOUCH", "TOQUE", "TACTILE", "TOUCH", "TOUCH"],
    ),
]);

#[lang::app]
mod auto_labels {
    pub fn basics() -> [&'static str; 2] {
        ["Conectar", "Enviar"]
    }
}

pub struct MmoLangHost {
    pub slot: Rect,
    pub btn_bounds: Rect,
    pub btn_label: &'static str,
    pub current: Language,
}

impl MmoLangHost {
    pub fn new(slot: Rect) -> Self {
        Self {
            slot,
            btn_bounds: slot,
            btn_label: "ES",
            current: Language::Es,
        }
    }
}

impl LangHost for MmoLangHost {
    fn ready_slot(&self) -> Rect {
        self.slot
    }

    fn draw_lang_button(&mut self, view: &ButtonView<'_>) {
        self.btn_bounds = view.bounds;
        self.btn_label = view.label;
    }

    fn on_language_changed(&mut self, new_lang: Language) {
        self.current = new_lang;
    }
}

pub struct MmoLangController {
    pub host: MmoLangHost,
    pub button: LangButton,
}

impl MmoLangController {
    pub fn new(slot: Rect) -> Self {
        lang::install_overrides(&MMO_CATALOG);
        lang::set_language(Language::Es);
        let mut host = MmoLangHost::new(slot);
        let button = LangButton::mount_in_slot(
            &Language::ALL,
            ButtonMode::Cycle,
            LabelStyle::CodeOnly,
            &mut host,
        );
        Self { host, button }
    }

    pub fn set_slot(&mut self, slot: Rect) {
        self.host.slot = slot;
        self.host.btn_bounds = slot;
    }

    pub fn handle_tap(&mut self, x: i32, y: i32) -> bool {
        if self.host.btn_bounds.contains(x, y) {
            self.button.handle_event(UiEvent::Click, &mut self.host)
        } else {
            false
        }
    }

    pub fn cycle(&mut self) {
        self.button.handle_event(UiEvent::Click, &mut self.host);
    }

    pub fn tr(&self, key: &'static str) -> &'static str {
        lang::tr(key)
    }

    pub fn tr_auto_connect(&self) -> &'static str {
        auto_labels::basics()[0]
    }
}

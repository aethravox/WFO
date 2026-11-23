#![allow(clippy::all)]
use super::*;
use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClassId {
    Vanguard = 0,
    Arcanist = 1,
    Ranger = 2,
    Guardian = 3,
    Duelist = 4,
    Summoner = 5,
    Artificer = 6,
    Chronicler = 7,
    Berserker = 8,
}

impl ClassId {
    pub fn from_str_code(s: &str) -> Self {
        match s {
            "arcanist" | "1" => Self::Arcanist,
            "ranger" | "2" => Self::Ranger,
            "guardian" | "3" => Self::Guardian,
            "duelist" | "4" => Self::Duelist,
            "summoner" | "5" => Self::Summoner,
            "artificer" | "6" => Self::Artificer,
            "chronicler" | "7" => Self::Chronicler,
            "berserker" | "8" => Self::Berserker,
            _ => Self::Vanguard,
        }
    }

    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Vanguard => "vanguard",
            Self::Arcanist => "arcanist",
            Self::Ranger => "ranger",
            Self::Guardian => "guardian",
            Self::Duelist => "duelist",
            Self::Summoner => "summoner",
            Self::Artificer => "artificer",
            Self::Chronicler => "chronicler",
            Self::Berserker => "berserker",
        }
    }

    pub const fn base_hp(self) -> i32 {
        match self {
            Self::Vanguard => 160,
            Self::Arcanist => 110,
            Self::Ranger => 130,
            Self::Guardian => 190,
            Self::Duelist => 130,
            Self::Summoner => 125,
            Self::Artificer => 145,
            Self::Chronicler => 125,
            Self::Berserker => 180,
        }
    }

    pub const fn base_mp(self) -> i32 {
        match self {
            Self::Vanguard => 70,
            Self::Arcanist => 140,
            Self::Ranger => 95,
            Self::Guardian => 90,
            Self::Duelist => 85,
            Self::Summoner => 145,
            Self::Artificer => 110,
            Self::Chronicler => 155,
            Self::Berserker => 65,
        }
    }

    pub const fn move_speed(self) -> f32 {
        match self {
            Self::Vanguard => 9.2,
            Self::Arcanist => 8.8,
            Self::Ranger => 10.4,
            Self::Guardian => 8.4,
            Self::Duelist => 11.2,
            Self::Summoner => 8.8,
            Self::Artificer => 8.6,
            Self::Chronicler => 8.2,
            Self::Berserker => 9.5,
        }
    }

    pub const fn attack_range(self) -> f32 {
        match self {
            Self::Vanguard => 4.6,
            Self::Arcanist => 14.5,
            Self::Ranger => 13.0,
            Self::Guardian => 5.0,
            Self::Duelist => 5.2,
            Self::Summoner => 14.0,
            Self::Artificer => 12.0,
            Self::Chronicler => 12.0,
            Self::Berserker => 4.8,
        }
    }

    pub const fn base_damage(self) -> i32 {
        match self {
            Self::Vanguard => 24,
            Self::Arcanist => 26,
            Self::Ranger => 22,
            Self::Guardian => 20,
            Self::Duelist => 23,
            Self::Summoner => 19,
            Self::Artificer => 23,
            Self::Chronicler => 18,
            Self::Berserker => 29,
        }
    }

    pub const fn accent_rgb(self) -> u32 {
        match self {
            Self::Vanguard => 0xF59E0B,
            Self::Arcanist => 0x38BDF8,
            Self::Ranger => 0x10B981,
            Self::Guardian => 0xC4B5FD,
            Self::Duelist => 0xFB7185,
            Self::Summoner => 0xA78BFA,
            Self::Artificer => 0xF97316,
            Self::Chronicler => 0xFDE047,
            Self::Berserker => 0xDC2626,
        }
    }

    pub const fn primary_rgb(self) -> u32 {
        match self {
            Self::Vanguard => 0xC27824,
            Self::Arcanist => 0x1D4ED8,
            Self::Ranger => 0x15803D,
            Self::Guardian => 0x6D28D9,
            Self::Duelist => 0x9F1239,
            Self::Summoner => 0x5B21B6,
            Self::Artificer => 0x9A3412,
            Self::Chronicler => 0xA16207,
            Self::Berserker => 0x7F1D1D,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Vanguard => "VANGUARDIA",
            Self::Arcanist => "ARCANISTA",
            Self::Ranger => "MONTARAZ",
            Self::Guardian => "GUARDIAN",
            Self::Duelist => "DUELISTA",
            Self::Summoner => "INVOCADOR",
            Self::Artificer => "ARTIFICE",
            Self::Chronicler => "CRONISTA",
            Self::Berserker => "BERSERKER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AdventurerRank {
    H,
    G,
    F,
    E,
    D,
    C,
    B,
    A,
    S,
    SS,
    SSS,
}

impl AdventurerRank {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::H => "H",
            Self::G => "G",
            Self::F => "F",
            Self::E => "E",
            Self::D => "D",
            Self::C => "C",
            Self::B => "B",
            Self::A => "A",
            Self::S => "S",
            Self::SS => "SS",
            Self::SSS => "SSS",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "H" => Some(Self::H),
            "G" => Some(Self::G),
            "F" => Some(Self::F),
            "E" => Some(Self::E),
            "D" => Some(Self::D),
            "C" => Some(Self::C),
            "B" => Some(Self::B),
            "A" => Some(Self::A),
            "S" => Some(Self::S),
            "SS" => Some(Self::SS),
            "SSS" => Some(Self::SSS),
            _ => None,
        }
    }

    pub const fn from_points(points: i32) -> Self {
        let points = if points < 0 { 0 } else { points };
        if points >= 10_000 {
            Self::SSS
        } else if points >= 7_400 {
            Self::SS
        } else if points >= 5_200 {
            Self::S
        } else if points >= 3_600 {
            Self::A
        } else if points >= 2_400 {
            Self::B
        } else if points >= 1_500 {
            Self::C
        } else if points >= 900 {
            Self::D
        } else if points >= 500 {
            Self::E
        } else if points >= 250 {
            Self::F
        } else if points >= 100 {
            Self::G
        } else {
            Self::H
        }
    }

    pub const fn threshold_points(self) -> i32 {
        match self {
            Self::H => 0,
            Self::G => 100,
            Self::F => 250,
            Self::E => 500,
            Self::D => 900,
            Self::C => 1_500,
            Self::B => 2_400,
            Self::A => 3_600,
            Self::S => 5_200,
            Self::SS => 7_400,
            Self::SSS => 10_000,
        }
    }

    pub const fn next_threshold_points(self) -> Option<i32> {
        match self {
            Self::H => Some(100),
            Self::G => Some(250),
            Self::F => Some(500),
            Self::E => Some(900),
            Self::D => Some(1_500),
            Self::C => Some(2_400),
            Self::B => Some(3_600),
            Self::A => Some(5_200),
            Self::S => Some(7_400),
            Self::SS => Some(10_000),
            Self::SSS => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmoteId {
    Wave,
    Cheer,
    Laugh,
    Dance,
}

impl EmoteId {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Cheer => "cheer",
            Self::Laugh => "laugh",
            Self::Dance => "dance",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "wave" => Some(Self::Wave),
            "cheer" => Some(Self::Cheer),
            "laugh" => Some(Self::Laugh),
            "dance" => Some(Self::Dance),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wave => "HOLA!",
            Self::Cheer => "VAMOS!",
            Self::Laugh => "JAJA!",
            Self::Dance => "BAILA!",
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
    pub rank_points: i32,
    pub attack_tick: u32,
    pub shield_hp: i32,
    pub haste: bool,
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

pub enum ClientPacket {
    RegisterAccount {
        username: String,
        password: String,
    },
    LoginAccount {
        username: String,
        password: String,
    },
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
        dx: f32,
        dz: f32,
        yaw: f32,
        moving: bool,
    },
    Dodge,
    Jump,
    Attack {
        skill: u8,
        target_id: String,
        yaw: f32,
    },
    Gather {
        crystal_id: String,
    },
    Emote {
        emote: EmoteId,
    },
    Chat {
        text: String,
    },
}

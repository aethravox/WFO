#![no_std]
extern crate alloc;

pub mod font;
pub mod raster3d;

mod engine;
mod input;
mod render;
mod hud;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lang::ui::Rect;
use mmo_protocol::{
    abs_f32, ceil_f32, clamp_f32, cos_approx, floor_f32, ground_height_at, sin_approx, sqrt_f32,
    zone_at, ChatEntry, ClassId, ClientPacket, CrystalSnapshot, MmoLangController, MobKind,
    LanPacket, MobSnapshot, PlayerSnapshot, ServerPacket, ZoneId, PLAYABLE_RADIUS,
};
use raster3d::{Camera, Framebuffer, Vec3};

pub struct ActiveRingFx {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: u32,
    pub age: f32,
    pub life: f32,
    pub max_r: f32,
}

pub struct FloatingPopup {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub text: String,
    pub color: u32,
    pub age: f32,
}

pub struct LanPeer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub udp_port: u16,
}

pub struct DuelUiState {
    pub opponent_id: String,
    pub opponent_name: String,
    pub round: i32,
    pub wins: i32,
    pub opponent_wins: i32,
    pub round_active: bool,
    pub round_winner: String,
}

pub struct InputState {
    pub w: bool,
    pub a: bool,
    pub s: bool,
    pub d: bool,
    pub dragging: bool,
    pub last_mouse_x: i32,
    pub last_mouse_y: i32,
    pub drag_dist: i32,
    pub joy_x: f32,
    pub joy_y: f32,
    pub joy_pointer: Option<u64>,
    pub cam_pointer: Option<u64>,
    pub cam_last_x: i32,
    pub cam_last_y: i32,
    pub cam_drag_dist: i32,
}

impl InputState {
    pub const fn new() -> Self {
        Self {
            w: false,
            a: false,
            s: false,
            d: false,
            dragging: false,
            last_mouse_x: 0,
            last_mouse_y: 0,
            drag_dist: 0,
            joy_x: 0.0,
            joy_y: 0.0,
            joy_pointer: None,
            cam_pointer: None,
            cam_last_x: 0,
            cam_last_y: 0,
            cam_drag_dist: 0,
        }
    }
}

pub struct MmoClientEngine {
    pub fb: Framebuffer,
    pub lang: MmoLangController,
    pub joined: bool,
    pub lobby_server_ready: bool,
    pub lobby_room_id: String,
    pub lobby_invite_input: String,
    pub lobby_invite_focus: bool,
    pub connecting: bool,
    pub self_id: Option<String>,
    pub lobby_name: String,
    pub lobby_class: ClassId,
    pub lobby_focus_name: bool,
    pub players: BTreeMap<String, PlayerSnapshot>,
    pub mobs: BTreeMap<String, MobSnapshot>,
    pub crystals: BTreeMap<String, CrystalSnapshot>,
    pub chat: Vec<ChatEntry>,
    pub chat_input: String,
    pub chat_focused: bool,
    pub target_mob_id: Option<String>,
    pub duel: Option<DuelUiState>,
    pub lan_mode: bool,
    pub lan_peers: Vec<LanPeer>,
    pub lan_duel_opponent: Option<String>,
    pub lan_pending_skill: Option<u8>,
    pub lan_remote_skill: Option<u8>,
    pub lan_duel_wins: i32,
    pub lan_duel_opponent_wins: i32,
    pub lan_duel_round: i32,
    pub lan_outbox: Vec<LanPacket>,
    pub local_x: f32,
    pub local_y: f32,
    pub local_z: f32,
    pub local_yaw: f32,
    pub local_vy: f32,
    pub grounded: bool,
    pub local_moving: bool,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_dist: f32,
    pub elapsed: f32,
    pub move_net_accum: f32,
    pub cooldowns: [f32; 3],
    pub input: InputState,
    pub rings: Vec<ActiveRingFx>,
    pub popups: Vec<FloatingPopup>,
    pub outbox: Vec<ClientPacket>,
    pub touch_mode: bool,
}


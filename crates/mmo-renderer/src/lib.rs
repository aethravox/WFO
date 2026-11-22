#![no_std]
extern crate alloc;

pub mod font;
pub mod raster3d;

mod engine;
mod hud;
mod input;
mod offline;
mod render;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lang::ui::Rect;
use mmo_protocol::{
    abs_f32, ceil_f32, clamp_f32, cos_approx, floor_f32, ground_height_at, sin_approx, sqrt_f32,
    zone_at, ChatEntry, ClassId, ClientPacket, CombatFx, CrystalSnapshot, LanPacket,
    MmoLangController, MobKind, MobSnapshot, PlayerSnapshot, ServerPacket, ZoneId, PLAYABLE_RADIUS,
    SAFE_PLAZA_RADIUS,
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
    pub auth_username: String,
    pub auth_password: String,
    pub auth_status: String,
    pub auth_connected: bool,
    pub account_authenticated: bool,
    pub offline_mode: bool,
    pub offline_level: i32,
    pub offline_xp: i32,
    pub offline_xp_next: i32,
    pub offline_gold: i32,
    pub auth_panel_open: bool,
    pub auth_password_focus: bool,
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
    offline_mob_respawns: BTreeMap<String, f32>,
    offline_mob_attack_timers: BTreeMap<String, f32>,
    offline_crystal_respawns: BTreeMap<String, f32>,
    offline_regen_at: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_render_collects_scene_triangles_and_transparent_hud() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.render_gpu();

        assert!(!engine.fb.scene_vertices.is_empty());
        assert_eq!(engine.fb.scene_vertices.len() % 3, 0);
        assert!(engine.fb.ui_pixels.iter().any(|pixel| pixel >> 24 != 0));
        assert_eq!(
            engine.fb.gpu_background_pixels.len(),
            engine.fb.pixels.len()
        );
        assert_ne!(engine.fb.gpu_background_pixels, engine.fb.pixels);
    }

    #[test]
    fn software_render_path_remains_available_for_wasm() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.render();

        assert!(engine.fb.scene_vertices.is_empty());
        assert!(engine.fb.pixels.iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn gpu_depth_uses_perspective_projection() {
        let mut framebuffer = Framebuffer::new(100, 100);
        framebuffer.set_gpu_scene_mode(true);
        let camera = Camera::look_at(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 100);
        framebuffer.draw_triangle(
            &camera,
            Vec3::new(-1.0, -1.0, 2.0),
            Vec3::new(1.0, -1.0, 2.0),
            Vec3::new(0.0, 1.0, 2.0),
            0xFFFFFF,
            false,
        );

        assert_eq!(framebuffer.scene_vertices.len(), 3);
        let expected = 145.0 / 144.65 * (1.0 - 0.35 / 2.0);
        assert!((framebuffer.scene_vertices[0].position[2] - expected).abs() < 0.0001);
    }

    #[test]
    fn offline_world_gathering_updates_progress_without_network_packets() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.submit_join();
        engine.local_x = -26.0;
        engine.local_z = 26.0;
        engine.sync_offline_player();
        engine.trigger_gather();

        assert!(engine.joined);
        assert!(engine.outbox.is_empty());
        assert_eq!((engine.offline_xp, engine.offline_gold), (22, 18));
        assert!(!engine.crystals["local_crystal_5"].available);
        assert_eq!(engine.players["offline_player"].xp, 22);
    }

    #[test]
    fn offline_combat_grants_rewards_without_network_packets() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.lobby_class = ClassId::Arcanist;
        engine.start_offline_world();
        engine.local_x = -32.0;
        engine.local_z = 18.0;
        engine.sync_offline_player();
        engine.target_mob_id = Some(String::from("local_mob_1"));

        engine.trigger_skill(1);
        engine.cooldowns[1] = 0.0;
        engine.trigger_skill(1);

        assert!(engine.outbox.is_empty());
        assert!(!engine.mobs["local_mob_1"].alive);
        assert_eq!((engine.offline_xp, engine.offline_gold), (28, 12));
        assert_eq!(engine.players["offline_player"].level, 1);
    }
}

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
    zone_at, AdventurerRank, ChatEntry, ClassId, ClientPacket, CombatFx, CrystalSnapshot, EmoteId,
    LanPacket, MmoLangController, MobKind, MobSnapshot, PlayerSnapshot, ServerPacket, ZoneId,
    PLAYABLE_RADIUS, SAFE_PLAZA_RADIUS,
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
    pub joy_origin_x: i32,
    pub joy_origin_y: i32,
    pub joy_pointer: Option<u64>,
    pub cam_pointer: Option<u64>,
    pub cam_last_x: i32,
    pub cam_last_y: i32,
    pub cam_drag_dist: i32,
    pub pinch_pointer: Option<u64>,
    pub pinch_last_x: i32,
    pub pinch_last_y: i32,
    pub pinch_last_distance: f32,
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
            joy_origin_x: 0,
            joy_origin_y: 0,
            joy_pointer: None,
            cam_pointer: None,
            cam_last_x: 0,
            cam_last_y: 0,
            cam_drag_dist: 0,
            pinch_pointer: None,
            pinch_last_x: 0,
            pinch_last_y: 0,
            pinch_last_distance: 0.0,
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
    pub offline_rank_points: i32,
    pub offline_has_card: bool,
    pub offline_rank: AdventurerRank,
    pub guild_notice: Option<(String, f32)>,
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
    pub dodge_cooldown: f32,
    pub gathering_id: Option<String>,
    pub gathering_progress: f32,
    pub input: InputState,
    pub rings: Vec<ActiveRingFx>,
    pub popups: Vec<FloatingPopup>,
    pub discovered_zones: [bool; 4],
    pub personal_marker: Option<(f32, f32)>,
    pub bestiary_known: [bool; 4],
    pub bestiary_open: bool,
    pub touch_emote_menu: bool,
    pub outbox: Vec<ClientPacket>,
    pub touch_mode: bool,
    offline_mob_respawns: BTreeMap<String, f32>,
    offline_mob_attack_timers: BTreeMap<String, f32>,
    offline_crystal_respawns: BTreeMap<String, f32>,
    offline_shield_expires_at: f32,
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
        engine.offline_rank_points = 90;
        engine.submit_join();
        engine.local_x = -26.0;
        engine.local_z = 26.0;
        engine.sync_offline_player();
        engine.trigger_gather();

        assert!(engine.joined);
        assert!(engine.gathering_id.is_some());
        assert!(engine.gathering_progress > 0.0);
        for _ in 0..40 {
            engine.tick(0.08);
        }
        assert!(engine.outbox.is_empty());
        assert_eq!((engine.offline_xp, engine.offline_gold), (22, 18));
        assert_eq!(engine.offline_rank_points, 112);
        assert_eq!(
            AdventurerRank::from_points(engine.offline_rank_points),
            AdventurerRank::G
        );
        assert!(engine.popups.iter().any(|popup| popup.text == "RANGO G!"));
        assert!(!engine.crystals["local_crystal_5"].available);
        assert_eq!(engine.players["offline_player"].xp, 22);
    }

    #[test]
    fn guardian_shield_absorbs_offline_damage_until_it_expires() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.lobby_class = ClassId::Guardian;
        engine.start_offline_world();
        engine.local_x = -32.0;
        engine.local_z = 18.0;
        engine.sync_offline_player();
        engine.offline_attack(2);

        let player = &engine.players["offline_player"];
        assert_eq!(player.shield_hp, 57);
        let hp_before = player.hp;

        engine.elapsed = 2.0;
        engine.tick_offline_world();

        let player = &engine.players["offline_player"];
        assert_eq!(player.hp, hp_before);
        assert_eq!(player.shield_hp, 48);

        engine.elapsed = 9.0;
        engine.tick_offline_world();
        assert_eq!(engine.players["offline_player"].shield_hp, 0);
    }

    #[test]
    fn exploration_discovers_zones_and_nearby_creatures() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.start_offline_world();
        engine.local_x = -32.0;
        engine.local_z = 18.0;
        engine.sync_offline_player();
        engine.tick(0.016);

        assert!(engine.discovered_zones[1]);
        assert!(engine.bestiary_known[0]);
        assert!(engine
            .popups
            .iter()
            .any(|popup| popup.text.starts_with("ZONA DESCUBIERTA:")));
    }

    #[test]
    fn personal_marker_toggles_and_offline_emotes_are_visible() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.start_offline_world();
        engine.local_x = 12.0;
        engine.local_z = 13.0;

        engine.toggle_personal_marker();
        assert_eq!(engine.personal_marker, Some((12.0, 13.0)));
        engine.toggle_personal_marker();
        assert_eq!(engine.personal_marker, None);

        engine.trigger_emote(EmoteId::Wave);
        assert!(engine.popups.iter().any(|popup| popup.text == "HOLA!"));
        assert!(engine.outbox.is_empty());
    }

    #[test]
    fn level_up_creates_a_ring_animation() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.offline_mode = true;
        engine.start_offline_world();
        engine.apply_server_packet(ServerPacket::LevelUp {
            id: String::from("offline_player"),
            level: 2,
        });

        assert_eq!(engine.rings.len(), 1);
        assert_eq!(engine.rings[0].color, 0xFBBF24);
        assert!(engine.popups.iter().any(|popup| popup.text == "LV 2!"));
    }

    #[test]
    fn new_offline_classes_execute_their_special_actions() {
        for class_id in [
            ClassId::Duelist,
            ClassId::Summoner,
            ClassId::Artificer,
            ClassId::Chronicler,
            ClassId::Berserker,
        ] {
            let mut engine = MmoClientEngine::new(480, 320);
            engine.offline_mode = true;
            engine.lobby_class = class_id;
            engine.start_offline_world();
            engine.offline_attack(2);
            assert!(engine.joined);
            assert_eq!(engine.players["offline_player"].class_id, class_id);
        }
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

    #[test]
    fn virtual_joystick_and_action_buttons_work_in_lobby_and_world() {
        let mut engine = MmoClientEngine::new(480, 320);
        engine.enter_offline_mode();

        // Lobby floating virtual joystick movement inside bottom-left capture zone
        engine.on_touch_start(1, 74, 165);
        engine.on_touch_move(1, 110, 165);
        assert!(engine.touch_mode);
        assert!(engine.input.joy_x > 0.5);
        let x_before = engine.local_x;
        engine.tick(0.1);
        assert!(engine.local_x > x_before);
        engine.on_touch_end(1, 110, 165);
        assert_eq!(engine.input.joy_x, 0.0);

        // Lobby interact button (h - 156..h - 110) enters world when standing by the portal
        engine.local_x = 0.0;
        engine.local_z = -9.0;
        engine.on_touch_start(2, 480 - 50, 320 - 130);
        assert!(engine.joined);

        // Multi-touch world combat: hold floating joystick + tap Dodge
        engine.on_touch_start(3, 74, 170);
        engine.on_touch_move(3, 74, 130);
        engine.on_touch_start(4, 480 - 128, 320 - 110);
        assert!(engine.dodge_cooldown > 0.0);
        assert!(engine.input.joy_y < -0.5);
    }

    #[test]
    fn mobile_portrait_hud_and_chat_render_cleanly() {
        let mut engine = MmoClientEngine::new(360, 740);
        engine.enter_offline_mode();
        engine.touch_mode = true;
        engine.tick(0.016);
        engine.render();

        // Enter world and toggle mobile chat overlay
        engine.start_offline_world();
        engine.on_touch_start(10, 360 - 50, 740 - 165);
        assert!(engine.chat_focused);
        engine.on_char_input('H');
        engine.render();
        assert_eq!(engine.chat_input, "H");
    }
}

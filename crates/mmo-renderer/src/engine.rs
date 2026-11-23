use super::*;

mod engine_actions;
mod engine_auth_tick;
mod engine_lan;
mod engine_packets;
mod engine_pointer;
mod engine_touch;

impl MmoClientEngine {
    pub fn new(width: usize, height: usize) -> Self {
        let slot = Rect::new(width as i32 - 86, 16, 70, 30);
        Self {
            fb: Framebuffer::new(width, height),
            lang: MmoLangController::new(slot),
            joined: false,
            lobby_server_ready: false,
            lobby_room_id: String::new(),
            lobby_invite_input: String::new(),
            lobby_invite_focus: false,
            connecting: false,
            self_id: None,
            lobby_name: String::from("Wayfarer"),
            lobby_class: ClassId::Vanguard,
            lobby_focus_name: false,
            auth_username: String::new(),
            auth_password: String::new(),
            auth_status: String::from("Conectando al servidor seguro..."),
            auth_connected: false,
            account_authenticated: false,
            offline_mode: false,
            offline_level: 1,
            offline_xp: 0,
            offline_xp_next: 100,
            offline_gold: 0,
            offline_rank_points: 0,
            auth_panel_open: true,
            auth_password_focus: false,
            players: BTreeMap::new(),
            mobs: BTreeMap::new(),
            crystals: BTreeMap::new(),
            chat: Vec::new(),
            chat_input: String::new(),
            chat_focused: false,
            target_mob_id: None,
            duel: None,
            lan_mode: false,
            lan_peers: Vec::new(),
            lan_duel_opponent: None,
            lan_pending_skill: None,
            lan_remote_skill: None,
            lan_duel_wins: 0,
            lan_duel_opponent_wins: 0,
            lan_duel_round: 1,
            lan_outbox: Vec::new(),
            local_x: 0.0,
            local_y: 0.0,
            local_z: 4.0,
            local_yaw: 3.14159,
            local_vy: 0.0,
            grounded: true,
            local_moving: false,
            cam_yaw: 0.0,
            cam_pitch: 0.38,
            cam_dist: 12.5,
            elapsed: 0.0,
            move_net_accum: 0.0,
            cooldowns: [0.0, 0.0, 0.0],
            dodge_cooldown: 0.0,
            gathering_id: None,
            gathering_progress: 0.0,
            input: InputState::new(),
            rings: Vec::new(),
            popups: Vec::new(),
            discovered_zones: [false; 4],
            personal_marker: None,
            bestiary_known: [false; 4],
            bestiary_open: false,
            outbox: Vec::new(),
            touch_mode: false,
            touch_emote_menu: false,
            offline_mob_respawns: BTreeMap::new(),
            offline_mob_attack_timers: BTreeMap::new(),
            offline_crystal_respawns: BTreeMap::new(),
            offline_shield_expires_at: 0.0,
            offline_regen_at: 0.0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let w = width.max(320);
        let h = height.max(240);
        self.fb.resize(w, h);
        self.lang.set_slot(Rect::new(w as i32 - 84, 12, 70, 30));
    }

    pub fn auth_panel_layout(&self) -> (i32, i32, i32) {
        let width = (self.fb.width as i32 - 20).min(340);
        let height = 146;
        (
            (self.fb.width as i32 - width) / 2,
            (self.fb.height as i32 - height) / 2,
            width,
        )
    }
}

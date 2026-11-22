#![no_std]
extern crate alloc;

pub mod font;
pub mod raster3d;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lang::ui::Rect;
use mmo_protocol::{
    abs_f32, ceil_f32, clamp_f32, cos_approx, floor_f32, ground_height_at, sin_approx, sqrt_f32,
    zone_at, ChatEntry, ClassId, ClientPacket, CrystalSnapshot, MmoLangController, MobKind,
    MobSnapshot, PlayerSnapshot, ServerPacket, ZoneId, PLAYABLE_RADIUS,
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

impl MmoClientEngine {
    pub fn new(width: usize, height: usize) -> Self {
        let slot = Rect::new(width as i32 - 86, 16, 70, 30);
        Self {
            fb: Framebuffer::new(width, height),
            lang: MmoLangController::new(slot),
            joined: false,
            connecting: false,
            self_id: None,
            lobby_name: String::from("Wayfarer"),
            lobby_class: ClassId::Vanguard,
            lobby_focus_name: false,
            players: BTreeMap::new(),
            mobs: BTreeMap::new(),
            crystals: BTreeMap::new(),
            chat: Vec::new(),
            chat_input: String::new(),
            chat_focused: false,
            target_mob_id: None,
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
            input: InputState::new(),
            rings: Vec::new(),
            popups: Vec::new(),
            outbox: Vec::new(),
            touch_mode: false,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let w = width.max(320);
        let h = height.max(240);
        self.fb.resize(w, h);
        self.lang.set_slot(Rect::new(w as i32 - 86, 16, 70, 30));
    }

    pub fn apply_server_packet(&mut self, pkt: ServerPacket) {
        match pkt {
            ServerPacket::Init { self_id } => {
                self.self_id = Some(self_id);
                self.joined = true;
                self.connecting = false;
            }
            ServerPacket::Player(p) => {
                if let Some(ref sid) = self.self_id {
                    if &p.id == sid {
                        let drift = sqrt_f32(
                            (p.x - self.local_x) * (p.x - self.local_x)
                                + (p.z - self.local_z) * (p.z - self.local_z),
                        );
                        if drift > 16.0 || self.elapsed < 0.5 {
                            self.local_x = p.x;
                            self.local_y = p.y;
                            self.local_z = p.z;
                        }
                    }
                }
                self.players.insert(p.id.clone(), p);
            }
            ServerPacket::Mob(m) => {
                self.mobs.insert(m.id.clone(), m);
            }
            ServerPacket::Crystal(c) => {
                self.crystals.insert(c.id.clone(), c);
            }
            ServerPacket::PlayerLeft { id } => {
                self.players.remove(&id);
            }
            ServerPacket::CombatFx(fx) => {
                let color = if fx.heal > 0 {
                    0x10B981
                } else if fx.skill == 3 {
                    0x22D3EE
                } else {
                    fx.class_id.accent_rgb()
                };
                self.rings.push(ActiveRingFx {
                    x: fx.x,
                    y: ground_height_at(fx.x, fx.z) + 0.12,
                    z: fx.z,
                    color,
                    age: 0.0,
                    life: 0.45,
                    max_r: if fx.skill == 1 { 4.8 } else { 2.4 },
                });
                if fx.damage > 0 {
                    self.popups.push(FloatingPopup {
                        x: fx.x,
                        y: fx.y + 0.6,
                        z: fx.z,
                        text: if fx.crit {
                            format!("CRIT -{}", fx.damage)
                        } else {
                            format!("-{}", fx.damage)
                        },
                        color: if fx.crit { 0xFBBF24 } else { 0xFFFFFF },
                        age: 0.0,
                    });
                } else if fx.heal > 0 {
                    self.popups.push(FloatingPopup {
                        x: fx.x,
                        y: fx.y + 0.8,
                        z: fx.z,
                        text: format!("+{}", fx.heal),
                        color: 0x34D399,
                        age: 0.0,
                    });
                }
                if fx.xp_gained > 0 {
                    if let Some(ref sid) = self.self_id {
                        if &fx.attacker_id == sid {
                            self.popups.push(FloatingPopup {
                                x: fx.x,
                                y: fx.y + 1.3,
                                z: fx.z,
                                text: format!("+{} XP", fx.xp_gained),
                                color: 0x38BDF8,
                                age: 0.0,
                            });
                        }
                    }
                }
            }
            ServerPacket::LevelUp { id, level } => {
                if let Some(p) = self.players.get(&id) {
                    self.popups.push(FloatingPopup {
                        x: p.x,
                        y: p.y + 2.4,
                        z: p.z,
                        text: format!("LV {}!", level),
                        color: 0xF59E0B,
                        age: 0.0,
                    });
                }
            }
            ServerPacket::Chat(c) => {
                self.chat.push(c);
                if self.chat.len() > 24 {
                    self.chat.remove(0);
                }
            }
            ServerPacket::SnapshotEnd => {}
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.001, 0.08);
        self.elapsed += dt;

        for cd in &mut self.cooldowns {
            if *cd > 0.0 {
                *cd = (*cd - dt).max(0.0);
            }
        }

        // Update VFX
        let mut i = self.rings.len();
        while i > 0 {
            i -= 1;
            self.rings[i].age += dt;
            if self.rings[i].age >= self.rings[i].life {
                self.rings.remove(i);
            }
        }

        let mut j = self.popups.len();
        while j > 0 {
            j -= 1;
            self.popups[j].age += dt;
            self.popups[j].y += dt * 1.3;
            if self.popups[j].age >= 0.95 {
                self.popups.remove(j);
            }
        }

        if self.joined {
            let speed = self
                .self_id
                .as_ref()
                .and_then(|id| self.players.get(id))
                .map(|p| p.class_id.move_speed())
                .unwrap_or(9.0);

            let mut mx = 0.0f32;
            let mut mz = 0.0f32;
            if !self.chat_focused {
                if self.input.w {
                    mz -= 1.0;
                }
                if self.input.s {
                    mz += 1.0;
                }
                if self.input.a {
                    mx -= 1.0;
                }
                if self.input.d {
                    mx += 1.0;
                }
                mx += self.input.joy_x;
                mz += self.input.joy_y;
            }

            let mag_sq = mx * mx + mz * mz;
            if mag_sq > 0.02 {
                let len = sqrt_f32(mag_sq).max(1.0);
                let nx = mx / len;
                let nz = mz / len;
                let sy = sin_approx(self.cam_yaw);
                let cy = cos_approx(self.cam_yaw);
                let wx = nx * cy + nz * sy;
                let wz = -nx * sy + nz * cy;

                self.local_x += wx * speed * dt;
                self.local_z += wz * speed * dt;
                let d = sqrt_f32(self.local_x * self.local_x + self.local_z * self.local_z);
                if d > PLAYABLE_RADIUS {
                    self.local_x = (self.local_x / d) * PLAYABLE_RADIUS;
                    self.local_z = (self.local_z / d) * PLAYABLE_RADIUS;
                }
                self.local_yaw = mmo_protocol::atan2_approx(wx, wz);
                self.local_moving = true;
            } else {
                self.local_moving = false;
            }

            let gy = ground_height_at(self.local_x, self.local_z);
            if !self.grounded || self.local_y > gy {
                self.local_vy -= 22.0 * dt;
                self.local_y += self.local_vy * dt;
                if self.local_y <= gy {
                    self.local_y = gy;
                    self.local_vy = 0.0;
                    self.grounded = true;
                }
            } else {
                self.local_y = gy;
            }

            self.move_net_accum += dt;
            if self.move_net_accum >= 0.05 {
                self.move_net_accum = 0.0;
                self.outbox.push(ClientPacket::Move {
                    x: self.local_x,
                    y: self.local_y,
                    z: self.local_z,
                    yaw: self.local_yaw,
                    moving: self.local_moving,
                });
            }
        }
    }

    pub fn trigger_skill(&mut self, skill: u8) {
        if !self.joined {
            return;
        }
        let s_idx = match skill {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        if self.cooldowns[s_idx] > 0.0 {
            return;
        }
        self.cooldowns[s_idx] = match s_idx {
            0 => 0.52,
            1 => 3.0,
            _ => 5.0,
        };
        let target_id = self.target_mob_id.clone().unwrap_or_default();
        self.outbox.push(ClientPacket::Attack {
            skill: s_idx as u8,
            target_id,
            yaw: self.local_yaw,
        });
    }

    pub fn trigger_gather(&mut self) {
        if !self.joined {
            return;
        }
        let mut best_id: Option<String> = None;
        let mut best_d = 6.5f32;
        for c in self.crystals.values() {
            if !c.available {
                continue;
            }
            let dx = c.x - self.local_x;
            let dz = c.z - self.local_z;
            let d = sqrt_f32(dx * dx + dz * dz);
            if d < best_d {
                best_d = d;
                best_id = Some(c.id.clone());
            }
        }
        if let Some(cid) = best_id {
            self.outbox.push(ClientPacket::Gather { crystal_id: cid });
        }
    }

    pub fn trigger_jump(&mut self) {
        if self.joined && self.grounded && !self.chat_focused {
            self.local_vy = 8.4;
            self.grounded = false;
        }
    }

    pub fn on_char_input(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        if !self.joined && self.lobby_focus_name {
            if self.lobby_name.len() < 16 && ch != '|' {
                self.lobby_name.push(ch);
            }
        } else if self.joined && self.chat_focused {
            if self.chat_input.len() < 120 && ch != '|' {
                self.chat_input.push(ch);
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if !self.joined && self.lobby_focus_name {
            self.lobby_name.pop();
        } else if self.joined && self.chat_focused {
            self.chat_input.pop();
        }
    }

    pub fn on_enter(&mut self) {
        if !self.joined {
            self.submit_join();
            return;
        }
        if self.chat_focused {
            let trimmed = self.chat_input.trim().to_string();
            if !trimmed.is_empty() {
                self.outbox.push(ClientPacket::Chat { text: trimmed });
                self.chat_input.clear();
            }
            self.chat_focused = false;
        } else {
            self.chat_focused = true;
        }
    }

    pub fn submit_join(&mut self) {
        if self.connecting {
            return;
        }
        self.connecting = true;
        let name = if self.lobby_name.trim().is_empty() {
            String::from("Hero")
        } else {
            self.lobby_name.trim().to_string()
        };
        self.outbox.push(ClientPacket::Join {
            name,
            class_id: self.lobby_class,
        });
    }

    pub fn on_pointer_down(&mut self, x: i32, y: i32) {
        self.input.dragging = true;
        self.input.last_mouse_x = x;
        self.input.last_mouse_y = y;
        self.input.drag_dist = 0;
    }

    pub fn on_pointer_move(&mut self, x: i32, y: i32) {
        if !self.input.dragging || !self.joined {
            return;
        }
        let dx = x - self.input.last_mouse_x;
        let dy = y - self.input.last_mouse_y;
        self.input.drag_dist += dx.abs() + dy.abs();
        self.input.last_mouse_x = x;
        self.input.last_mouse_y = y;
        self.cam_yaw -= (dx as f32) * 0.007;
        self.cam_pitch = clamp_f32(self.cam_pitch + (dy as f32) * 0.0055, 0.14, 1.08);
    }

    pub fn on_pointer_up(&mut self, x: i32, y: i32) {
        let was_click = self.input.dragging && self.input.drag_dist < 8;
        self.input.dragging = false;
        if !was_click {
            return;
        }

        // 1. Check LangButton in ready_slot (powered by aethravox/Lang!)
        if self.lang.handle_tap(x, y) {
            return;
        }

        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        // Touch / Pad HUD toggle button (left of LangButton)
        if x >= w - 154 && x <= w - 92 && y >= 16 && y <= 46 {
            self.touch_mode = !self.touch_mode;
            return;
        }

        // Virtual Jump button when touch_mode is active in-game
        if self.joined && self.touch_mode && x >= w - 88 && x <= w - 16 && y >= h - 134 && y <= h - 88 {
            self.trigger_jump();
            return;
        }

        if !self.joined {
            // Lobby UI hitboxes
            let panel_w = 540.min(w - 32);
            let panel_x = (w - panel_w) / 2;
            let panel_y = h - 245;

            // Name input box
            if x >= panel_x + 16 && x <= panel_x + panel_w - 170 && y >= panel_y + 34 && y <= panel_y + 66 {
                self.lobby_focus_name = true;
                return;
            }
            self.lobby_focus_name = false;

            // Enter World button
            if x >= panel_x + panel_w - 160 && x <= panel_x + panel_w - 16 && y >= panel_y + 34 && y <= panel_y + 66 {
                self.submit_join();
                return;
            }

            // 3 Class cards
            let card_w = (panel_w - 48) / 3;
            let card_y = panel_y + 98;
            for idx in 0..3 {
                let cx = panel_x + 16 + (idx as i32) * (card_w + 8);
                if x >= cx && x <= cx + card_w && y >= card_y && y <= card_y + 72 {
                    self.lobby_class = match idx {
                        0 => ClassId::Vanguard,
                        1 => ClassId::Arcanist,
                        _ => ClassId::Ranger,
                    };
                    return;
                }
            }
            return;
        }

        // In-Game HUD hitboxes
        // Chat input box
        if x >= 16 && x <= 290 && y >= h - 46 && y <= h - 16 {
            self.chat_focused = true;
            return;
        }
        self.chat_focused = false;

        // Action Bar (Bottom-Center)
        let bar_x = (w - 280) / 2;
        let bar_y = h - 68;
        if y >= bar_y && y <= bar_y + 52 && x >= bar_x && x <= bar_x + 280 {
            let slot = (x - bar_x) / 70;
            match slot {
                0 => self.trigger_skill(0),
                1 => self.trigger_skill(1),
                2 => self.trigger_skill(2),
                _ => self.trigger_gather(),
            }
            return;
        }

        // World click: target nearest mob near click ray, else basic attack
        let cam = self.current_camera();
        let mut clicked_mob: Option<String> = None;
        let mut best_screen_d = 48.0f32;
        for m in self.mobs.values() {
            if !m.alive {
                continue;
            }
            let hy = if m.kind == MobKind::Colossus { 2.2 } else { 1.1 };
            if let Some((sx, sy, _)) = cam.project_point(Vec3::new(m.x, m.y + hy, m.z), self.fb.width, self.fb.height) {
                let d = sqrt_f32((sx - x as f32) * (sx - x as f32) + (sy - y as f32) * (sy - y as f32));
                if d < best_screen_d {
                    best_screen_d = d;
                    clicked_mob = Some(m.id.clone());
                }
            }
        }
        if let Some(mid) = clicked_mob {
            self.target_mob_id = Some(mid);
        } else {
            self.trigger_skill(0);
        }
    }

    pub fn on_wheel(&mut self, delta: f32) {
        if self.joined {
            self.cam_dist = clamp_f32(self.cam_dist + delta * 0.8, 6.5, 22.0);
        }
    }

    pub fn on_touch_start(&mut self, id: u64, x: i32, y: i32) {
        self.touch_mode = true;
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        if self.joined {
            // Virtual joystick zone (left side, above chat)
            let jx = 68;
            let jy = h - 208;
            let dx = x - jx;
            let dy = y - jy;
            if dx * dx + dy * dy <= 62 * 62 && self.input.joy_pointer.is_none() {
                self.input.joy_pointer = Some(id);
                self.update_joystick(x, y);
                return;
            }

            // Virtual Jump button (right side)
            if x >= w - 88 && x <= w - 16 && y >= h - 134 && y <= h - 88 {
                self.trigger_jump();
                return;
            }
        }

        if self.input.cam_pointer.is_none() {
            self.input.cam_pointer = Some(id);
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.input.cam_drag_dist = 0;
        }
    }

    pub fn on_touch_move(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.update_joystick(x, y);
            return;
        }
        if self.input.cam_pointer == Some(id) && self.joined {
            let dx = x - self.input.cam_last_x;
            let dy = y - self.input.cam_last_y;
            self.input.cam_drag_dist += dx.abs() + dy.abs();
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.cam_yaw -= (dx as f32) * 0.007;
            self.cam_pitch = clamp_f32(self.cam_pitch + (dy as f32) * 0.0055, 0.14, 1.08);
        }
    }

    pub fn on_touch_end(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.input.joy_pointer = None;
            self.input.joy_x = 0.0;
            self.input.joy_y = 0.0;
            return;
        }
        if self.input.cam_pointer == Some(id) {
            self.input.cam_pointer = None;
            if self.input.cam_drag_dist < 12 {
                // Emulate click at tap point
                self.input.dragging = true;
                self.input.drag_dist = 0;
                self.on_pointer_up(x, y);
            }
        }
    }

    fn update_joystick(&mut self, x: i32, y: i32) {
        let h = self.fb.height as i32;
        let dx = (x - 68) as f32;
        let dy = (y - (h - 208)) as f32;
        let max_r = 42.0f32;
        let dist = sqrt_f32(dx * dx + dy * dy);
        if dist > max_r {
            self.input.joy_x = dx / dist;
            self.input.joy_y = dy / dist;
        } else if dist > 2.0 {
            self.input.joy_x = dx / max_r;
            self.input.joy_y = dy / max_r;
        } else {
            self.input.joy_x = 0.0;
            self.input.joy_y = 0.0;
        }
    }

    fn current_camera(&self) -> Camera {
        if self.joined {
            let target = Vec3::new(self.local_x, self.local_y + 1.5, self.local_z);
            let horiz = cos_approx(self.cam_pitch) * self.cam_dist;
            let vert = sin_approx(self.cam_pitch) * self.cam_dist;
            let cam_pos = Vec3::new(
                self.local_x + sin_approx(self.cam_yaw) * horiz,
                self.local_y + 1.5 + vert,
                self.local_z + cos_approx(self.cam_yaw) * horiz,
            );
            Camera::look_at(cam_pos, target, self.fb.width)
        } else {
            let ang = self.elapsed * 0.16;
            let cam_pos = Vec3::new(
                sin_approx(ang) * 24.0,
                10.5 + sin_approx(self.elapsed * 0.35) * 1.1,
                cos_approx(ang) * 24.0,
            );
            Camera::look_at(cam_pos, Vec3::new(0.0, 2.2, -14.0), self.fb.width)
        }
    }

    pub fn render(&mut self) {
        self.fb.clear_sky();
        let cam = self.current_camera();

        self.render_terrain(&cam);
        self.render_landmarks(&cam);
        self.render_forest_and_crystals(&cam);
        self.render_entities(&cam);
        self.render_vfx(&cam);
        self.render_hud(&cam);
    }

    fn render_terrain(&mut self, cam: &Camera) {
        // Render sculpted terrain grid centered around camera focus
        let focus_x = if self.joined { self.local_x } else { 0.0 };
        let focus_z = if self.joined { self.local_z } else { -8.0 };
        let step = 7.5f32;
        let gx0 = floor_f32((focus_x - 75.0) / step);
        let gx1 = ceil_f32((focus_x + 75.0) / step);
        let gz0 = floor_f32((focus_z - 75.0) / step);
        let gz1 = ceil_f32((focus_z + 75.0) / step);

        for gz in gz0..gz1 {
            for gx in gx0..gx1 {
                let x0 = (gx as f32) * step;
                let z0 = (gz as f32) * step;
                let x1 = x0 + step;
                let z1 = z0 + step;

                let p00 = Vec3::new(x0, ground_height_at(x0, z0), z0);
                let p10 = Vec3::new(x1, ground_height_at(x1, z0), z0);
                let p11 = Vec3::new(x1, ground_height_at(x1, z1), z1);
                let p01 = Vec3::new(x0, ground_height_at(x0, z1), z1);

                let cx = (x0 + x1) * 0.5;
                let cz = (z0 + z1) * 0.5;
                let dist = sqrt_f32(cx * cx + cz * cz);
                let rgb = if dist < 16.0 {
                    0x536275 // Sanctuary flagstones
                } else if dist > 82.0 {
                    0x3E4C5E // Outer mountain cliff
                } else if cz < -32.0 && abs_f32(cx) < 32.0 {
                    0x2A3240 // Obsidian Citadel basalt
                } else if cx > 22.0 {
                    0x2F5254 // Crystal Veins slate
                } else if ((gx + gz) & 1) == 0 {
                    0x2F6B3E // Emerald meadow A
                } else {
                    0x285E36 // Emerald meadow B
                };

                self.fb.draw_quad(cam, p00, p10, p11, p01, rgb, false);
            }
        }
    }

    fn render_landmarks(&mut self, cam: &Camera) {
        // 6 Sanctuary Rune Pillars + Floating Cyan Crystals
        for i in 0..6 {
            let ang = (i as f32 / 6.0) * 6.283185 + 0.5236;
            let px = cos_approx(ang) * 14.2;
            let pz = sin_approx(ang) * 14.2;
            self.fb.draw_box(cam, Vec3::new(px, 2.0, pz), Vec3::new(0.45, 2.0, 0.45), ang, 0x526072, false);
            self.fb.draw_box(cam, Vec3::new(px, 4.15, pz), Vec3::new(0.6, 0.2, 0.6), ang, 0xD97706, false);
            let float_y = 4.9 + sin_approx(self.elapsed * 2.3 + i as f32) * 0.16;
            self.fb.draw_pyramid(cam, Vec3::new(px, float_y, pz), 0.3, 0.65, self.elapsed + i as f32, 0x38BDF8, true);
        }

        // Grand North Waystone Portal Arch (0, 0, -14.5)
        self.fb.draw_box(cam, Vec3::new(-3.2, 3.3, -14.5), Vec3::new(0.55, 3.3, 0.55), 0.0, 0x475569, false);
        self.fb.draw_box(cam, Vec3::new(3.2, 3.3, -14.5), Vec3::new(0.55, 3.3, 0.55), 0.0, 0x475569, false);
        self.fb.draw_box(cam, Vec3::new(0.0, 6.9, -14.5), Vec3::new(4.1, 0.55, 0.7), 0.0, 0x475569, false);
        self.fb.draw_ring_3d(cam, Vec3::new(0.0, 0.12, 0.0), 14.8, 15.2, 0x38BDF8);

        // Northern Obsidian Citadel Monoliths (0, -58)
        let cy = ground_height_at(0.0, -58.0);
        self.fb.draw_box(cam, Vec3::new(0.0, cy + 0.3, -58.0), Vec3::new(6.8, 0.35, 6.8), 0.39, 0x1E2530, false);
        let spires = [(7.5, 7.5), (-7.5, 7.5), (7.5, -7.5), (-7.5, -7.5)];
        for (idx, (sx, sz)) in spires.iter().enumerate() {
            let py = ground_height_at(*sx, -58.0 + *sz);
            self.fb.draw_pyramid(cam, Vec3::new(*sx, py, -58.0 + *sz), 1.25, 8.5, 0.4, 0x1A202C, false);
            self.fb.draw_pyramid(
                cam,
                Vec3::new(*sx * 0.72, py + 1.1, -58.0 + *sz * 0.72),
                0.4,
                0.8,
                self.elapsed + idx as f32,
                0xF59E0B,
                true,
            );
        }
    }

    fn render_forest_and_crystals(&mut self, cam: &Camera) {
        // Deterministic Pine Trees
        for i in 0..56 {
            let ang = (i as f32) * 2.39996;
            let rad = 22.0 + ((i * 73) % 58) as f32;
            let tx = cos_approx(ang) * rad;
            let tz = sin_approx(ang) * rad;
            if sqrt_f32(tx * tx + (tz + 58.0) * (tz + 58.0)) < 14.0 {
                continue;
            }
            if abs_f32(tx) < 5.5 && tz < -10.0 && tz > -55.0 {
                continue;
            }
            let ty = ground_height_at(tx, tz);
            self.fb.draw_box(cam, Vec3::new(tx, ty + 1.0, tz), Vec3::new(0.24, 1.0, 0.24), 0.0, 0x5C4028, false);
            self.fb.draw_pyramid(cam, Vec3::new(tx, ty + 1.7, tz), 1.55, 3.0, ang, 0x1E5631, false);
            self.fb.draw_pyramid(cam, Vec3::new(tx, ty + 3.4, tz), 1.1, 2.4, ang + 0.5, 0x266B3E, false);
        }

        // Harvestable Aether Crystals
        for c in self.crystals.values() {
            self.fb.draw_box(cam, Vec3::new(c.x, c.y + 0.2, c.z), Vec3::new(0.75, 0.2, 0.75), 0.3, 0x334155, false);
            if c.available {
                self.fb.draw_pyramid(cam, Vec3::new(c.x, c.y + 0.35, c.z), 0.36, 1.5, self.elapsed * 0.4, 0x22D3EE, true);
                self.fb.draw_pyramid(cam, Vec3::new(c.x + 0.35, c.y + 0.3, c.z + 0.2), 0.22, 0.95, 0.3, 0x06B6D4, true);
                self.fb.draw_pyramid(cam, Vec3::new(c.x - 0.32, c.y + 0.3, c.z - 0.22), 0.22, 0.9, -0.4, 0x06B6D4, true);
            }
        }
    }

    fn render_entities(&mut self, cam: &Camera) {
        // Players (Composite 3D Rigs)
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, py, pz, yaw, moving) = if is_self && self.joined {
                (self.local_x, self.local_y, self.local_z, self.local_yaw, self.local_moving)
            } else {
                (p.x, p.y, p.z, p.yaw, p.moving)
            };

            let primary = p.class_id.primary_rgb();
            let accent = p.class_id.accent_rgb();
            let walk = if moving { sin_approx(self.elapsed * 10.0) * 0.22 } else { 0.0 };

            // Selection ring on ground
            self.fb.draw_ring_3d(cam, Vec3::new(px, py + 0.06, pz), 0.72, 0.88, accent);

            // Legs
            let lx = Vec3::new(-0.18, 0.42, walk).rotate_y(yaw).add(Vec3::new(px, py, pz));
            let rx = Vec3::new(0.18, 0.42, -walk).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, lx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);
            self.fb.draw_box(cam, rx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);

            // Torso & Pauldrons & Cape
            self.fb.draw_box(cam, Vec3::new(px, py + 1.2, pz), Vec3::new(0.32, 0.38, 0.22), yaw, primary, false);
            let cape_pos = Vec3::new(0.0, 1.1, -0.26).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, cape_pos, Vec3::new(0.28, 0.44, 0.04), yaw, 0x1E293B, false);

            // Head + Class Gear
            self.fb.draw_box(cam, Vec3::new(px, py + 1.82, pz), Vec3::new(0.2, 0.2, 0.2), yaw, 0xE6C3A5, false);
            if p.class_id == ClassId::Arcanist {
                self.fb.draw_pyramid(cam, Vec3::new(px, py + 1.98, pz), 0.28, 0.48, yaw, primary, false);
            } else {
                self.fb.draw_box(cam, Vec3::new(px, py + 1.95, pz), Vec3::new(0.22, 0.1, 0.22), yaw, accent, false);
            }

            // Weapon in Right Hand
            let weap = Vec3::new(0.42, 1.28, 0.28).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, weap, Vec3::new(0.07, 0.55, 0.07), yaw, accent, true);
        }

        // Mobs (Composite 3D Rigs)
        for (id, m) in &self.mobs {
            if !m.alive {
                continue;
            }
            if self.target_mob_id.as_ref() == Some(id) {
                let r = if m.kind == MobKind::Colossus { 2.3 } else { 1.1 };
                self.fb.draw_ring_3d(cam, Vec3::new(m.x, m.y + 0.07, m.z), r, r + 0.18, 0xEF4444);
            }
            match m.kind {
                MobKind::Stalker => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 0.7, m.z), Vec3::new(0.42, 0.28, 0.68), m.yaw, 0x3B2154, false);
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 0.98, m.z), 0.26, 0.45, m.yaw, 0xC084FC, true);
                }
                MobKind::Wisp => {
                    let bob = sin_approx(self.elapsed * 3.2) * 0.22;
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 1.2 + bob, m.z), 0.45, 0.9, self.elapsed * 1.5, 0x38BDF8, true);
                    self.fb.draw_box(cam, Vec3::new(m.x + cos_approx(self.elapsed * 2.0) * 0.75, m.y + 1.45, m.z + sin_approx(self.elapsed * 2.0) * 0.75), Vec3::new(0.14, 0.24, 0.14), self.elapsed, 0x1E293B, false);
                }
                MobKind::Sentinel => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 1.35, m.z), Vec3::new(0.55, 0.75, 0.36), m.yaw, 0x1E2430, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.3, m.z), Vec3::new(0.28, 0.28, 0.28), m.yaw, 0x9A6B32, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 1.5, m.z), Vec3::new(0.18, 0.18, 0.42), m.yaw, 0xF97316, true);
                }
                MobKind::Colossus => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.1, m.z), Vec3::new(1.05, 1.35, 0.72), m.yaw, 0x181E29, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 3.8, m.z), Vec3::new(0.45, 0.45, 0.45), m.yaw, 0xD97706, false);
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 4.35, m.z), 0.55, 0.85, self.elapsed, 0xFBBF24, true);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.3, m.z), Vec3::new(0.38, 0.38, 0.78), m.yaw, 0xF59E0B, true);
                }
            }
        }
    }

    fn render_vfx(&mut self, cam: &Camera) {
        for r in &self.rings {
            let t = r.age / r.life;
            let rad = 0.5 + t * r.max_r;
            self.fb.draw_ring_3d(cam, Vec3::new(r.x, r.y, r.z), rad, rad + 0.25, r.color);
        }
    }

    fn render_hud(&mut self, cam: &Camera) {
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        // Top-Right LangButton from aethravox/Lang ready_slot!
        let slot = self.lang.host.btn_bounds;
        self.fb.fill_rect_alpha(slot.x, slot.y, slot.width as i32, slot.height as i32, 0x0B121E, 230);
        self.fb.stroke_rect(slot.x, slot.y, slot.width as i32, slot.height as i32, 0x38BDF8);
        let lbl = self.lang.host.btn_label;
        let lw = Framebuffer::text_width(lbl, 2);
        self.fb.draw_text(slot.x + (slot.width as i32 - lw) / 2, slot.y + 8, lbl, 2, 0xF8FAFC);

        // Touch / Pad HUD toggle button
        let pad_lbl = self.lang.tr("TACTIL");
        let pad_x = w - 154;
        self.fb.fill_rect_alpha(pad_x, 16, 62, 30, if self.touch_mode { 0x0F766E } else { 0x0B121E }, 225);
        self.fb.stroke_rect(pad_x, 16, 62, 30, if self.touch_mode { 0x2DD4BF } else { 0x475569 });
        let pw = Framebuffer::text_width(pad_lbl, 1);
        self.fb.draw_text(pad_x + (62 - pw) / 2, 27, pad_lbl, 1, 0xF8FAFC);

        if !self.joined {
            // Top Title + Real Players Online
            let title = self.lang.tr("WAYFARERS ONLINE (WFO)");
            self.fb.fill_rect_alpha(16, 16, Framebuffer::text_width(title, 2) + 28, 34, 0x0B121E, 215);
            self.fb.draw_text(30, 26, title, 2, 0xF8FAFC);

            let online_str = format!("{} {}", self.players.len(), self.lang.tr("EN LINEA"));
            let ow = Framebuffer::text_width(&online_str, 1) + 20;
            self.fb.fill_rect_alpha(pad_x - ow - 8, 16, ow, 30, 0x0B121E, 215);
            self.fb.draw_text(pad_x - ow + 2, 27, &online_str, 1, 0x34D399);

            // Bottom-Center Lobby Card
            let panel_w = 540.min(w - 32);
            let panel_x = (w - panel_w) / 2;
            let panel_y = h - 245;
            self.fb.fill_rect_alpha(panel_x, panel_y, panel_w, 224, 0x0B121E, 232);
            self.fb.stroke_rect(panel_x, panel_y, panel_w, 224, 0x334155);

            let name_lbl = self.lang.tr("NOMBRE");
            self.fb.draw_text(panel_x + 16, panel_y + 16, name_lbl, 1, 0x94A3B8);

            // Hero Name Input
            self.fb.fill_rect(panel_x + 16, panel_y + 34, panel_w - 186, 32, 0x111827);
            self.fb.stroke_rect(
                panel_x + 16,
                panel_y + 34,
                panel_w - 186,
                32,
                if self.lobby_focus_name { 0x38BDF8 } else { 0x475569 },
            );
            let disp_name = if self.lobby_focus_name && ((self.elapsed * 2.0) as i32 % 2 == 0) {
                format!("{}_", self.lobby_name)
            } else {
                self.lobby_name.clone()
            };
            self.fb.draw_text(panel_x + 24, panel_y + 43, &disp_name, 2, 0xFFFFFF);

            // Enter World CTA
            let enter_lbl = if self.connecting { "..." } else { self.lang.tr("ENTRAR AL MUNDO") };
            self.fb.fill_rect(panel_x + panel_w - 160, panel_y + 34, 144, 32, 0xF59E0B);
            let ew = Framebuffer::text_width(enter_lbl, 1);
            self.fb.draw_text(panel_x + panel_w - 160 + (144 - ew) / 2, panel_y + 46, enter_lbl, 1, 0x090D16);

            // 3 Class Cards
            let card_w = (panel_w - 48) / 3;
            let card_y = panel_y + 98;
            let classes = [
                (ClassId::Vanguard, self.lang.tr("VANGUARDIA"), "HP 160 / DMG 24"),
                (ClassId::Arcanist, self.lang.tr("ARCANISTA"), "HP 110 / MP 140"),
                (ClassId::Ranger, self.lang.tr("MONTARAZ"), "HP 130 / SPD 10.4"),
            ];
            for (idx, (cid, cname, stat)) in classes.iter().enumerate() {
                let cx = panel_x + 16 + (idx as i32) * (card_w + 8);
                let active = self.lobby_class == *cid;
                self.fb.fill_rect(cx, card_y, card_w, 72, if active { 0x1E293B } else { 0x0F172A });
                self.fb.stroke_rect(cx, card_y, card_w, 72, if active { cid.accent_rgb() } else { 0x334155 });
                self.fb.draw_text(cx + 10, card_y + 16, cname, 1, cid.accent_rgb());
                self.fb.draw_text(cx + 10, card_y + 40, stat, 1, 0xCBD5E1);
            }

            self.fb.draw_text(panel_x + 16, panel_y + 190, "WASD · SPACE · 1-3 · E · ENTER CHAT", 1, 0x94A3B8);
            return;
        }

        // 3D Projected Nameplates & Damage Popups
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, py, pz) = if is_self {
                (self.local_x, self.local_y, self.local_z)
            } else {
                (p.x, p.y, p.z)
            };
            if let Some((sx, sy, _)) = cam.project_point(Vec3::new(px, py + 2.45, pz), self.fb.width, self.fb.height) {
                let ix = sx as i32;
                let iy = sy as i32;
                let tag = format!("{} LV.{}", p.name, p.level);
                let tw = Framebuffer::text_width(&tag, 1);
                self.fb.fill_rect_alpha(ix - tw / 2 - 4, iy - 16, tw + 8, 12, 0x0B121E, 190);
                self.fb.draw_text(ix - tw / 2, iy - 13, &tag, 1, if is_self { 0xFFFFFF } else { 0x34D399 });
                let hp_w = ((p.hp.max(0) as f32 / p.max_hp.max(1) as f32) * 44.0) as i32;
                self.fb.fill_rect(ix - 22, iy - 2, 44, 3, 0x1E293B);
                self.fb.fill_rect(ix - 22, iy - 2, hp_w, 3, 0x10B981);
            }
        }

        for m in self.mobs.values() {
            if !m.alive {
                continue;
            }
            let d = sqrt_f32((m.x - self.local_x) * (m.x - self.local_x) + (m.z - self.local_z) * (m.z - self.local_z));
            if d > 35.0 && self.target_mob_id.as_ref() != Some(&m.id) {
                continue;
            }
            let hy = if m.kind == MobKind::Colossus { 5.2 } else { 2.3 };
            if let Some((sx, sy, _)) = cam.project_point(Vec3::new(m.x, m.y + hy, m.z), self.fb.width, self.fb.height) {
                let ix = sx as i32;
                let iy = sy as i32;
                let tag = format!("{} LV.{}", m.name, m.level);
                let tw = Framebuffer::text_width(&tag, 1);
                self.fb.fill_rect_alpha(ix - tw / 2 - 4, iy - 16, tw + 8, 12, 0x0B121E, 195);
                let tc = if m.kind == MobKind::Colossus { 0xFBBF24 } else { 0xFCA5A5 };
                self.fb.draw_text(ix - tw / 2, iy - 13, &tag, 1, tc);
                let hp_w = ((m.hp.max(0) as f32 / m.max_hp.max(1) as f32) * 48.0) as i32;
                self.fb.fill_rect(ix - 24, iy - 2, 48, 3, 0x1E293B);
                self.fb.fill_rect(ix - 24, iy - 2, hp_w, 3, 0xEF4444);
            }
        }

        for pop in &self.popups {
            if let Some((sx, sy, _)) = cam.project_point(Vec3::new(pop.x, pop.y, pop.z), self.fb.width, self.fb.height) {
                let tw = Framebuffer::text_width(&pop.text, 2);
                self.fb.draw_text(sx as i32 - tw / 2 + 1, sy as i32 + 1, &pop.text, 2, 0x000000);
                self.fb.draw_text(sx as i32 - tw / 2, sy as i32, &pop.text, 2, pop.color);
            }
        }

        // Top-Left Self Unit Frame
        if let Some(self_p) = self.self_id.as_ref().and_then(|id| self.players.get(id)) {
            self.fb.fill_rect_alpha(16, 16, 250, 96, 0x0B121E, 225);
            self.fb.stroke_rect(16, 16, 250, 96, 0x334155);

            let cname = match self_p.class_id {
                ClassId::Vanguard => self.lang.tr("VANGUARDIA"),
                ClassId::Arcanist => self.lang.tr("ARCANISTA"),
                ClassId::Ranger => self.lang.tr("MONTARAZ"),
            };
            let zname = match zone_at(self.local_x, self.local_z) {
                ZoneId::Plaza => self.lang.tr("PLAZA SANTUARIO"),
                ZoneId::Woods => self.lang.tr("BOSQUE ESMERALDA"),
                ZoneId::Citadel => self.lang.tr("CIUDADELA"),
                ZoneId::Veins => self.lang.tr("VETAS CRISTAL"),
            };

            self.fb.draw_text(26, 26, &format!("{} · LV.{}", self_p.name, self_p.level), 1, 0xFFFFFF);
            self.fb.draw_text(26, 38, &format!("{} · {}", cname, zname), 1, 0x94A3B8);

            // HP bar
            let hp_w = ((self_p.hp.max(0) as f32 / self_p.max_hp.max(1) as f32) * 228.0) as i32;
            self.fb.fill_rect(26, 54, 228, 12, 0x1E293B);
            self.fb.fill_rect(26, 54, hp_w, 12, 0x10B981);
            self.fb.draw_text(30, 56, &format!("HP {}/{}", self_p.hp, self_p.max_hp), 1, 0xFFFFFF);

            // MP bar
            let mp_w = ((self_p.mp.max(0) as f32 / self_p.max_mp.max(1) as f32) * 228.0) as i32;
            self.fb.fill_rect(26, 70, 228, 10, 0x1E293B);
            self.fb.fill_rect(26, 70, mp_w, 10, 0x38BDF8);
            self.fb.draw_text(30, 71, &format!("MP {}/{}", self_p.mp, self_p.max_mp), 1, 0xFFFFFF);

            // XP + Gold
            let xp_w = ((self_p.xp.max(0) as f32 / self_p.xp_next.max(1) as f32) * 228.0) as i32;
            self.fb.fill_rect(26, 86, 228, 4, 0x1E293B);
            self.fb.fill_rect(26, 86, xp_w, 4, 0xF59E0B);
            self.fb.draw_text(26, 95, &format!("{} {}", self_p.gold, self.lang.tr("ORO")), 1, 0xFBBF24);
        }

        // Target Mob Frame
        if let Some(tid) = &self.target_mob_id {
            if let Some(m) = self.mobs.get(tid) {
                if m.alive {
                    self.fb.fill_rect_alpha(16, 120, 250, 42, 0x0B121E, 225);
                    self.fb.stroke_rect(16, 120, 250, 42, 0xEF4444);
                    self.fb.draw_text(26, 128, &format!("{} · LV.{}", m.name, m.level), 1, 0xFCA5A5);
                    let mhp_w = ((m.hp.max(0) as f32 / m.max_hp.max(1) as f32) * 228.0) as i32;
                    self.fb.fill_rect(26, 143, 228, 8, 0x1E293B);
                    self.fb.fill_rect(26, 143, mhp_w, 8, 0xEF4444);
                }
            }
        }

        // Top-Right Mini-Radar
        let rx = w - 114;
        let ry = 56;
        self.fb.fill_rect_alpha(rx, ry, 98, 98, 0x0B121E, 225);
        self.fb.stroke_rect(rx, ry, 98, 98, 0x334155);
        let rcx = rx + 49;
        let rcy = ry + 49;
        let rscale = 44.0 / 90.0;
        for c in self.crystals.values() {
            if c.available {
                self.fb.fill_rect(rcx + (c.x * rscale) as i32 - 1, rcy + (c.z * rscale) as i32 - 1, 3, 3, 0x22D3EE);
            }
        }
        for m in self.mobs.values() {
            if m.alive {
                let col = if m.kind == MobKind::Colossus { 0xF59E0B } else { 0xEF4444 };
                self.fb.fill_rect(rcx + (m.x * rscale) as i32 - 1, rcy + (m.z * rscale) as i32 - 1, 3, 3, col);
            }
        }
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, pz) = if is_self { (self.local_x, self.local_z) } else { (p.x, p.z) };
            self.fb.fill_rect(
                rcx + (px * rscale) as i32 - 2,
                rcy + (pz * rscale) as i32 - 2,
                4,
                4,
                if is_self { 0xFFFFFF } else { 0x10B981 },
            );
        }

        // Bottom-Left Live World Chat
        self.fb.fill_rect_alpha(16, h - 146, 274, 130, 0x0B121E, 215);
        self.fb.stroke_rect(16, h - 146, 274, 130, 0x334155);
        self.fb.draw_text(24, h - 138, self.lang.tr("CHAT"), 1, 0x38BDF8);
        let start_idx = self.chat.len().saturating_sub(5);
        for (row, msg) in self.chat[start_idx..].iter().enumerate() {
            let y_line = h - 122 + (row as i32) * 14;
            let line = format!("{}: {}", msg.sender_name, msg.text);
            let clipped: String = line.chars().take(38).collect();
            self.fb.draw_text(24, y_line, &clipped, 1, 0xE2E8F0);
        }
        self.fb.fill_rect(22, h - 44, 262, 22, 0x111827);
        self.fb.stroke_rect(22, h - 44, 262, 22, if self.chat_focused { 0x38BDF8 } else { 0x475569 });
        let chat_disp = if self.chat_focused {
            format!("{}_", self.chat_input)
        } else if self.chat_input.is_empty() {
            String::from("ENTER...")
        } else {
            self.chat_input.clone()
        };
        let chat_clipped: String = chat_disp.chars().take(36).collect();
        self.fb.draw_text(28, h - 37, &chat_clipped, 1, if self.chat_focused { 0xFFFFFF } else { 0x64748B });

        // Bottom-Center Action Bar (1, 2, 3, E)
        let bar_x = (w - 280) / 2;
        let bar_y = h - 68;
        self.fb.fill_rect_alpha(bar_x, bar_y, 280, 52, 0x0B121E, 225);
        self.fb.stroke_rect(bar_x, bar_y, 280, 52, 0x334155);
        let actions = [
            ("1", self.lang.tr("GOLPE"), self.cooldowns[0], 0xF59E0B),
            ("2", self.lang.tr("NOVA"), self.cooldowns[1], 0x38BDF8),
            ("3", self.lang.tr("CURAR"), self.cooldowns[2], 0x10B981),
            ("E", self.lang.tr("MINAR"), 0.0, 0x22D3EE),
        ];
        for (idx, (key, lbl, cd, col)) in actions.iter().enumerate() {
            let ax = bar_x + 6 + (idx as i32) * 68;
            let ay = bar_y + 6;
            self.fb.fill_rect(ax, ay, 62, 40, 0x111827);
            self.fb.stroke_rect(ax, ay, 62, 40, *col);
            self.fb.draw_text(ax + 5, ay + 5, key, 1, 0x94A3B8);
            self.fb.draw_text(ax + 5, ay + 24, lbl, 1, 0xF8FAFC);
            if *cd > 0.05 {
                self.fb.fill_rect_alpha(ax, ay, 62, 40, 0x020617, 180);
                self.fb.draw_text(ax + 18, ay + 15, &format!("{:.1}", cd), 1, 0xFFFFFF);
            }
        }

        // Virtual Touch Joystick + Jump Button (Mobile / Tablet / Touchscreens)
        if self.touch_mode {
            let jx = 68;
            let jy = h - 208;
            self.fb.fill_rect_alpha(jx - 46, jy - 46, 92, 92, 0x0B121E, 165);
            self.fb.stroke_rect(jx - 46, jy - 46, 92, 92, 0x38BDF8);
            let kx = jx + (self.input.joy_x * 30.0) as i32;
            let ky = jy + (self.input.joy_y * 30.0) as i32;
            self.fb.fill_rect(kx - 14, ky - 14, 28, 28, 0x38BDF8);

            // Jump button on right side
            let jbx = w - 88;
            let jby = h - 134;
            self.fb.fill_rect_alpha(jbx, jby, 72, 46, 0x0B121E, 210);
            self.fb.stroke_rect(jbx, jby, 72, 46, 0xF59E0B);
            let jlbl = self.lang.tr("SALTAR");
            let jw = Framebuffer::text_width(jlbl, 1);
            self.fb.draw_text(jbx + (72 - jw) / 2, jby + 19, jlbl, 1, 0xF8FAFC);
        }
    }
}

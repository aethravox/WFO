use super::*;

impl MmoClientEngine {
    pub(crate) fn render_hud(&mut self, cam: &Camera) {
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        // Top-Right LangButton from aethravox/Lang ready_slot!
        let slot = self.lang.host.btn_bounds;
        self.fb.rounded_panel(slot.x, slot.y, slot.width as i32, slot.height as i32, 0x0B121E, 238);
        self.fb.panel_border(slot.x, slot.y, slot.width as i32, slot.height as i32, 0x38BDF8);
        let lbl = self.lang.host.btn_label;
        let lw = Framebuffer::text_width(lbl, 2);
        self.fb.draw_text_shadow(slot.x + (slot.width as i32 - lw) / 2, slot.y + 8, lbl, 2, 0xF8FAFC);

        // Touch / Pad HUD toggle button
        let pad_lbl = self.lang.tr("TACTIL");
        let pad_x = w - 154;
        self.fb.fill_rect_alpha(pad_x, 16, 62, 30, if self.touch_mode { 0x0F766E } else { 0x0B121E }, 225);
        self.fb.stroke_rect(pad_x, 16, 62, 30, if self.touch_mode { 0x2DD4BF } else { 0x475569 });
        let pw = Framebuffer::text_width(pad_lbl, 1);
        self.fb.draw_text(pad_x + (62 - pw) / 2, 27, pad_lbl, 1, 0xF8FAFC);

        let lan_x = w - 224;
        self.fb.fill_rect_alpha(lan_x, 16, 62, 30, if self.lan_mode { 0x1E3A8A } else { 0x0B121E }, 225);
        self.fb.stroke_rect(lan_x, 16, 62, 30, if self.lan_mode { 0x60A5FA } else { 0x475569 });
        let lan_lbl = "LAN";
        let lw2 = Framebuffer::text_width(lan_lbl, 1);
        self.fb.draw_text(lan_x + (62 - lw2) / 2, 27, lan_lbl, 1, 0xF8FAFC);

        if self.lan_mode {
            let panel_w = if w < 680 { w - 32 } else { 220 };
            let panel_x = if w < 680 { 16 } else { w - 236 };
            let panel_y = 56;
            self.fb.fill_rect_alpha(panel_x, panel_y, panel_w, 96, 0x0B121E, 235);
            self.fb.stroke_rect(panel_x, panel_y, panel_w, 96, 0x60A5FA);
            self.fb.draw_text(panel_x + 10, panel_y + 10, "JUGADORES LAN", 1, 0x60A5FA);
            if self.lan_peers.is_empty() {
                self.fb.draw_text(panel_x + 10, panel_y + 34, "BUSCANDO...", 1, 0x94A3B8);
            } else {
                for (idx, peer) in self.lan_peers.iter().take(4).enumerate() {
                    let y = panel_y + 30 + (idx as i32) * 14;
                    let name: String = peer.name.chars().take(16).collect();
                    self.fb.draw_text(panel_x + 10, y, &name, 1, 0xF8FAFC);
                }
            }
        }

        if !self.joined {
            // In-world private lodge HUD: no menu card over the room.
            let title = self.lang.tr("CASA / MODO LOCAL");
            self.fb.rounded_panel(16, 16, Framebuffer::text_width(title, 2) + 28, 34, 0x0B121E, 225);
            self.fb.panel_border(16, 16, Framebuffer::text_width(title, 2) + 28, 34, 0x38BDF8);
            self.fb.draw_text_shadow(30, 26, title, 2, 0xF8FAFC);

            let cname = match self.lobby_class {
                ClassId::Vanguard => self.lang.tr("VANGUARDIA"),
                ClassId::Arcanist => self.lang.tr("ARCANISTA"),
                ClassId::Ranger => self.lang.tr("MONTARAZ"),
            };
            self.fb.fill_rect_alpha(16, 58, 222, 30, 0x0B121E, 220);
            self.fb.draw_text(26, 69, &format!("{} · {}", self.lobby_name, cname), 1, 0xF8FAFC);
            if self.lobby_server_ready {
                let room = format!("INSTANCIA {}", self.lobby_room_id);
                self.fb.fill_rect_alpha(16, 96, 222, 24, 0x0B121E, 210);
                self.fb.draw_text(26, 103, &room, 1, 0x34D399);
            }

            let invite_w = if w < 680 { w - 32 } else { 270 };
            let invite_x = if w < 680 { 16 } else { w - 286 };
            let invite_y = if w < 680 { h - 98 } else { 128 };
            self.fb.fill_rect_alpha(invite_x, invite_y, invite_w, 38, 0x0B121E, 230);
            self.fb.stroke_rect(invite_x, invite_y, invite_w, 38, if self.lobby_invite_focus { 0x38BDF8 } else { 0x475569 });
            let input_w = invite_w - 74;
            let disp = if self.lobby_invite_focus && ((self.elapsed * 2.0) as i32 % 2 == 0) {
                format!("{}_", self.lobby_invite_input)
            } else if self.lobby_invite_input.is_empty() {
                String::from("CODIGO DE CASA")
            } else {
                self.lobby_invite_input.clone()
            };
            let disp_clipped: String = disp.chars().take(if w < 680 { 24 } else { 18 }).collect();
            self.fb.draw_text(invite_x + 10, invite_y + 15, &disp_clipped, 1, 0xF8FAFC);
            self.fb.fill_rect(invite_x + input_w, invite_y + 5, 58, 28, 0xF59E0B);
            let join = "UNIR";
            let jw = Framebuffer::text_width(join, 1);
            self.fb.draw_text(invite_x + input_w + (58 - jw) / 2, invite_y + 14, join, 1, 0x090D16);

            let prompt = if sqrt_f32(self.local_x * self.local_x + (self.local_z + 9.35) * (self.local_z + 9.35)) < 4.5 {
                "E  CRUZAR PORTAL AL MUNDO"
            } else if sqrt_f32((self.local_x + 9.0) * (self.local_x + 9.0) + (self.local_z - 4.0) * (self.local_z - 4.0)) < 3.2 {
                "E  SOLICITAR CLASE ALEATORIA"
            } else if self.local_x > 5.5 && self.local_z > 1.0 {
                "ALMACEN  ·  OFFLINE: SOLO TUTORIAL"
            } else if self.local_x > 5.0 && self.local_z < -1.5 {
                "ARENA DE DUELOS  ·  SIN PERDIDA DE OBJETOS"
            } else {
                "WASD / JOYSTICK  ·  E PARA INTERACTUAR"
            };
            let pw = Framebuffer::text_width(prompt, 1) + 24;
            let px = ((w - pw) / 2).max(8);
            self.fb.fill_rect_alpha(px, h - 52, pw, 36, 0x0B121E, 230);
            self.fb.stroke_rect(px, h - 52, pw, 36, 0x38BDF8);
            self.fb.draw_text(px + 12, h - 40, prompt, 1, 0xF8FAFC);
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

        if let Some(duel) = &self.duel {
            let panel_w = if w < 680 { w - 32 } else { 360 };
            let panel_x = (w - panel_w) / 2;
            let panel_y = 16;
            self.fb.fill_rect_alpha(panel_x, panel_y, panel_w, 58, 0x0B121E, 235);
            self.fb.stroke_rect(panel_x, panel_y, panel_w, 58, 0xF59E0B);
            let title = format!("DUELLO {} · R{} · {} / {}", duel.opponent_name, duel.round, duel.wins, duel.opponent_wins);
            let title: String = title.chars().take(if w < 680 { 38 } else { 48 }).collect();
            self.fb.draw_text(panel_x + 10, panel_y + 10, &title, 1, 0xFBBF24);
            let outcome = if duel.round_winner.is_empty() {
                String::from("ELIGE: 1 GOLPE · 2 NOVA · 3 CURAR")
            } else {
                format!("RONDA: {}", duel.round_winner)
            };
            self.fb.draw_text(panel_x + 10, panel_y + 34, &outcome, 1, 0xF8FAFC);
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
        let compact_hud = w < 680 || self.touch_mode;
        if !compact_hud {
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
        }

        // Bottom-Center Action Bar (1, 2, 3, E)
        let compact_actions = w < 680;
        let bar_w = if compact_actions { 208 } else { 280 };
        let bar_x = (w - bar_w) / 2;
        let bar_y = h - 68;
        self.fb.fill_rect_alpha(bar_x, bar_y, bar_w, 52, 0x0B121E, 225);
        self.fb.stroke_rect(bar_x, bar_y, bar_w, 52, 0x334155);
        let actions = [
            ("1", self.lang.tr("GOLPE"), self.cooldowns[0], 0xF59E0B),
            ("2", self.lang.tr("NOVA"), self.cooldowns[1], 0x38BDF8),
            ("3", self.lang.tr("CURAR"), self.cooldowns[2], 0x10B981),
            ("E", self.lang.tr("MINAR"), 0.0, 0x22D3EE),
        ];
        for (idx, (key, lbl, cd, col)) in actions.iter().enumerate() {
            let ax = bar_x + 6 + (idx as i32) * 68;
            let ax = if compact_actions {
                bar_x + 6 + (idx as i32) * 50
            } else {
                ax
            };
            let ay = bar_y + 6;
            let slot_w = if compact_actions { 44 } else { 62 };
            self.fb.fill_rect(ax, ay, slot_w, 40, 0x111827);
            self.fb.stroke_rect(ax, ay, slot_w, 40, *col);
            self.fb.draw_text(ax + 5, ay + 5, key, 1, 0x94A3B8);
            if !compact_actions {
                self.fb.draw_text(ax + 5, ay + 24, lbl, 1, 0xF8FAFC);
            }
            if *cd > 0.05 {
                self.fb.fill_rect_alpha(ax, ay, slot_w, 40, 0x020617, 180);
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

            // Compact chat button that stays clear of movement and jump controls.
            let cbx = w - 88;
            let cby = h - 188;
            self.fb.fill_rect_alpha(cbx, cby, 72, 46, 0x0B121E, 210);
            self.fb.stroke_rect(cbx, cby, 72, 46, if self.chat_focused { 0x38BDF8 } else { 0x475569 });
            let clbl = self.lang.tr("CHAT");
            let cw = Framebuffer::text_width(clbl, 1);
            self.fb.draw_text(cbx + (72 - cw) / 2, cby + 19, clbl, 1, 0xF8FAFC);
        }
    }
}

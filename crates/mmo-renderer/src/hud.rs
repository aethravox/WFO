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

        if !self.joined {
            // Top Title + Real Players Online
            let title = self.lang.tr("WAYFARERS ONLINE (WFO)");
            self.fb.rounded_panel(16, 16, Framebuffer::text_width(title, 2) + 28, 34, 0x0B121E, 225);
            self.fb.panel_border(16, 16, Framebuffer::text_width(title, 2) + 28, 34, 0x38BDF8);
            self.fb.draw_text_shadow(30, 26, title, 2, 0xF8FAFC);

            let online_str = format!("{} {}", self.players.len(), self.lang.tr("EN LINEA"));
            let ow = Framebuffer::text_width(&online_str, 1) + 20;
            self.fb.fill_rect_alpha(pad_x - ow - 8, 16, ow, 30, 0x0B121E, 215);
            self.fb.draw_text(pad_x - ow + 2, 27, &online_str, 1, 0x34D399);

            // Bottom-Center Lobby Card
            let panel_w = 540.min(w - 32);
            let panel_x = (w - panel_w) / 2;
            let panel_y = h - 245;
            self.fb.rounded_panel(panel_x, panel_y, panel_w, 224, 0x0B121E, 242);
            self.fb.panel_border(panel_x, panel_y, panel_w, 224, 0x38BDF8);
            self.fb.fill_rect(panel_x + 16, panel_y + 6, 92, 2, 0x38BDF8);

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

use super::*;
use crate::lan::LanOutbound;

pub(crate) struct PlayerApp {
    window: Option<Arc<Window>>,
    gpu: Option<GpuSurface>,
    engine: MmoClientEngine,
    rx_in: Receiver<ServerPacket>,
    tx_out: Sender<String>,
    rx_lan: Receiver<LanPacket>,
    tx_lan: Option<Sender<LanOutbound>>,
    profile: LocalProfile,
    last_frame: Instant,
    cursor_x: i32,
    cursor_y: i32,
}

impl PlayerApp {
    pub(crate) fn new(
        rx_in: Receiver<ServerPacket>,
        tx_out: Sender<String>,
        rx_lan: Receiver<LanPacket>,
        tx_lan: Option<Sender<LanOutbound>>,
        profile: LocalProfile,
    ) -> Self {
        #[cfg(target_os = "android")]
        let mut engine = MmoClientEngine::new(960, 600);
        #[cfg(target_os = "android")]
        {
            engine.touch_mode = true;
        }
        #[cfg(not(target_os = "android"))]
        let mut engine = MmoClientEngine::new(960, 600);
        engine.auth_username = profile.username.clone();
        engine.lobby_name = profile.username.clone();
        engine.lobby_class = profile.class_id;
        engine.offline_level = profile.level;
        engine.offline_xp = profile.xp;
        engine.offline_xp_next = profile.xp_next;
        engine.offline_gold = profile.gold;
        engine.offline_rank_points = profile.rank_points;
        Self {
            window: None,
            gpu: None,
            engine,
            rx_in,
            tx_out,
            rx_lan,
            tx_lan,
            profile,
            last_frame: Instant::now(),
            cursor_x: 480,
            cursor_y: 300,
        }
    }

    fn is_typing(&self) -> bool {
        (!self.engine.joined
            && (self.engine.auth_panel_open
                || self.engine.lobby_focus_name
                || self.engine.lobby_invite_focus))
            || (self.engine.joined && self.engine.chat_focused)
    }

    fn handle_key(&mut self, event: KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let typing = self.is_typing();

        if let PhysicalKey::Code(code) = event.physical_key {
            match code {
                KeyCode::KeyW | KeyCode::ArrowUp => {
                    self.engine.input.w = pressed && !typing;
                }
                KeyCode::KeyS | KeyCode::ArrowDown => {
                    self.engine.input.s = pressed && !typing;
                }
                KeyCode::KeyA | KeyCode::ArrowLeft => {
                    self.engine.input.a = pressed && !typing;
                }
                KeyCode::KeyD | KeyCode::ArrowRight => {
                    self.engine.input.d = pressed && !typing;
                }
                KeyCode::Space if pressed && !typing => {
                    self.engine.trigger_jump();
                    return;
                }
                KeyCode::Digit1 if pressed && !typing => {
                    self.engine.trigger_skill(0);
                    return;
                }
                KeyCode::Digit2 if pressed && !typing => {
                    self.engine.trigger_skill(1);
                    return;
                }
                KeyCode::Digit3 if pressed && !typing => {
                    self.engine.trigger_skill(2);
                    return;
                }
                KeyCode::KeyE if pressed && !typing => {
                    self.engine.trigger_gather();
                    return;
                }
                KeyCode::ShiftLeft | KeyCode::ShiftRight if pressed && !typing => {
                    self.engine.trigger_dodge();
                    return;
                }
                KeyCode::KeyM if pressed && !typing => {
                    self.engine.toggle_personal_marker();
                    return;
                }
                KeyCode::KeyB if pressed && !typing => {
                    self.engine.toggle_bestiary();
                    return;
                }
                KeyCode::Tab if pressed && !self.engine.joined && self.engine.auth_panel_open => {
                    self.engine.auth_password_focus = !self.engine.auth_password_focus;
                    return;
                }
                KeyCode::Enter if pressed => {
                    self.engine.on_enter();
                    return;
                }
                KeyCode::Backspace if pressed => {
                    self.engine.on_backspace();
                    return;
                }
                KeyCode::Escape if pressed => {
                    self.engine.chat_focused = false;
                    self.engine.lobby_focus_name = false;
                    if self.engine.auth_panel_open && !self.engine.account_authenticated {
                        self.engine.enter_offline_mode();
                    }
                    return;
                }
                _ => {}
            }
        }

        if pressed && typing {
            if let Some(txt) = &event.text {
                for ch in txt.chars() {
                    self.engine.on_char_input(ch);
                }
            }
        }
    }

    fn render_frame(&mut self) {
        while let Ok(pkt) = self.rx_in.try_recv() {
            let restore_local_profile =
                matches!(&pkt, ServerPacket::ConnectionStatus { connected: false })
                    && self.engine.account_authenticated;
            self.engine.apply_server_packet(pkt);
            if restore_local_profile {
                self.engine.auth_username = self.profile.username.clone();
                self.engine.lobby_name = self.profile.username.clone();
                self.engine.lobby_class = self.profile.class_id;
                self.engine.offline_level = self.profile.level;
                self.engine.offline_xp = self.profile.xp;
                self.engine.offline_xp_next = self.profile.xp_next;
                self.engine.offline_gold = self.profile.gold;
                self.engine.offline_rank_points = self.profile.rank_points;
            }
        }
        if self.engine.offline_mode && !self.engine.account_authenticated {
            self.profile.save(
                &self.engine.auth_username,
                self.engine.lobby_class,
                self.engine.offline_level,
                self.engine.offline_xp,
                self.engine.offline_xp_next,
                self.engine.offline_gold,
                self.engine.offline_rank_points,
            );
        }
        while let Ok(pkt) = self.rx_lan.try_recv() {
            self.engine.apply_lan_packet(pkt);
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        self.engine.tick(dt);
        for pkt in self.engine.outbox.drain(..) {
            if self.engine.account_authenticated
                || matches!(
                    &pkt,
                    mmo_protocol::ClientPacket::RegisterAccount { .. }
                        | mmo_protocol::ClientPacket::LoginAccount { .. }
                )
            {
                let _ = self.tx_out.send(pkt.encode());
            }
        }
        for pkt in self.engine.lan_outbox.drain(..) {
            let peer = self
                .engine
                .lan_duel_opponent
                .as_ref()
                .and_then(|id| self.engine.lan_peers.iter().find(|p| p.id == *id));
            if let Some(peer) = peer {
                if let Some(tx) = &self.tx_lan {
                    let _ = tx.send(LanOutbound {
                        packet: pkt,
                        address: peer.address.clone(),
                        port: peer.udp_port,
                    });
                }
            }
        }
        self.engine.render_gpu();

        if let Some(gpu) = self.gpu.as_mut() {
            gpu.present(
                &self.engine.fb.scene_vertices,
                &self.engine.fb.gpu_background_pixels,
                &self.engine.fb.ui_pixels,
                self.engine.fb.width,
                self.engine.fb.height,
            );
        }
    }
}

impl ApplicationHandler for PlayerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let win = if let Some(existing) = &self.window {
            existing.clone()
        } else {
            let attrs = Window::default_attributes()
                .with_title("Wayfarers Online (WFO) — Native Rust MMO")
                .with_inner_size(LogicalSize::new(960.0, 600.0));
            let created = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("create native MMO window"),
            );
            #[cfg(target_os = "android")]
            created.set_ime_allowed(true);
            self.window = Some(created.clone());
            created
        };
        let size = win.inner_size();
        let width = size.width.max(320);
        let height = size.height.max(240);
        self.engine.resize(width as usize, height as usize);
        if self.gpu.is_none() {
            self.gpu = Some(GpuSurface::new(win));
        } else if let Some(gpu) = self.gpu.as_mut() {
            gpu.resize(width, height);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        {
            self.gpu = None;
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.engine
                    .resize(size.width.max(320) as usize, size.height.max(240) as usize);
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width.max(320), size.height.max(240));
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_x = position.x as i32;
                self.cursor_y = position.y as i32;
                self.engine.on_pointer_move(self.cursor_x, self.cursor_y);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        self.engine.on_pointer_down(self.cursor_x, self.cursor_y);
                    }
                    ElementState::Released => {
                        self.engine.on_pointer_up(self.cursor_x, self.cursor_y);
                    }
                }
                #[cfg(target_os = "android")]
                if let Some(win) = &self.window {
                    win.set_ime_allowed(self.is_typing());
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 40.0,
                };
                self.engine.on_wheel(dy);
            }
            WindowEvent::Touch(touch) => {
                let tx = touch.location.x as i32;
                let ty = touch.location.y as i32;
                match touch.phase {
                    TouchPhase::Started => self.engine.on_touch_start(touch.id, tx, ty),
                    TouchPhase::Moved => self.engine.on_touch_move(touch.id, tx, ty),
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.engine.on_touch_end(touch.id, tx, ty);
                    }
                }
                #[cfg(target_os = "android")]
                if let Some(win) = &self.window {
                    win.set_ime_allowed(self.is_typing());
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                for ch in text.chars() {
                    self.engine.on_char_input(ch);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(event);
                #[cfg(target_os = "android")]
                if let Some(win) = &self.window {
                    win.set_ime_allowed(self.is_typing());
                }
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(win) = &self.window {
            win.request_redraw();
        }
        thread::sleep(Duration::from_millis(8));
    }
}

use mmo_protocol::{LanPacket, ServerPacket};
use mmo_renderer::MmoClientEngine;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod gpu;
use gpu::GpuSurface;

const LAN_PORT: u16 = 7889;

struct LanOutbound {
    packet: LanPacket,
    address: String,
    port: u16,
}

fn spawn_lan_service(tx_in: Sender<LanPacket>) -> Option<Sender<LanOutbound>> {
    let listener = UdpSocket::bind(("0.0.0.0", LAN_PORT)).ok()?;
    listener.set_broadcast(true).ok();
    listener.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let broadcaster = listener.try_clone().expect("clone LAN duel socket");
    let outbound = listener.try_clone().expect("clone LAN outbound socket");

    thread::spawn(move || {
        let peer_id = format!("lan_{}", std::process::id());
        let name = std::env::var("WFO_LAN_NAME").unwrap_or_else(|_| String::from("Wayfarer LAN"));
        loop {
            let packet = LanPacket::Announce {
                peer_id: peer_id.clone(),
                name: name.clone(),
                udp_port: LAN_PORT,
                address: String::from("255.255.255.255"),
            };
            let _ = broadcaster.send_to(packet.encode().as_bytes(), ("255.255.255.255", LAN_PORT));
            thread::sleep(Duration::from_secs(2));
        }
    });

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match listener.recv_from(&mut buf) {
                Ok((len, source)) => {
                    if let Some(mut packet) = LanPacket::decode(&String::from_utf8_lossy(&buf[..len])) {
                        if let LanPacket::Announce { ref mut address, ref mut udp_port, .. } = packet {
                            *address = source.ip().to_string();
                            *udp_port = source.port();
                        }
                        let _ = tx_in.send(packet);
                    }
                }
                Err(_) => {}
            }
        }
    });

    let (tx_out, rx_out) = mpsc::channel::<LanOutbound>();
    thread::spawn(move || {
        for item in rx_out {
            let _ = outbound.send_to(
                item.packet.encode().as_bytes(),
                (item.address.as_str(), item.port),
            );
        }
    });
    Some(tx_out)
}

fn spawn_tcp_client(
    addr: String,
    tx_in: Sender<ServerPacket>,
    rx_out: Receiver<String>,
) {
    thread::spawn(move || loop {
        match TcpStream::connect(&addr) {
            Ok(mut stream) => {
                let _ = stream.set_nodelay(true);
                println!("[wfo-player] Connected to native MMO server at {}", addr);
                let Ok(read_stream) = stream.try_clone() else {
                    thread::sleep(Duration::from_secs(2));
                    continue;
                };
                let tx_in_clone = tx_in.clone();
                thread::spawn(move || {
                    let mut reader = BufReader::new(read_stream);
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap_or(0) > 0 {
                        if let Some(pkt) = ServerPacket::decode(&line) {
                            let _ = tx_in_clone.send(pkt);
                        }
                        line.clear();
                    }
                });

                loop {
                    match rx_out.recv_timeout(Duration::from_millis(20)) {
                        Ok(msg) => {
                            let wire = format!("{}\n", msg);
                            if stream.write_all(wire.as_bytes()).is_err() {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                println!("[wfo-player] Disconnected from {}, reconnecting...", addr);
            }
            Err(_) => {
                // Drain stale queued movement packets while waiting for server
                while rx_out.try_recv().is_ok() {}
                thread::sleep(Duration::from_secs(2));
            }
        }
    });
}

struct PlayerApp {
    window: Option<Arc<Window>>,
    gpu: Option<GpuSurface>,
    engine: MmoClientEngine,
    rx_in: Receiver<ServerPacket>,
    tx_out: Sender<String>,
    rx_lan: Receiver<LanPacket>,
    tx_lan: Option<Sender<LanOutbound>>,
    last_frame: Instant,
    cursor_x: i32,
    cursor_y: i32,
}

impl PlayerApp {
    fn new(
        rx_in: Receiver<ServerPacket>,
        tx_out: Sender<String>,
        rx_lan: Receiver<LanPacket>,
        tx_lan: Option<Sender<LanOutbound>>,
    ) -> Self {
        #[cfg(target_os = "android")]
        let mut engine = MmoClientEngine::new(960, 600);
        #[cfg(target_os = "android")]
        {
            engine.touch_mode = true;
        }
        #[cfg(not(target_os = "android"))]
        let engine = MmoClientEngine::new(960, 600);
        Self {
            window: None,
            gpu: None,
            engine,
            rx_in,
            tx_out,
            rx_lan,
            tx_lan,
            last_frame: Instant::now(),
            cursor_x: 480,
            cursor_y: 300,
        }
    }

    fn is_typing(&self) -> bool {
        (!self.engine.joined && (self.engine.lobby_focus_name || self.engine.lobby_invite_focus))
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
            self.engine.apply_server_packet(pkt);
        }
        while let Ok(pkt) = self.rx_lan.try_recv() {
            self.engine.apply_lan_packet(pkt);
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        self.engine.tick(dt);
        for pkt in self.engine.outbox.drain(..) {
            let _ = self.tx_out.send(pkt.encode());
        }
        for pkt in self.engine.lan_outbox.drain(..) {
            let peer = self.engine.lan_duel_opponent.as_ref()
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
        self.engine.render();

        if let Some(gpu) = self.gpu.as_mut() {
            gpu.present(&self.engine.fb.pixels, self.engine.fb.width, self.engine.fb.height);
        }
    }
}

impl ApplicationHandler for PlayerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Wayfarers Online (WFO) — Native Rust MMO")
            .with_inner_size(LogicalSize::new(960.0, 600.0));
        let win = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create native MMO window"),
        );
        let size = win.inner_size();
        self.engine
            .resize(size.width.max(320) as usize, size.height.max(240) as usize);

        let gpu = GpuSurface::new(win.clone());
        self.window = Some(win);
        self.gpu = Some(gpu);
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
            } => match state {
                ElementState::Pressed => {
                    self.engine.on_pointer_down(self.cursor_x, self.cursor_y);
                }
                ElementState::Released => {
                    self.engine.on_pointer_up(self.cursor_x, self.cursor_y);
                }
            },
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
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(event);
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

fn resolve_server_addr() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!(
                    "Wayfarers Online (WFO) Native Rust MMO Player\nUsage: wfo-player [--server HOST:PORT]\nEnv: WFO_SERVER (default 127.0.0.1:7878)"
                );
                return None;
            }
            "--server" if i + 1 < args.len() => {
                return Some(args[i + 1].clone());
            }
            _ => {}
        }
        i += 1;
    }
    Some(std::env::var("WFO_SERVER").unwrap_or_else(|_| String::from("127.0.0.1:7878")))
}

fn main() {
    let Some(server_addr) = resolve_server_addr() else {
        return;
    };
    let (tx_in, rx_in) = mpsc::channel::<ServerPacket>();
    let (tx_out, rx_out) = mpsc::channel::<String>();
    let (tx_lan_in, rx_lan) = mpsc::channel::<LanPacket>();
    let tx_lan = spawn_lan_service(tx_lan_in);
    spawn_tcp_client(server_addr, tx_in, rx_out);

    let event_loop = EventLoop::new().expect("create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = PlayerApp::new(rx_in, tx_out, rx_lan, tx_lan);
    event_loop.run_app(&mut app).expect("run native MMO player");
}

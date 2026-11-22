use mmo_protocol::{ClassId, LanPacket, ServerPacket};
use mmo_renderer::MmoClientEngine;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    ElementState, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod gpu;
use gpu::GpuSurface;

const LAN_PORT: u16 = 7889;

struct LocalProfile {
    path: Option<PathBuf>,
    username: String,
    class_id: ClassId,
    level: i32,
    xp: i32,
    xp_next: i32,
    gold: i32,
}

impl LocalProfile {
    fn load(path: Option<PathBuf>) -> Self {
        let contents = path
            .as_ref()
            .and_then(|path| match fs::read_to_string(path) {
                Ok(contents) => Some(contents),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    eprintln!("[wfo-player] Could not read local profile: {error}");
                    None
                }
            })
            .unwrap_or_default();
        let mut lines = contents.lines();
        let username = lines.next().unwrap_or("").trim();
        let username = if valid_local_username(username) {
            username.to_string()
        } else {
            String::new()
        };
        let class_id = lines
            .next()
            .map(ClassId::from_str_code)
            .unwrap_or(ClassId::Vanguard);
        let level = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|level| (1..=10_000).contains(level))
            .unwrap_or(1);
        let xp_next = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|xp_next| *xp_next > 0)
            .unwrap_or(100);
        let xp = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|xp| (0..xp_next).contains(xp))
            .unwrap_or(0);
        let gold = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|gold| *gold >= 0)
            .unwrap_or(0);
        Self {
            path,
            username,
            class_id,
            level,
            xp,
            xp_next,
            gold,
        }
    }

    fn save(
        &mut self,
        username: &str,
        class_id: ClassId,
        level: i32,
        xp: i32,
        xp_next: i32,
        gold: i32,
    ) {
        let username = username.trim();
        if (!username.is_empty() && !valid_local_username(username))
            || (self.username == username
                && self.class_id == class_id
                && self.level == level
                && self.xp == xp
                && self.xp_next == xp_next
                && self.gold == gold)
        {
            return;
        }
        self.username = username.to_string();
        self.class_id = class_id;
        self.level = level.clamp(1, 10_000);
        self.xp_next = xp_next.max(1);
        self.xp = xp.clamp(0, self.xp_next - 1);
        self.gold = gold.max(0);
        if let Some(path) = self.path.as_ref() {
            let result = (|| {
                if let Some(parent) = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    fs::create_dir_all(parent)?;
                }
                fs::write(
                    path,
                    format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        self.username,
                        class_id.as_code(),
                        self.level,
                        self.xp_next,
                        self.xp,
                        self.gold
                    ),
                )
            })();
            if let Err(error) = result {
                eprintln!("[wfo-player] Could not save local profile: {error}");
            }
        }
    }
}

fn valid_local_username(name: &str) -> bool {
    (3..=18).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn local_profile_path(_android_data_dir: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WFO_PROFILE_PATH") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "android")]
    {
        return _android_data_dir.map(|dir| dir.join("wfo-profile.txt"));
    }
    #[cfg(not(target_os = "android"))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
        base.map(|dir| dir.join("WFO").join("profile.txt"))
    }
}

struct LanOutbound {
    packet: LanPacket,
    address: String,
    port: u16,
}

fn spawn_lan_service(tx_in: Sender<LanPacket>) -> Option<Sender<LanOutbound>> {
    let listener = UdpSocket::bind(("0.0.0.0", LAN_PORT)).ok()?;
    listener.set_broadcast(true).ok();
    listener
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok();
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
                    if let Some(mut packet) =
                        LanPacket::decode(&String::from_utf8_lossy(&buf[..len]))
                    {
                        if let LanPacket::Announce {
                            ref mut address,
                            ref mut udp_port,
                            ..
                        } = packet
                        {
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

fn build_client_tls_config(
    app_data_dir: Option<PathBuf>,
) -> Result<Arc<ClientConfig>, Box<dyn std::error::Error>> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let ca_path = std::env::var_os("WFO_CA_CERT")
        .map(PathBuf::from)
        .or_else(|| app_data_dir.map(|dir| dir.join("wfo-ca.pem")));
    if let Some(ca_path) = ca_path {
        if !ca_path.exists() {
            if std::env::var_os("WFO_CA_CERT").is_some() {
                return Err(format!("CA certificate does not exist: {}", ca_path.display()).into());
            }
        } else {
            let certs = rustls_pemfile::certs(&mut BufReader::new(fs::File::open(ca_path)?))
                .collect::<Result<Vec<_>, _>>()?;
            for cert in certs {
                roots.add(cert)?;
            }
        }
    }
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

fn server_name_from_addr(addr: &str) -> Result<ServerName<'static>, Box<dyn std::error::Error>> {
    let host = if let Some(bracketed) = addr.strip_prefix('[') {
        bracketed
            .split_once(']')
            .map(|(host, _)| host)
            .unwrap_or(bracketed)
    } else {
        addr.rsplit_once(':').map(|(host, _)| host).unwrap_or(addr)
    };
    Ok(ServerName::try_from(host.to_string())?)
}

fn spawn_tls_client(
    addr: String,
    tx_in: Sender<ServerPacket>,
    rx_out: Receiver<String>,
    tls_config: Arc<ClientConfig>,
) {
    thread::spawn(move || {
        let server_name = match server_name_from_addr(&addr) {
            Ok(name) => name,
            Err(error) => {
                eprintln!("[wfo-player] Invalid TLS server name: {error}");
                return;
            }
        };
        let mut was_connected = false;
        let mut reported_failure = false;
        loop {
            let result = TcpStream::connect(&addr).and_then(|socket| {
                let _ = socket.set_nodelay(true);
                let connection =
                    ClientConnection::new(Arc::clone(&tls_config), server_name.clone())
                        .map_err(std::io::Error::other)?;
                let mut stream = StreamOwned::new(connection, socket);
                while stream.conn.is_handshaking() {
                    stream
                        .conn
                        .complete_io(&mut stream.sock)
                        .map_err(std::io::Error::other)?;
                }
                stream
                    .sock
                    .set_read_timeout(Some(Duration::from_millis(40)))?;

                was_connected = true;
                reported_failure = false;
                let _ = tx_in.send(ServerPacket::ConnectionStatus { connected: true });
                println!("[wfo-player] Securely connected to {}", addr);

                let mut pending = Vec::new();
                let mut buf = [0u8; 2048];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(read) => {
                            pending.extend_from_slice(&buf[..read]);
                            if pending.len() > 4096 {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "server packet too large",
                                ));
                            }
                            while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                                let line = pending.drain(..=end).collect::<Vec<_>>();
                                if let Some(packet) =
                                    ServerPacket::decode(&String::from_utf8_lossy(&line))
                                {
                                    let _ = tx_in.send(packet);
                                }
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(error) => return Err(error),
                    }

                    while let Ok(message) = rx_out.try_recv() {
                        writeln!(stream, "{message}")?;
                    }
                    stream.flush()?;
                }
                Ok(())
            });

            match result {
                Err(error) if was_connected => {
                    let _ = tx_in.send(ServerPacket::ConnectionStatus { connected: false });
                    was_connected = false;
                    reported_failure = true;
                    eprintln!("[wfo-player] Secure connection lost: {error}");
                }
                Err(error) if !reported_failure => {
                    let _ = tx_in.send(ServerPacket::ConnectionStatus { connected: false });
                    reported_failure = true;
                    eprintln!("[wfo-player] Could not establish TLS connection: {error}");
                }
                Ok(()) if was_connected => {
                    let _ = tx_in.send(ServerPacket::ConnectionStatus { connected: false });
                    was_connected = false;
                    reported_failure = true;
                    eprintln!("[wfo-player] Server closed the secure connection.");
                }
                _ => {}
            }
            thread::sleep(Duration::from_secs(2));
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
    profile: LocalProfile,
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
                    "Wayfarers Online (WFO) Native Rust MMO Player\nUsage: wfo-player [--server HOST:PORT]\nEnv: WFO_SERVER (default 127.0.0.1:7878, TLS), WFO_CA_CERT (optional PEM CA)"
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

fn run(android_data_dir: Option<PathBuf>) {
    let Some(server_addr) = resolve_server_addr() else {
        return;
    };
    let tls_config = build_client_tls_config(android_data_dir.clone())
        .unwrap_or_else(|error| panic!("configure trusted TLS certificates: {error}"));
    let profile = LocalProfile::load(local_profile_path(android_data_dir));
    let (tx_in, rx_in) = mpsc::channel::<ServerPacket>();
    let (tx_out, rx_out) = mpsc::channel::<String>();
    let (tx_lan_in, rx_lan) = mpsc::channel::<LanPacket>();
    let tx_lan = spawn_lan_service(tx_lan_in);
    spawn_tls_client(server_addr, tx_in, rx_out, tls_config);

    let event_loop = EventLoop::new().expect("create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = PlayerApp::new(rx_in, tx_out, rx_lan, tx_lan, profile);
    event_loop.run_app(&mut app).expect("run native MMO player");
}

fn main() {
    run(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_profile_persists_offline_progress_and_ignores_invalid_input() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("wfo-profile-{}-{nonce}.txt", std::process::id()));
        let mut profile = LocalProfile::load(Some(path.clone()));
        profile.save("Player_01", ClassId::Arcanist, 4, 17, 200, 39);
        assert_eq!(
            fs::read_to_string(&path).expect("read profile"),
            "Player_01\narcanist\n4\n200\n17\n39"
        );

        let mut loaded = LocalProfile::load(Some(path.clone()));
        assert_eq!(loaded.username, "Player_01");
        assert_eq!(loaded.class_id, ClassId::Arcanist);
        assert_eq!(
            (loaded.level, loaded.xp, loaded.xp_next, loaded.gold),
            (4, 17, 200, 39)
        );
        loaded.save("not valid", ClassId::Ranger, 1, 0, 100, 0);
        assert_eq!(
            fs::read_to_string(&path).expect("profile remains unchanged"),
            "Player_01\narcanist\n4\n200\n17\n39"
        );
        fs::write(&path, "Legacy_User").expect("write previous profile format");
        let legacy = LocalProfile::load(Some(path.clone()));
        assert_eq!(legacy.username, "Legacy_User");
        assert_eq!(legacy.class_id, ClassId::Vanguard);
        assert_eq!(
            (legacy.level, legacy.xp, legacy.xp_next, legacy.gold),
            (1, 0, 100, 0)
        );
        std::fs::remove_file(path).expect("remove test profile");
    }
}

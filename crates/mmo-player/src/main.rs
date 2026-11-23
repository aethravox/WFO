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
    ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod gpu;
use gpu::GpuSurface;

const LAN_PORT: u16 = 7889;

mod app;
mod lan;
mod profile;

use app::PlayerApp;
use lan::{build_client_tls_config, spawn_lan_service, spawn_tls_client};
use profile::{local_profile_path, LocalProfile};

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
#[cfg(test)]
mod tests;

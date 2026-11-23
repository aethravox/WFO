use super::*;

pub(crate) struct LanOutbound {
    pub(crate) packet: LanPacket,
    pub(crate) address: String,
    pub(crate) port: u16,
}

pub(crate) fn spawn_lan_service(tx_in: Sender<LanPacket>) -> Option<Sender<LanOutbound>> {
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

pub(crate) fn build_client_tls_config(
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

pub(crate) fn server_name_from_addr(
    addr: &str,
) -> Result<ServerName<'static>, Box<dyn std::error::Error>> {
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

pub(crate) fn spawn_tls_client(
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

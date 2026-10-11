//! Experimental LAN sync between Copyosity installs (cargo feature `lan-sync`, off by default,
//! never enabled by `scripts/release.sh`). Plan: docs/plans/windows-port-and-lan-sync.md, part B.
//!
//! - discovery: mDNS `_copyosity._tcp.local.` with the device id/name/OS in TXT;
//! - pairing: one device shows a 6-digit code, the other enters it (SPAKE2, see crypto.rs);
//! - sync: TCP on [`PORT`], encrypted frames, both sides stream their `sync_log` changes
//!   (store.rs); only paired devices on private addresses are accepted.
//!
//! Threads, not async: the app's DB is a `Mutex<Connection>` and the traffic is tiny.

pub mod crypto;
pub mod store;

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::Database;

pub const PORT: u16 = 47900;
const SERVICE: &str = "_copyosity._tcp.local.";
const PROTO: u32 = 1;
const PAIRING_TTL: Duration = Duration::from_secs(300);
const POLL: Duration = Duration::from_millis(700);
const PING_EVERY: Duration = Duration::from_secs(15);
const BATCH: i64 = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    pub device_id: String,
    pub name: String,
    pub os: String,
    pub addr: String,
    #[serde(skip)]
    pub sock: Option<SocketAddr>,
}

pub struct LanSync {
    app: AppHandle,
    db: Arc<Database>,
    pub device_id: String,
    pub name: String,
    pub os: &'static str,
    enabled: AtomicBool,
    /// Bumped on every enable, so threads from a previous run notice they are stale.
    generation: AtomicU64,
    pairing: Mutex<Option<(String, Instant)>>,
    discovered: Mutex<HashMap<String, Discovered>>,
    online: Mutex<HashSet<String>>,
    last_error: Mutex<Option<String>>,
    mdns: Mutex<Option<mdns_sd::ServiceDaemon>>,
}

fn os_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

fn device_name() -> String {
    let raw = gethostname::gethostname().to_string_lossy().into_owned();
    let name = raw.trim_end_matches(".local").to_owned();
    if name.is_empty() {
        os_name().to_owned()
    } else {
        name
    }
}

/// Only peers on the local network: RFC 1918, link-local, CGNAT-free private ranges, loopback.
fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Msg {
    Hello {
        proto: u32,
        name: String,
        os: String,
        since: i64,
    },
    Batch {
        changes: Vec<store::Change>,
        upto: i64,
    },
    Ping,
}

impl LanSync {
    fn with_db<R>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R>,
    ) -> Result<R, String> {
        let mut conn = self
            .db
            .conn
            .lock()
            .map_err(|_| "db lock poisoned".to_owned())?;
        f(&mut conn).map_err(|e| e.to_string())
    }

    fn set_error(&self, e: impl Into<String>) {
        let e = e.into();
        eprintln!("[lan-sync] {e}");
        *self.last_error.lock().unwrap() = Some(e);
        self.notify();
    }

    fn notify(&self) {
        let _ = self.app.emit("lan-sync-status", ());
    }

    fn running(&self, generation: u64) -> bool {
        self.enabled.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == generation
    }

    pub fn set_enabled(self: &Arc<Self>, on: bool) -> Result<(), String> {
        self.with_db(|c| store::set_state(c, "enabled", if on { "1" } else { "0" }))?;
        let was = self.enabled.swap(on, Ordering::AcqRel);
        if on && !was {
            self.start();
        } else if !on && was {
            if let Some(d) = self.mdns.lock().unwrap().take() {
                let _ = d.shutdown();
            }
            self.online.lock().unwrap().clear();
            self.discovered.lock().unwrap().clear();
        }
        self.notify();
        Ok(())
    }

    fn start(self: &Arc<Self>) {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.last_error.lock().unwrap() = None;
        let me = self.clone();
        std::thread::spawn(move || me.listen(generation));
        let me = self.clone();
        std::thread::spawn(move || me.discover(generation));
        let me = self.clone();
        std::thread::spawn(move || me.connector(generation));
    }

    // ---------- discovery ----------

    fn discover(self: Arc<Self>, generation: u64) {
        let daemon = match mdns_sd::ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => return self.set_error(format!("mDNS: {e}")),
        };
        let host = format!("copyosity-{}.local.", &self.device_id[..8]);
        let props = [
            ("id", self.device_id.as_str()),
            ("name", self.name.as_str()),
            ("os", self.os),
        ];
        match mdns_sd::ServiceInfo::new(SERVICE, &self.device_id, &host, "", PORT, &props[..]) {
            Ok(info) => {
                if let Err(e) = daemon.register(info.enable_addr_auto()) {
                    self.set_error(format!("mDNS register: {e}"));
                }
            }
            Err(e) => self.set_error(format!("mDNS info: {e}")),
        }
        let rx = match daemon.browse(SERVICE) {
            Ok(rx) => rx,
            Err(e) => return self.set_error(format!("mDNS browse: {e}")),
        };
        *self.mdns.lock().unwrap() = Some(daemon);
        while self.running(generation) {
            let Ok(event) = rx.recv_timeout(Duration::from_secs(1)) else {
                continue;
            };
            if let mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) = &event {
                // fullname is "<device_id>._copyosity._tcp.local."
                let id = fullname.split('.').next().unwrap_or_default().to_owned();
                if self.discovered.lock().unwrap().remove(&id).is_some() {
                    self.notify();
                }
                continue;
            }
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                let Some(id) = info.get_property_val_str("id").map(str::to_owned) else {
                    continue;
                };
                if id == self.device_id {
                    continue;
                }
                let ip = info
                    .get_addresses()
                    .iter()
                    .copied()
                    .find(|ip| ip.is_ipv4() && is_local(*ip));
                let name = info.get_property_val_str("name").unwrap_or("?").to_owned();
                let os = info.get_property_val_str("os").unwrap_or("?").to_owned();
                let sock = ip.map(|ip| SocketAddr::new(ip, info.get_port()));
                self.discovered.lock().unwrap().insert(
                    id.clone(),
                    Discovered {
                        device_id: id,
                        name,
                        os,
                        addr: sock.map(|s| s.to_string()).unwrap_or_default(),
                        sock,
                    },
                );
                self.notify();
            }
        }
    }

    // ---------- server ----------

    fn listen(self: Arc<Self>, generation: u64) {
        let listener = match TcpListener::bind(("0.0.0.0", PORT)) {
            Ok(l) => l,
            Err(e) => return self.set_error(format!("port {PORT}: {e}")),
        };
        let _ = listener.set_nonblocking(true);
        while self.running(generation) {
            match listener.accept() {
                Ok((stream, addr)) => {
                    if !is_local(addr.ip()) {
                        continue;
                    }
                    let _ = stream.set_nonblocking(false);
                    let me = self.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = me.serve(stream, generation) {
                            eprintln!("[lan-sync] incoming {addr}: {e}");
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(200))
                }
                Err(e) => {
                    self.set_error(format!("accept: {e}"));
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    fn serve(self: Arc<Self>, stream: TcpStream, generation: u64) -> Result<(), String> {
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut writer = stream;
        let first: serde_json::Value = read_json_line(&mut reader)?;
        let peer_id = first["device_id"].as_str().unwrap_or_default().to_owned();
        match first["t"].as_str() {
            Some("pair") => {
                let code = {
                    let p = self.pairing.lock().unwrap();
                    match &*p {
                        Some((code, until)) if Instant::now() < *until => code.clone(),
                        _ => {
                            write_json_line(&mut writer, &json!({"error": "not_pairing"}))?;
                            return Err("pair request while not in pairing mode".into());
                        }
                    }
                };
                write_json_line(
                    &mut writer,
                    &json!({"device_id": self.device_id, "name": self.name, "os": self.os}),
                )?;
                let name = first["name"].as_str().unwrap_or("?").to_owned();
                let os = first["os"].as_str().unwrap_or("?").to_owned();
                let secret =
                    run_pairing(&mut reader, &mut writer, &code, &self.device_id, &peer_id)?;
                self.with_db(|c| store::save_peer(c, &peer_id, &name, &os, &hex::encode(secret)))?;
                *self.pairing.lock().unwrap() = None;
                self.notify();
                Ok(())
            }
            Some("sync") => {
                let peer = self
                    .with_db(|c| store::peers(c))?
                    .into_iter()
                    .find(|p| p.device_id == peer_id);
                let Some(peer) = peer else {
                    write_json_line(&mut writer, &json!({"error": "unknown_device"}))?;
                    return Err(format!("sync from unpaired device {peer_id}"));
                };
                let nc = first["nonce"].as_str().unwrap_or_default().to_owned();
                let ns = crypto::random_hex(16);
                write_json_line(&mut writer, &json!({"nonce": ns}))?;
                let secret = hex::decode(&peer.secret_hex).map_err(|e| e.to_string())?;
                self.session(
                    writer, reader, &secret, &nc, &ns, false, &peer_id, generation,
                )
            }
            _ => Err("unknown hello".into()),
        }
    }

    // ---------- client ----------

    /// The device with the smaller id dials, so a pair keeps exactly one connection.
    fn connector(self: Arc<Self>, generation: u64) {
        while self.running(generation) {
            let peers = self.with_db(|c| store::peers(c)).unwrap_or_default();
            for p in peers {
                if p.device_id.as_str() <= self.device_id.as_str()
                    || self.online.lock().unwrap().contains(&p.device_id)
                {
                    continue;
                }
                let sock = self
                    .discovered
                    .lock()
                    .unwrap()
                    .get(&p.device_id)
                    .and_then(|d| d.sock);
                let Some(sock) = sock else { continue };
                let me = self.clone();
                std::thread::spawn(move || {
                    if let Err(e) = me.dial(sock, &p.device_id, &p.secret_hex, generation) {
                        eprintln!("[lan-sync] dial {}: {e}", p.device_id);
                    }
                });
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    fn dial(
        self: Arc<Self>,
        sock: SocketAddr,
        peer_id: &str,
        secret_hex: &str,
        generation: u64,
    ) -> Result<(), String> {
        if !self.online.lock().unwrap().insert(peer_id.to_owned()) {
            return Ok(());
        }
        let result = (|| {
            let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(4))
                .map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
            let mut writer = stream;
            let nc = crypto::random_hex(16);
            write_json_line(
                &mut writer,
                &json!({"t": "sync", "device_id": self.device_id, "nonce": nc}),
            )?;
            let reply: serde_json::Value = read_json_line(&mut reader)?;
            if let Some(err) = reply["error"].as_str() {
                return Err(format!("peer refused: {err}"));
            }
            let ns = reply["nonce"].as_str().unwrap_or_default().to_owned();
            let secret = hex::decode(secret_hex).map_err(|e| e.to_string())?;
            self.clone()
                .session_inner(writer, reader, &secret, &nc, &ns, true, peer_id, generation)
        })();
        self.online.lock().unwrap().remove(peer_id);
        self.notify();
        result
    }

    /// Pair with a discovered device using the code it shows.
    pub fn pair_with(self: &Arc<Self>, device_id: &str, code: &str) -> Result<(), String> {
        let d = self
            .discovered
            .lock()
            .unwrap()
            .get(device_id)
            .cloned()
            .ok_or("device not found on the network")?;
        let sock = d.sock.ok_or("device has no local address")?;
        let stream =
            TcpStream::connect_timeout(&sock, Duration::from_secs(4)).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut writer = stream;
        write_json_line(
            &mut writer,
            &json!({"t": "pair", "device_id": self.device_id, "name": self.name, "os": self.os}),
        )?;
        let reply: serde_json::Value = read_json_line(&mut reader)?;
        if reply["error"].as_str() == Some("not_pairing") {
            return Err("На другом устройстве не открыт код сопряжения".into());
        }
        let peer_id = reply["device_id"].as_str().unwrap_or_default().to_owned();
        if peer_id != device_id {
            return Err("device id mismatch".into());
        }
        let secret = run_pairing(
            &mut reader,
            &mut writer,
            code.trim(),
            &self.device_id,
            &peer_id,
        )?;
        let name = reply["name"].as_str().unwrap_or(&d.name).to_owned();
        let os = reply["os"].as_str().unwrap_or(&d.os).to_owned();
        self.with_db(|c| store::save_peer(c, &peer_id, &name, &os, &hex::encode(secret)))?;
        self.notify();
        Ok(())
    }

    // ---------- session ----------

    #[allow(clippy::too_many_arguments)]
    fn session(
        self: Arc<Self>,
        writer: TcpStream,
        reader: BufReader<TcpStream>,
        secret: &[u8],
        nc: &str,
        ns: &str,
        initiator: bool,
        peer_id: &str,
        generation: u64,
    ) -> Result<(), String> {
        if !self.online.lock().unwrap().insert(peer_id.to_owned()) {
            return Err("already connected".into());
        }
        let r = self.clone().session_inner(
            writer, reader, secret, nc, ns, initiator, peer_id, generation,
        );
        self.online.lock().unwrap().remove(peer_id);
        self.notify();
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn session_inner(
        self: Arc<Self>,
        writer: TcpStream,
        reader: BufReader<TcpStream>,
        secret: &[u8],
        nc: &str,
        ns: &str,
        initiator: bool,
        peer_id: &str,
        generation: u64,
    ) -> Result<(), String> {
        let shutdown = writer.try_clone().map_err(|e| e.to_string())?;
        // Pings keep the 20 s read timeout from firing on an idle but healthy link.
        let (mut tx, mut rx) = crypto::channel(writer, reader, secret, nc, ns, initiator);
        let since = self.with_db(|c| {
            Ok(store::peers(c)?
                .into_iter()
                .find(|p| p.device_id == peer_id)
                .map(|p| p.last_recv_seq)
                .unwrap_or(0))
        })?;
        send_msg(
            &mut tx,
            &Msg::Hello {
                proto: PROTO,
                name: self.name.clone(),
                os: self.os.to_owned(),
                since,
            },
        )?;
        let their_since = match recv_msg(&mut rx)? {
            Msg::Hello { since, .. } => since,
            _ => return Err("expected hello".into()),
        };
        self.with_db(|c| store::touch_peer(c, peer_id))?;
        self.notify();

        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();
        let me = self.clone();
        let peer = peer_id.to_owned();
        let reader_thread = std::thread::spawn(move || {
            let r = (|| -> Result<(), String> {
                loop {
                    match recv_msg(&mut rx)? {
                        Msg::Batch { changes, upto } => {
                            let n = me.with_db(|c| {
                                let n = store::apply_changes(c, &peer, &changes)?;
                                store::mark_received(c, &peer, upto)?;
                                Ok(n)
                            })?;
                            if n > 0 {
                                let _ = me.app.emit("history-changed", ());
                            }
                            me.notify();
                        }
                        Msg::Ping => {
                            let _ = me.with_db(|c| store::touch_peer(c, &peer));
                        }
                        Msg::Hello { .. } => {}
                    }
                }
            })();
            reader_alive.store(false, Ordering::Release);
            r
        });

        let mut sent_upto = their_since;
        let mut last_ping = Instant::now();
        let result: Result<(), String> = loop {
            if !alive.load(Ordering::Acquire)
                || !self.running(generation)
                || !self.is_paired(peer_id)
            {
                break Ok(());
            }
            let (changes, top) = self.with_db(|c| {
                Ok((
                    store::changes_since(c, sent_upto, peer_id, BATCH)?,
                    store::max_seq(c)?,
                ))
            })?;
            if let Some(last) = changes.last().map(|c| c.seq) {
                if let Err(e) = send_msg(
                    &mut tx,
                    &Msg::Batch {
                        changes,
                        upto: last,
                    },
                ) {
                    break Err(e);
                }
                sent_upto = last;
                continue;
            }
            sent_upto = sent_upto.max(top);
            if last_ping.elapsed() >= PING_EVERY {
                if let Err(e) = send_msg(&mut tx, &Msg::Ping) {
                    break Err(e);
                }
                last_ping = Instant::now();
            }
            std::thread::sleep(POLL);
        };
        let _ = shutdown.shutdown(std::net::Shutdown::Both);
        let reader_result = reader_thread
            .join()
            .unwrap_or_else(|_| Err("reader panicked".into()));
        result.and(reader_result.or_else(|e| {
            if e.contains("failed to fill") || e.contains("os error") {
                Ok(())
            } else {
                Err(e)
            }
        }))
    }

    fn is_paired(&self, peer_id: &str) -> bool {
        self.with_db(|c| store::peers(c))
            .map(|p| p.iter().any(|p| p.device_id == peer_id))
            .unwrap_or(false)
    }

    // ---------- commands ----------

    pub fn start_pairing(&self) -> String {
        let code = crypto::random_code();
        *self.pairing.lock().unwrap() = Some((code.clone(), Instant::now() + PAIRING_TTL));
        self.notify();
        code
    }

    pub fn cancel_pairing(&self) {
        *self.pairing.lock().unwrap() = None;
        self.notify();
    }

    pub fn unpair(&self, device_id: &str) -> Result<(), String> {
        self.with_db(|c| store::remove_peer(c, device_id))?;
        self.notify();
        Ok(())
    }

    pub fn status(&self) -> serde_json::Value {
        let peers = self.with_db(|c| store::peers(c)).unwrap_or_default();
        let paired: HashSet<String> = peers.iter().map(|p| p.device_id.clone()).collect();
        let online = self.online.lock().unwrap().clone();
        let found: Vec<serde_json::Value> = self
            .discovered
            .lock()
            .unwrap()
            .values()
            .filter(|d| !paired.contains(&d.device_id))
            .map(|d| json!({"deviceId": d.device_id, "name": d.name, "os": d.os, "addr": d.addr}))
            .collect();
        let peers: Vec<serde_json::Value> = peers
            .iter()
            .map(|p| {
                json!({"deviceId": p.device_id, "name": p.name, "os": p.os, "lastSeen": p.last_seen,
                       "online": online.contains(&p.device_id),
                       "visible": self.discovered.lock().unwrap().contains_key(&p.device_id)})
            })
            .collect();
        let pairing = self
            .pairing
            .lock()
            .unwrap()
            .clone()
            .filter(|(_, until)| Instant::now() < *until);
        json!({
            "enabled": self.enabled.load(Ordering::Acquire),
            "deviceId": self.device_id, "name": self.name, "os": self.os, "port": PORT,
            "pairingCode": pairing.as_ref().map(|(c, _)| c.clone()),
            "pairingSecondsLeft": pairing.map(|(_, until)| until.saturating_duration_since(Instant::now()).as_secs()),
            "discovered": found, "peers": peers,
            "lastError": self.last_error.lock().unwrap().clone(),
        })
    }
}

fn run_pairing(
    reader: &mut BufReader<TcpStream>,
    writer: &mut TcpStream,
    code: &str,
    me: &str,
    peer: &str,
) -> Result<[u8; 32], String> {
    let (state, outbound) = crypto::pair_start(code);
    write_json_line(writer, &json!({"spake": hex::encode(&outbound)}))?;
    let msg: serde_json::Value = read_json_line(reader)?;
    let inbound =
        hex::decode(msg["spake"].as_str().unwrap_or_default()).map_err(|e| e.to_string())?;
    let key = crypto::pair_finish(state, &inbound)?;
    write_json_line(
        writer,
        &json!({"confirm": hex::encode(crypto::confirm_tag(&key, me))}),
    )?;
    let theirs: serde_json::Value = read_json_line(reader)?;
    if theirs["confirm"].as_str() != Some(hex::encode(crypto::confirm_tag(&key, peer)).as_str()) {
        return Err("Неверный код".into());
    }
    Ok(crypto::pair_secret(&key, me, peer))
}

fn read_json_line<R: BufRead, T: serde::de::DeserializeOwned>(r: &mut R) -> Result<T, String> {
    let mut line = String::new();
    let n = std::io::Read::take(&mut *r, 64 * 1024)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("connection closed".into());
    }
    serde_json::from_str(&line).map_err(|e| e.to_string())
}

fn write_json_line<W: Write>(w: &mut W, v: &serde_json::Value) -> Result<(), String> {
    let mut s = v.to_string();
    s.push('\n');
    w.write_all(s.as_bytes())
        .and_then(|_| w.flush())
        .map_err(|e| e.to_string())
}

fn send_msg(tx: &mut crypto::Sender<TcpStream>, m: &Msg) -> Result<(), String> {
    tx.send(&serde_json::to_vec(m).map_err(|e| e.to_string())?)
}

fn recv_msg(rx: &mut crypto::Receiver<BufReader<TcpStream>>) -> Result<Msg, String> {
    serde_json::from_slice(&rx.recv()?).map_err(|e| e.to_string())
}

/// Set up tables/triggers and start networking if the user turned sync on before.
pub fn init(app: &AppHandle) {
    let db = app.state::<Arc<Database>>().inner().clone();
    let (device_id, enabled) = {
        let conn = db.conn.lock().unwrap();
        if let Err(e) = store::ensure_schema(&conn) {
            eprintln!("[lan-sync] schema: {e}");
            return;
        }
        let id = match store::get_state(&conn, "device_id").ok().flatten() {
            Some(id) => id,
            None => {
                let id = crypto::random_hex(16);
                let _ = store::set_state(&conn, "device_id", &id);
                id
            }
        };
        let enabled = store::get_state(&conn, "enabled").ok().flatten().as_deref() == Some("1");
        (id, enabled)
    };
    let sync = Arc::new(LanSync {
        app: app.clone(),
        db,
        device_id,
        name: device_name(),
        os: os_name(),
        enabled: AtomicBool::new(false),
        generation: AtomicU64::new(0),
        pairing: Mutex::new(None),
        discovered: Mutex::new(HashMap::new()),
        online: Mutex::new(HashSet::new()),
        last_error: Mutex::new(None),
        mdns: Mutex::new(None),
    });
    app.manage(sync.clone());
    if enabled {
        let _ = sync.set_enabled(true);
    }
}

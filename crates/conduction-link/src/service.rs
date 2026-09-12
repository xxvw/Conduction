use crate::{
    dbserver, nfs, wire::*, LibrarySnapshot, LinkConfig, LinkDevice, LinkError, LinkEvent,
    LinkStatus, LocalClock,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SOURCE_NUMBER: u8 = 17;
const DEVICE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
struct Ports {
    discovery: u16,
    beat: u16,
    status: u16,
    query: u16,
    database: u16,
    nfs: u16,
}

impl Default for Ports {
    fn default() -> Self {
        Self {
            discovery: 50000,
            beat: 50001,
            status: 50002,
            query: 12523,
            database: 1051,
            nfs: 50111,
        }
    }
}

enum Command {
    Master,
}

/// Owns all network tasks. Dropping/stopping it closes the listeners and joins
/// their threads; it never controls local audio playback on network loss.
pub struct LinkHandle {
    config: LinkConfig,
    state: Arc<RwLock<LinkStatus>>,
    library: Arc<RwLock<LibrarySnapshot>>,
    clock: Arc<RwLock<(LocalClock, Instant)>>,
    stop: Arc<AtomicBool>,
    commands: SyncSender<Command>,
    events: Mutex<Receiver<LinkEvent>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl LinkHandle {
    pub fn start(config: LinkConfig, library: LibrarySnapshot) -> Result<Self, LinkError> {
        Self::start_with_ports(config, library, Ports::default())
    }

    fn start_with_ports(
        config: LinkConfig,
        library: LibrarySnapshot,
        mut ports: Ports,
    ) -> Result<Self, LinkError> {
        if config.enabled {
            validate_config(&config)?;
        }
        let state = Arc::new(RwLock::new(LinkStatus {
            running: config.enabled,
            library_tracks: if config.library_enabled {
                library.tracks.len()
            } else {
                0
            },
            ..LinkStatus::default()
        }));
        let library = Arc::new(RwLock::new(library));
        let clock = Arc::new(RwLock::new((LocalClock::default(), Instant::now())));
        let stop = Arc::new(AtomicBool::new(false));
        let (events_tx, events_rx) = mpsc::sync_channel(1024);
        let (command_tx, command_rx) = mpsc::sync_channel(32);
        let mut handles = Vec::new();
        if config.enabled {
            // No address reuse: another DJ application sharing the fixed ports
            // must produce a visible bind error instead of losing packets.
            let bind_ip = if config.interface_ip.is_loopback() {
                config.interface_ip
            } else {
                Ipv4Addr::UNSPECIFIED
            };
            let discovery = UdpSocket::bind((bind_ip, ports.discovery))?;
            let beat = UdpSocket::bind((bind_ip, ports.beat))?;
            let status = UdpSocket::bind((config.interface_ip, ports.status))?;
            for socket in [&discovery, &beat, &status] {
                socket.set_nonblocking(true)?;
                socket.set_broadcast(true)?;
            }
            ports.discovery = discovery.local_addr()?.port();
            ports.beat = beat.local_addr()?.port();
            ports.status = status.local_addr()?.port();
            if config.library_enabled {
                handles.extend(dbserver::start_dbserver(
                    config.interface_ip,
                    ports.query,
                    ports.database,
                    library.clone(),
                    stop.clone(),
                )?);
                match nfs::start_nfs(
                    config.interface_ip,
                    ports.nfs,
                    library.clone(),
                    stop.clone(),
                ) {
                    Ok(handle) => handles.push(handle),
                    Err(error) => {
                        stop.store(true, Ordering::Release);
                        for handle in handles {
                            let _ = handle.join();
                        }
                        return Err(error.into());
                    }
                }
            }
            let worker = Worker {
                config: config.clone(),
                ports,
                discovery,
                beat,
                status,
                state: state.clone(),
                library: library.clone(),
                clock: clock.clone(),
                stop: stop.clone(),
                events: events_tx,
                commands: command_rx,
                peers: BTreeMap::new(),
                library_greeted: HashSet::new(),
                seen: HashMap::new(),
                excluded: HashMap::new(),
                phase: Phase::Observe,
                phase_at: Instant::now() + Duration::from_secs(3),
                identity: WireIdentity {
                    number: 0,
                    name: "Conduction".into(),
                    ip: config.interface_ip,
                    mac: config.mac_address,
                },
                master: None,
                handoff: None,
                requesting_master: None,
                sync_counter: 0,
                first: true,
                last_keepalive: Instant::now() - Duration::from_secs(2),
                last_status: Instant::now(),
                last_beat: None,
                packet_counter: 0,
                library_ready_at: Instant::now() + Duration::from_secs(3),
            };
            match thread::Builder::new()
                .name("conduction-link".into())
                .spawn(move || worker.run())
            {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    stop.store(true, Ordering::Release);
                    for handle in handles {
                        let _ = handle.join();
                    }
                    return Err(error.into());
                }
            }
        }
        Ok(Self {
            config,
            state,
            library,
            clock,
            stop,
            commands: command_tx,
            events: Mutex::new(events_rx),
            threads: Mutex::new(handles),
        })
    }

    pub fn config(&self) -> LinkConfig {
        self.config.clone()
    }

    pub fn snapshot(&self) -> LinkStatus {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update_library(&self, library: LibrarySnapshot) {
        let count = library.tracks.len();
        *self.library.write().unwrap_or_else(|e| e.into_inner()) = library;
        self.state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .library_tracks = if self.config.library_enabled {
            count
        } else {
            0
        };
    }

    pub fn publish_clock(&self, clock: LocalClock) {
        if clock.deck != self.config.source_deck {
            return;
        }
        if !clock.bpm.is_finite() || !clock.beat_phase.is_finite() {
            return;
        }
        *self.clock.write().unwrap_or_else(|e| e.into_inner()) = (clock, Instant::now());
    }

    pub fn request_master(&self) {
        let _ = self.commands.try_send(Command::Master);
    }

    pub fn recv_event(&self) -> Option<LinkEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_recv()
            .ok()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        for handle in self
            .threads
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
        {
            let _ = handle.join();
        }
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        state.running = false;
        state.player_number = None;
        state.master_number = None;
    }
}

impl Drop for LinkHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn validate_config(config: &LinkConfig) -> Result<(), LinkError> {
    if config.interface_ip.is_unspecified()
        || config.interface_ip.is_multicast()
        || config.interface_ip.is_broadcast()
    {
        return Err(LinkError::InvalidConfig(
            "select a local IPv4 interface before enabling Link".into(),
        ));
    }
    if config.broadcast_ip.is_unspecified() || config.broadcast_ip.is_multicast() {
        return Err(LinkError::InvalidConfig(
            "broadcast address must be IPv4 broadcast or a loopback test address".into(),
        ));
    }
    if config.mac_address == [0; 6] || config.mac_address[0] & 1 != 0 {
        return Err(LinkError::InvalidConfig(
            "the selected interface requires its unicast MAC address".into(),
        ));
    }
    if !matches!(config.source_deck.as_str(), "A" | "B") {
        return Err(LinkError::InvalidConfig(
            "source_deck must be A or B".into(),
        ));
    }
    if !config.latency_ms.is_finite() || !(0.0..=1000.0).contains(&config.latency_ms) {
        return Err(LinkError::InvalidConfig(
            "latency_ms must be between 0 and 1000".into(),
        ));
    }
    if config
        .preferred_player
        .is_some_and(|number| !(1..=4).contains(&number))
    {
        return Err(LinkError::InvalidConfig(
            "preferred player must be between 1 and 4".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Phase {
    Observe,
    Initial(u8),
    Claim1(u8),
    Claim2(u8),
    Final(u8),
    Active,
}

struct Worker {
    config: LinkConfig,
    ports: Ports,
    discovery: UdpSocket,
    beat: UdpSocket,
    status: UdpSocket,
    state: Arc<RwLock<LinkStatus>>,
    library: Arc<RwLock<LibrarySnapshot>>,
    clock: Arc<RwLock<(LocalClock, Instant)>>,
    stop: Arc<AtomicBool>,
    events: SyncSender<LinkEvent>,
    commands: Receiver<Command>,
    peers: BTreeMap<u8, LinkDevice>,
    library_greeted: HashSet<u8>,
    seen: HashMap<u8, Instant>,
    excluded: HashMap<u8, Instant>,
    phase: Phase,
    phase_at: Instant,
    identity: WireIdentity,
    master: Option<u8>,
    handoff: Option<u8>,
    requesting_master: Option<Instant>,
    sync_counter: u32,
    first: bool,
    last_keepalive: Instant,
    last_status: Instant,
    last_beat: Option<u32>,
    packet_counter: u32,
    library_ready_at: Instant,
}

impl Worker {
    fn run(mut self) {
        while !self.stop.load(Ordering::Acquire) {
            self.receive();
            self.expire();
            while let Ok(Command::Master) = self.commands.try_recv() {
                self.become_master();
            }
            self.advance_claim();
            self.transmit();
            {
                let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
                state.player_number = if matches!(self.phase, Phase::Active) {
                    Some(self.identity.number)
                } else {
                    None
                };
                state.master_number = self.master;
                state.devices = self.peers.values().cloned().collect();
            }
            thread::sleep(Duration::from_millis(3));
        }
        self.state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .running = false;
    }

    fn emit(&self, event: LinkEvent) {
        let _ = self.events.try_send(event);
    }

    fn error(&self, message: impl Into<String>) {
        let message = message.into();
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        if state.last_error.as_ref() != Some(&message) {
            self.emit(LinkEvent::Error {
                message: message.clone(),
            });
            state.last_error = Some(message);
        }
    }

    fn send(&self, socket: &UdpSocket, packet: &[u8], target: SocketAddrV4) {
        if let Err(error) = socket.send_to(packet, target) {
            self.error(format!("send to {target}: {error}"));
        }
    }

    fn announce(&self, packet: &[u8]) {
        self.send(
            &self.discovery,
            packet,
            SocketAddrV4::new(self.config.broadcast_ip, self.ports.discovery),
        );
    }

    fn source_identity(&self) -> WireIdentity {
        WireIdentity {
            number: SOURCE_NUMBER,
            name: "rekordbox".into(),
            ip: self.config.interface_ip,
            mac: self.config.mac_address,
        }
    }

    fn receive(&mut self) {
        let mut buffer = [0_u8; 2048];
        for channel in 0..3 {
            // Cap work per socket so a noisy peer cannot starve the clock.
            for _ in 0..128 {
                let result = match channel {
                    0 => self.discovery.recv_from(&mut buffer),
                    1 => self.beat.recv_from(&mut buffer),
                    _ => self.status.recv_from(&mut buffer),
                };
                let (size, from) = match result {
                    Ok(value) => value,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => {
                        self.error(format!("receive: {error}"));
                        break;
                    }
                };
                let SocketAddr::V4(from) = from else { continue };
                if *from.ip() == self.config.interface_ip {
                    continue;
                }
                let packet = &buffer[..size];
                match channel {
                    0 => self.discovery_packet(packet, from),
                    1 => self.beat_packet(packet, from),
                    _ => self.status_packet(packet, from),
                }
            }
        }
    }

    fn discovery_packet(&mut self, packet: &[u8], from: SocketAddrV4) {
        let is_final_claim = packet.get(0x0a) == Some(&4);
        let Some(packet) = parse_discovery(packet) else {
            return;
        };
        match packet {
            DiscoveryPacket::Keepalive {
                number,
                name,
                ip,
                mac: _,
                kind,
            } => {
                if ip != *from.ip() || number == 0 {
                    return;
                }
                if number == SOURCE_NUMBER && self.config.library_enabled {
                    self.error("Library source 17 is already occupied; Link stopped to avoid a rekordbox collision");
                    self.stop.store(true, Ordering::Release);
                    return;
                }
                if number == self.identity.number
                    && !matches!(
                        self.phase,
                        Phase::Observe | Phase::Initial(_) | Phase::Claim1(_)
                    )
                {
                    self.relinquish(number);
                }
                self.seen.insert(number, Instant::now());
                if self.peers.get(&number).is_some_and(|peer| peer.ip != ip) {
                    self.library_greeted.remove(&number);
                }
                let device = self.peers.entry(number).or_insert(LinkDevice {
                    device_number: number,
                    name: name.clone(),
                    ip,
                    kind,
                    bpm: None,
                    playing: false,
                    synced: false,
                    master: false,
                    beat: 0,
                    track_id: None,
                    last_seen_micros: epoch_micros(),
                });
                device.name = name;
                device.ip = ip;
                device.kind = kind;
                device.last_seen_micros = epoch_micros();
            }
            DiscoveryPacket::Claim { number } => {
                self.excluded.insert(number, Instant::now());
                if number == self.identity.number
                    && matches!(
                        self.phase,
                        Phase::Claim2(_) | Phase::Final(_) | Phase::Active
                    )
                {
                    self.relinquish(number);
                } else if is_final_claim && matches!(self.phase, Phase::Active) {
                    self.send(
                        &self.discovery,
                        &encode_assignment_finished(&self.identity),
                        SocketAddrV4::new(*from.ip(), self.ports.discovery),
                    );
                }
            }
            DiscoveryPacket::Conflict { number } => {
                if number == self.identity.number {
                    self.relinquish(number);
                }
            }
            DiscoveryPacket::Assignment { number } => {
                if (1..=4).contains(&number)
                    && !self.peers.contains_key(&number)
                    && !self.excluded.contains_key(&number)
                    && !matches!(self.phase, Phase::Active)
                {
                    self.identity.number = number;
                    self.phase = Phase::Final(1);
                    self.phase_at = Instant::now();
                }
            }
            DiscoveryPacket::AssignmentFinished => {
                if matches!(self.phase, Phase::Final(_)) {
                    self.activate();
                }
            }
            DiscoveryPacket::AssignIntent => {
                if !matches!(self.phase, Phase::Active) {
                    // A mixer on a dedicated channel port first asks us to
                    // request an assignment. Use the documented unicast
                    // request variant and accept only an unoccupied 1–4 slot.
                    let mut unassigned = self.identity.clone();
                    unassigned.number = 0;
                    let mut request = encode_claim2(&unassigned, 1);
                    request[0x0b] = 1;
                    self.send(
                        &self.discovery,
                        &request,
                        SocketAddrV4::new(*from.ip(), self.ports.discovery),
                    );
                }
            }
        }
    }

    fn beat_packet(&mut self, packet: &[u8], from: SocketAddrV4) {
        if let Some(beat) = parse_beat(packet) {
            if self
                .peers
                .get(&beat.number)
                .is_none_or(|peer| peer.ip != *from.ip())
            {
                return;
            }
            self.emit(LinkEvent::Beat {
                device_number: beat.number,
                bpm: beat.bpm,
                beat: beat.beat,
                received_at_micros: epoch_micros(),
            });
        }
        let Some(control) = parse_control(packet) else {
            return;
        };
        if !self.peers.values().any(|peer| peer.ip == *from.ip()) {
            return;
        }
        match control {
            ControlPacket::Sync(enabled) => self.emit(LinkEvent::SyncCommand { enabled }),
            ControlPacket::BecomeMaster => self.become_master(),
            ControlPacket::MasterRequest { number } => {
                if self.master == Some(self.identity.number)
                    && self
                        .peers
                        .get(&number)
                        .is_some_and(|peer| peer.ip == *from.ip())
                {
                    self.handoff = Some(number);
                    self.send(
                        &self.beat,
                        &encode_master_response(&self.identity),
                        SocketAddrV4::new(*from.ip(), self.ports.beat),
                    );
                }
            }
            ControlPacket::MasterResponse { number, accepted } => {
                if accepted && self.master == Some(number) && self.requesting_master.is_some() {
                    // Confirmation alone does not complete handoff; wait for
                    // our player number in the outgoing master's status.
                    self.requesting_master = Some(Instant::now());
                }
            }
        }
    }

    fn status_packet(&mut self, packet: &[u8], from: SocketAddrV4) {
        let known_sender = packet.get(0x21).is_some_and(|number| {
            self.peers
                .get(number)
                .is_some_and(|peer| peer.ip == *from.ip())
        });
        let valid_library_header = self.config.library_enabled
            && known_sender
            && packet.get(..10) == Some(b"Qspt1WmJOL")
            && packet.get(0x1f) == Some(&1);
        if valid_library_header
            && packet.len() == 0x24
            && packet[0x0a] == 0x10
            && packet[0x20] == 0
            && packet[0x22..0x24] == [0, 0]
        {
            self.greet_library_peer(*from.ip());
            return;
        }
        if valid_library_header
            && packet.len() == 0x30
            && packet[0x0a] == 0x05
            && packet[0x28..0x2c] == [0, 0, 0, SOURCE_NUMBER]
            && packet[0x2c..0x30] == [0, 0, 0, 4]
        {
            self.send_library_media(*from.ip());
            return;
        }
        let Some(status) = parse_status(packet) else {
            return;
        };
        let Some(peer) = self.peers.get_mut(&status.number) else {
            return;
        };
        if peer.ip != *from.ip() {
            return;
        }
        peer.bpm = status.bpm;
        peer.playing = status.playing;
        peer.synced = status.synced;
        peer.master = status.master;
        peer.beat = status.beat;
        peer.track_id = status.track_id;
        peer.last_seen_micros = epoch_micros();
        self.seen.insert(status.number, Instant::now());
        let device = peer.clone();
        self.sync_counter = self.sync_counter.max(status.sync_counter);
        if status.master {
            if status.handoff == Some(self.identity.number) && matches!(self.phase, Phase::Active) {
                self.set_master(Some(self.identity.number));
                self.requesting_master = None;
            } else if self.handoff == Some(status.number) {
                self.handoff = None;
                self.sync_counter = self.sync_counter.wrapping_add(1);
                self.set_master(Some(status.number));
            } else if self.master != Some(self.identity.number) {
                self.set_master(Some(status.number));
            }
        } else if self.master == Some(status.number) {
            self.set_master(None);
        }
        if self.master == Some(self.identity.number)
            && !self
                .clock
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .0
                .playing
            && status.playing
            && status.synced
        {
            self.handoff = Some(status.number);
        }
        self.emit(LinkEvent::Status { device });
    }

    fn set_master(&mut self, number: Option<u8>) {
        if self.master != number {
            self.master = number;
            self.emit(LinkEvent::MasterChanged {
                device_number: number,
            });
        }
    }

    fn become_master(&mut self) {
        if !matches!(self.phase, Phase::Active) {
            self.error("Wait for an available player number before requesting master");
            return;
        }
        if let Some(master) = self.master {
            if master == self.identity.number {
                return;
            }
            if let Some(peer) = self.peers.get(&master) {
                self.send(
                    &self.beat,
                    &encode_master_request(&self.identity),
                    SocketAddrV4::new(peer.ip, self.ports.beat),
                );
                self.requesting_master = Some(Instant::now());
            }
        } else {
            self.set_master(Some(self.identity.number));
        }
    }

    fn relinquish(&mut self, number: u8) {
        self.error(format!(
            "Player {number} is occupied; searching for another free player number"
        ));
        self.excluded.insert(number, Instant::now());
        if self.master == Some(number) {
            self.set_master(None);
        }
        self.identity.number = 0;
        self.handoff = None;
        self.phase = Phase::Observe;
        self.phase_at = Instant::now() + Duration::from_secs(3);
    }

    fn expire(&mut self) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .seen
            .iter()
            .filter_map(|(&number, &at)| {
                (now.duration_since(at) > DEVICE_TIMEOUT).then_some(number)
            })
            .collect();
        for number in expired {
            self.peers.remove(&number);
            self.seen.remove(&number);
            self.library_greeted.remove(&number);
            if self.master == Some(number) {
                self.set_master(None);
                self.error("Pro DJ Link sync source disconnected");
            }
        }
        self.excluded
            .retain(|_, at| now.duration_since(*at) < DEVICE_TIMEOUT);
        if self
            .requesting_master
            .is_some_and(|at| now.duration_since(at) > Duration::from_secs(3))
        {
            self.requesting_master = None;
            self.error("Tempo master handoff timed out");
        }
    }

    fn advance_claim(&mut self) {
        if Instant::now() < self.phase_at {
            return;
        }
        self.phase_at = Instant::now() + Duration::from_millis(300);
        match self.phase {
            Phase::Observe => {
                self.first = self.peers.is_empty();
                self.announce(&encode_initial(&self.identity));
                self.phase = Phase::Initial(2);
            }
            Phase::Initial(count) => {
                self.announce(&encode_initial(&self.identity));
                self.phase = if count == 3 {
                    Phase::Claim1(1)
                } else {
                    Phase::Initial(count + 1)
                };
            }
            Phase::Claim1(count) => {
                self.announce(&encode_claim1(&self.identity, count));
                if count == 3 {
                    if let Some(number) = choose_player(
                        self.config.preferred_player,
                        self.peers
                            .keys()
                            .copied()
                            .chain(self.excluded.keys().copied()),
                    ) {
                        self.identity.number = number;
                        self.phase = Phase::Claim2(1);
                    } else {
                        self.error("No free Pro DJ Link player number (1–4)");
                        self.phase = Phase::Observe;
                        self.phase_at = Instant::now() + Duration::from_secs(3);
                    }
                } else {
                    self.phase = Phase::Claim1(count + 1);
                }
            }
            Phase::Claim2(count) => {
                self.announce(&encode_claim2(&self.identity, count));
                self.phase = if count == 3 {
                    Phase::Final(1)
                } else {
                    Phase::Claim2(count + 1)
                };
            }
            Phase::Final(count) => {
                self.announce(&encode_claim_final(&self.identity, count));
                if count == 3 {
                    self.activate();
                } else {
                    self.phase = Phase::Final(count + 1);
                }
            }
            Phase::Active => {}
        }
    }

    fn activate(&mut self) {
        self.phase = Phase::Active;
        self.state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .last_error = None;
        self.last_keepalive = Instant::now() - Duration::from_secs(2);
    }

    fn send_library_media(&self, ip: Ipv4Addr) {
        let catalog = self.library.read().unwrap_or_else(|e| e.into_inner());
        self.send(
            &self.status,
            &encode_library_media(
                &self.source_identity(),
                catalog.tracks.len(),
                catalog.playlists.len(),
            ),
            SocketAddrV4::new(ip, self.ports.status),
        );
    }

    fn greet_library_peer(&self, ip: Ipv4Addr) {
        let source = self.source_identity();
        let destination = SocketAddrV4::new(ip, self.ports.status);
        self.send(&self.status, &encode_library_hello(&source), destination);
        // Explicit per-source display preferences: full overview, legacy blue
        // waveform, classic key notation, centred waveform. The activation
        // protocol has no documented preserve/neutral preference values.
        self.send(
            &self.status,
            &encode_library_activation(&source, [1, 2, 1, 1, 1, 1]),
            destination,
        );
        self.send_library_media(ip);
    }

    fn transmit(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_keepalive) >= Duration::from_secs(2) {
            self.last_keepalive = now;
            let count =
                (self.peers.len() + 1 + usize::from(self.config.library_enabled)).min(255) as u8;
            if self.config.library_enabled && now >= self.library_ready_at {
                self.announce(&encode_keepalive(
                    &self.source_identity(),
                    count,
                    self.first,
                    true,
                ));
                for peer in self
                    .peers
                    .values()
                    .filter(|peer| (1..=6).contains(&peer.device_number))
                {
                    if self.library_greeted.insert(peer.device_number) {
                        self.greet_library_peer(peer.ip);
                    } else {
                        self.send_library_media(peer.ip);
                    }
                }
            }
            if matches!(self.phase, Phase::Active) {
                self.announce(&encode_keepalive(&self.identity, count, self.first, false));
            }
        }
        let status_due = now.duration_since(self.last_status) >= Duration::from_millis(200);
        if status_due {
            self.last_status = now;
            if self.config.library_enabled && now >= self.library_ready_at {
                let packet = encode_library_status(&self.source_identity());
                for peer in self
                    .peers
                    .values()
                    .filter(|peer| self.library_greeted.contains(&peer.device_number))
                {
                    self.send(
                        &self.status,
                        &packet,
                        SocketAddrV4::new(peer.ip, self.ports.status),
                    );
                }
            }
        }
        if !matches!(self.phase, Phase::Active) {
            return;
        }
        let (mut clock, at) = self.clock.read().unwrap_or_else(|e| e.into_inner()).clone();
        let age = now.duration_since(at);
        if age > Duration::from_millis(250) {
            clock.playing = false;
        }
        if status_due {
            self.packet_counter = self.packet_counter.wrapping_add(1);
            let packet = encode_status(
                &self.identity,
                &clock,
                self.master == Some(self.identity.number),
                self.handoff,
                self.sync_counter,
                self.packet_counter,
            );
            for peer in self.peers.values() {
                self.send(
                    &self.status,
                    &packet,
                    SocketAddrV4::new(peer.ip, self.ports.status),
                );
            }
        }
        if !clock.playing || !clock.grid_available || clock.bpm <= 0.0 || clock.beat == 0 {
            self.last_beat = None;
            return;
        }
        // Extrapolate only a fresh audio-clock sample, delaying outgoing beat
        // boundaries by the configured device output latency.
        let position = (clock.beat as f64
            + clock.beat_phase.clamp(0.0, 0.999999)
            + (age.as_secs_f64() - self.config.latency_ms / 1000.0) * clock.bpm / 60.0)
            .max(1.0);
        let beat = position.floor() as u32;
        if self
            .last_beat
            .is_some_and(|previous| beat == previous.saturating_add(1))
        {
            clock.beat = beat;
            clock.beat_phase = 0.0;
            self.send(
                &self.beat,
                &encode_beat(&self.identity, &clock),
                SocketAddrV4::new(self.config.broadcast_ip, self.ports.beat),
            );
        }
        self.last_beat = Some(beat);
    }
}

fn choose_player(preferred: Option<u8>, occupied: impl IntoIterator<Item = u8>) -> Option<u8> {
    let mut unavailable = [false; 5];
    for number in occupied {
        if number <= 4 {
            unavailable[number as usize] = true;
        }
    }
    preferred
        .filter(|number| (1..=4).contains(number) && !unavailable[*number as usize])
        .or_else(|| (1..=4).rev().find(|number| !unavailable[*number as usize]))
}

fn epoch_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker() -> Worker {
        let socket = || UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let discovery = socket();
        let beat = socket();
        let status = socket();
        let (_, commands) = mpsc::sync_channel(4);
        let (events, _) = mpsc::sync_channel(32);
        let now = Instant::now();
        Worker {
            config: LinkConfig {
                interface_ip: Ipv4Addr::LOCALHOST,
                broadcast_ip: Ipv4Addr::LOCALHOST,
                mac_address: [2, 0, 0, 0, 0, 1],
                ..LinkConfig::default()
            },
            ports: Ports {
                discovery: discovery.local_addr().unwrap().port(),
                beat: beat.local_addr().unwrap().port(),
                status: status.local_addr().unwrap().port(),
                query: 0,
                database: 0,
                nfs: 0,
            },
            discovery,
            beat,
            status,
            state: Arc::new(RwLock::new(LinkStatus::default())),
            library: Arc::new(RwLock::new(LibrarySnapshot::default())),
            clock: Arc::new(RwLock::new((
                LocalClock {
                    playing: true,
                    ..LocalClock::default()
                },
                now,
            ))),
            stop: Arc::new(AtomicBool::new(false)),
            events,
            commands,
            peers: BTreeMap::new(),
            library_greeted: HashSet::new(),
            seen: HashMap::new(),
            excluded: HashMap::new(),
            phase: Phase::Active,
            phase_at: now,
            identity: WireIdentity {
                number: 4,
                name: "Conduction".into(),
                ip: Ipv4Addr::LOCALHOST,
                mac: [2, 0, 0, 0, 0, 1],
            },
            master: None,
            handoff: None,
            requesting_master: None,
            sync_counter: 0,
            first: true,
            last_keepalive: now,
            last_status: now,
            last_beat: None,
            packet_counter: 0,
            library_ready_at: now,
        }
    }

    fn peer(number: u8) -> WireIdentity {
        WireIdentity {
            number,
            name: "CDJ-3000".into(),
            ip: Ipv4Addr::new(127, 0, 0, 2),
            mac: [2, 0, 0, 0, 0, 2],
        }
    }

    #[test]
    fn active_collision_abandons_player_without_defending_it() {
        let mut worker = worker();
        let remote = peer(4);
        worker.master = Some(4);
        worker.discovery_packet(
            &encode_keepalive(&remote, 2, false, false),
            SocketAddrV4::new(remote.ip, 0),
        );
        assert_eq!(worker.identity.number, 0);
        assert!(matches!(worker.phase, Phase::Observe));
        assert_eq!(worker.master, None);
        assert!(worker.excluded.contains_key(&4));
        assert!(worker.peers.contains_key(&4));
    }

    #[test]
    fn source_collision_stops_publication() {
        let mut worker = worker();
        worker.config.library_enabled = true;
        let remote = peer(SOURCE_NUMBER);
        worker.discovery_packet(
            &encode_keepalive(&remote, 2, false, true),
            SocketAddrV4::new(remote.ip, 0),
        );
        assert!(worker.stop.load(Ordering::Acquire));
        assert!(worker
            .state
            .read()
            .unwrap()
            .last_error
            .as_ref()
            .unwrap()
            .contains("source 17"));
    }

    #[test]
    fn master_handoff_requires_outgoing_master_status_and_can_yield_again() {
        let mut worker = worker();
        let remote = peer(1);
        let from = SocketAddrV4::new(remote.ip, 0);
        worker.discovery_packet(&encode_keepalive(&remote, 2, false, false), from);
        let clock = LocalClock {
            playing: true,
            synced: true,
            grid_available: true,
            bpm: 128.0,
            beat: 5,
            ..LocalClock::default()
        };
        worker.status_packet(&encode_status(&remote, &clock, true, None, 5, 1), from);
        assert_eq!(worker.master, Some(1));
        worker.become_master();
        assert_eq!(worker.master, Some(1));
        worker.beat_packet(&encode_master_response(&remote), from);
        assert_eq!(worker.master, Some(1));
        worker.status_packet(&encode_status(&remote, &clock, true, Some(4), 5, 2), from);
        assert_eq!(worker.master, Some(4));
        worker.beat_packet(&encode_master_request(&remote), from);
        assert_eq!(worker.handoff, Some(1));
        worker.status_packet(&encode_status(&remote, &clock, true, None, 5, 3), from);
        assert_eq!(worker.master, Some(1));
        assert_eq!(worker.handoff, None);
        assert_eq!(worker.sync_counter, 6);
    }

    #[test]
    fn remote_master_timeout_does_not_change_local_clock() {
        let mut worker = worker();
        let remote = peer(1);
        worker.discovery_packet(
            &encode_keepalive(&remote, 2, false, false),
            SocketAddrV4::new(remote.ip, 0),
        );
        worker.master = Some(1);
        worker
            .seen
            .insert(1, Instant::now() - DEVICE_TIMEOUT - Duration::from_secs(1));
        worker.expire();
        assert_eq!(worker.master, None);
        assert!(worker.peers.is_empty());
        assert!(worker.clock.read().unwrap().0.playing);
    }

    #[test]
    fn library_source_and_virtual_player_announce_together() {
        let mut worker = worker();
        worker.config.library_enabled = true;
        worker.last_keepalive = Instant::now() - Duration::from_secs(3);
        worker
            .discovery
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        worker.transmit();
        let mut packet = [0; 2048];
        let mut numbers = Vec::new();
        for _ in 0..2 {
            let (size, _) = worker.discovery.recv_from(&mut packet).unwrap();
            let DiscoveryPacket::Keepalive { number, .. } =
                parse_discovery(&packet[..size]).unwrap()
            else {
                panic!("expected a keepalive")
            };
            numbers.push(number);
        }
        numbers.sort();
        assert_eq!(numbers, [4, SOURCE_NUMBER]);
    }

    #[test]
    fn disabled_is_inert_and_does_not_require_network_configuration() {
        let handle = LinkHandle::start(LinkConfig::default(), LibrarySnapshot::default()).unwrap();
        assert!(!handle.snapshot().running);
        assert!(!handle.snapshot().hardware_verified);
        assert_eq!(handle.snapshot().source_number, 17);
    }

    #[test]
    fn player_selection_never_steals_occupied_number() {
        assert_eq!(choose_player(None, [1, 2]), Some(4));
        assert_eq!(choose_player(Some(2), [1, 2, 4]), Some(3));
        assert_eq!(choose_player(None, [1, 2, 3, 4]), None);
        assert_eq!(choose_player(None, [5, 6, 17, 33]), Some(4));
    }

    #[test]
    fn validates_interface_latency_and_nxs2_player_range() {
        let mut config = LinkConfig {
            enabled: true,
            ..LinkConfig::default()
        };
        assert!(validate_config(&config).is_err());
        config.interface_ip = Ipv4Addr::LOCALHOST;
        config.mac_address = [2, 0, 0, 0, 0, 1];
        assert!(validate_config(&config).is_ok());
        config.preferred_player = Some(5);
        assert!(validate_config(&config).is_err());
        config.preferred_player = None;
        config.latency_ms = f64::NAN;
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn loopback_start_stop_uses_only_ephemeral_ports() {
        let config = LinkConfig {
            enabled: true,
            interface_ip: Ipv4Addr::LOCALHOST,
            broadcast_ip: Ipv4Addr::LOCALHOST,
            mac_address: [2, 0, 0, 0, 0, 1],
            ..LinkConfig::default()
        };
        let ports = Ports {
            discovery: 0,
            beat: 0,
            status: 0,
            query: 0,
            database: 0,
            nfs: 0,
        };
        let handle =
            LinkHandle::start_with_ports(config, LibrarySnapshot::default(), ports).unwrap();
        assert!(handle.snapshot().running);
        handle.stop();
        assert!(!handle.snapshot().running);
    }

    #[test]
    fn loopback_library_and_virtual_player_services_share_lifecycle() {
        let config = LinkConfig {
            enabled: true,
            library_enabled: true,
            interface_ip: Ipv4Addr::LOCALHOST,
            broadcast_ip: Ipv4Addr::LOCALHOST,
            mac_address: [2, 0, 0, 0, 0, 1],
            ..LinkConfig::default()
        };
        let ports = Ports {
            discovery: 0,
            beat: 0,
            status: 0,
            query: 0,
            database: 0,
            nfs: 0,
        };
        let library = LibrarySnapshot {
            tracks: vec![crate::LinkTrack {
                id: 7,
                ..crate::LinkTrack::default()
            }],
            ..LibrarySnapshot::default()
        };
        let handle = LinkHandle::start_with_ports(config, library, ports).unwrap();
        assert!(handle.snapshot().running);
        assert_eq!(handle.snapshot().library_tracks, 1);
        assert_eq!(handle.threads.lock().unwrap().len(), 4);
        handle.stop();
        assert!(!handle.snapshot().running);
        assert!(handle.threads.lock().unwrap().is_empty());
    }
}

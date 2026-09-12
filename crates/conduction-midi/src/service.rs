use crate::{
    decoder::led_messages, profiles, validate_bindings, ControllerAction, ControllerState,
    LedControl, MidiConfig, MidiConnectionStatus, MidiDecoder, MidiDevices, MidiPortInfo,
    MidiProfile, MidiStatus,
};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MidiError {
    #[error("MIDI configuration: {0}")]
    Configuration(String),
    #[error("MIDI driver: {0}")]
    Driver(String),
    #[error("MIDI worker is unavailable")]
    Unavailable,
    #[error("MIDI driver operation timed out")]
    Timeout,
}

enum Command {
    Devices(mpsc::Sender<Result<MidiDevices, MidiError>>),
    Configure(MidiConfig, mpsc::Sender<Result<(), MidiError>>),
    Disconnect(Option<String>, mpsc::Sender<Result<(), MidiError>>),
    Shutdown,
}

struct RawInput {
    connection: u64,
    data: [u8; 3],
}

struct Shared {
    state: Mutex<ControllerState>,
    state_generation: AtomicU64,
    status: Mutex<MidiStatus>,
    dropped: AtomicU64,
}

struct Inner {
    tx: SyncSender<Command>,
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}

/// Owns native MIDI connections on one worker. Creating it does not open ports.
/// Configure adds/replaces one input; disconnect(None) disconnects every input.
#[derive(Clone)]
pub struct MidiService {
    inner: Arc<Inner>,
}

impl MidiService {
    pub fn new(callback: impl Fn(ControllerAction) + Send + Sync + 'static) -> Self {
        let (tx, rx) = mpsc::sync_channel(32);
        let shared = Arc::new(Shared {
            state: Mutex::new(ControllerState::default()),
            state_generation: AtomicU64::new(0),
            status: Mutex::new(MidiStatus::default()),
            dropped: AtomicU64::new(0),
        });
        let worker_shared = shared.clone();
        let worker = thread::spawn(move || run(rx, worker_shared, callback));
        Self {
            inner: Arc::new(Inner {
                tx,
                shared,
                worker: Mutex::new(Some(worker)),
            }),
        }
    }

    pub fn profiles() -> Vec<MidiProfile> {
        profiles::builtin_profiles()
    }

    pub fn devices(&self) -> Result<MidiDevices, MidiError> {
        let (tx, rx) = mpsc::channel();
        self.inner
            .tx
            .send(Command::Devices(tx))
            .map_err(|_| MidiError::Unavailable)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| MidiError::Timeout)?
    }

    pub fn configure(&self, config: MidiConfig) -> Result<(), MidiError> {
        let (tx, rx) = mpsc::channel();
        self.inner
            .tx
            .send(Command::Configure(config, tx))
            .map_err(|_| MidiError::Unavailable)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| MidiError::Timeout)?
    }

    pub fn disconnect(&self, input_port: Option<String>) -> Result<(), MidiError> {
        let (tx, rx) = mpsc::channel();
        self.inner
            .tx
            .send(Command::Disconnect(input_port, tx))
            .map_err(|_| MidiError::Unavailable)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| MidiError::Timeout)?
    }

    /// Replaces pending state rather than queueing 50Hz updates behind I/O work.
    pub fn update_state(&self, state: ControllerState) -> Result<(), MidiError> {
        *self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| MidiError::Unavailable)? = state;
        self.inner
            .shared
            .state_generation
            .fetch_add(1, Ordering::Release);
        Ok(())
    }

    pub fn status(&self) -> MidiStatus {
        let mut status = self
            .inner
            .shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        status.dropped_messages = self.inner.shared.dropped.load(Ordering::Relaxed);
        status
    }
}

struct Connection {
    id: u64,
    config: MidiConfig,
    profile: MidiProfile,
    input_id: String,
    output_id: Option<String>,
    input_name: String,
    input: Option<MidiInputConnection<()>>,
    output: Option<MidiOutputConnection>,
    decoder: MidiDecoder,
    previous_leds: Vec<Vec<u8>>,
    error: Option<String>,
}

impl Connection {
    fn release(&mut self, callback: &impl Fn(ControllerAction)) {
        // Drop the input first so no further hardware events can enter the queue.
        self.input.take();
        self.output.take();
        for action in self.decoder.release_held() {
            callback(action);
        }
    }

    fn state(&mut self, state: &ControllerState) {
        self.decoder.update_state(state);
        if let Some(output) = &mut self.output {
            let leds = led_messages(&self.profile.leds, state);
            for (index, message) in leds.iter().enumerate() {
                if self.previous_leds.get(index) != Some(message) {
                    if let Err(error) = output.send(message) {
                        self.error = Some(format!("MIDI LED output failed: {error}"));
                        return;
                    }
                }
            }
            self.previous_leds = leds;
        }
    }
}

fn run(rx: Receiver<Command>, shared: Arc<Shared>, callback: impl Fn(ControllerAction)) {
    let (raw_tx, raw_rx) = mpsc::sync_channel::<RawInput>(2048);
    let mut connections = HashMap::<String, Connection>::new();
    let mut next_id = 0u64;
    let mut generation = 0;
    let mut checked = Instant::now();
    let mut dropped = 0;
    loop {
        match rx.recv_timeout(Duration::from_millis(2)) {
            Ok(Command::Devices(reply)) => {
                let devices = enumerate();
                apply_device_snapshot(&mut connections, &devices, &shared, &callback);
                publish(&connections, &shared);
                let _ = reply.send(devices);
            }
            Ok(Command::Configure(config, reply)) => {
                next_id = next_id.wrapping_add(1);
                let result = configure(
                    config,
                    next_id,
                    &raw_tx,
                    &shared,
                    &mut connections,
                    &callback,
                );
                publish(&connections, &shared);
                let _ = reply.send(result);
            }
            Ok(Command::Disconnect(port, reply)) => {
                let ports: Vec<_> = connections
                    .values()
                    .map(|c| MidiPortInfo {
                        id: c.input_id.clone(),
                        name: c.input_name.clone(),
                    })
                    .collect();
                let result = match port {
                    Some(port) => {
                        port_index(&ports, &port).map(|index| vec![ports[index].id.clone()])
                    }
                    None => Ok(ports.into_iter().map(|p| p.id).collect()),
                };
                let result = result.map(|keys| {
                    for key in keys {
                        if let Some(mut c) = connections.remove(&key) {
                            c.release(&callback);
                        }
                    }
                });
                publish(&connections, &shared);
                let _ = reply.send(result);
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for raw in raw_rx.try_iter().take(256) {
            if let Some(c) = connections
                .values_mut()
                .find(|c| c.id == raw.connection && c.input.is_some())
            {
                for action in c.decoder.process(&raw.data) {
                    callback(action);
                }
            }
        }
        let current_dropped = shared.dropped.load(Ordering::Relaxed);
        if current_dropped != dropped {
            // A dropped NoteOff must never leave a held Cue/jog/nudge latched.
            while raw_rx.try_recv().is_ok() {}
            for c in connections.values_mut() {
                for action in c.decoder.release_held() {
                    callback(action);
                }
                c.error = Some("MIDI input overflow; held controls were released".into());
            }
            dropped = current_dropped;
            publish(&connections, &shared);
        }
        let next_generation = shared.state_generation.load(Ordering::Acquire);
        if generation != next_generation {
            let state = shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            for c in connections.values_mut() {
                if c.input.is_some() {
                    c.state(&state);
                }
            }
            generation = next_generation;
        }
        if checked.elapsed() >= Duration::from_secs(1) && !connections.is_empty() {
            checked = Instant::now();
            let devices = enumerate();
            apply_device_snapshot(&mut connections, &devices, &shared, &callback);
            publish(&connections, &shared);
        }
    }
    for c in connections.values_mut() {
        c.release(&callback);
    }
}

fn endpoints_present(devices: &MidiDevices, input_id: &str, output_id: Option<&str>) -> bool {
    devices.inputs.iter().any(|p| p.id == input_id)
        && output_id.is_none_or(|id| devices.outputs.iter().any(|p| p.id == id))
}

fn apply_device_snapshot(
    connections: &mut HashMap<String, Connection>,
    devices: &Result<MidiDevices, MidiError>,
    shared: &Shared,
    callback: &impl Fn(ControllerAction),
) {
    let error = match devices {
        Ok(devices) => {
            for c in connections.values_mut().filter(|c| c.input.is_some()) {
                if !endpoints_present(devices, &c.input_id, c.output_id.as_deref()) {
                    c.release(callback);
                    c.error = Some(
                        "MIDI device disconnected; reconnect explicitly to resume control".into(),
                    );
                }
            }
            None
        }
        Err(error) => {
            let message = format!(
                "MIDI device enumeration failed; held controls were released; reconnect explicitly to resume control: {error}"
            );
            // An unavailable driver cannot establish which endpoint survives.
            // Disconnect all native inputs before accepting another queued event.
            for c in connections.values_mut() {
                c.release(callback);
                c.error = Some(message.clone());
            }
            Some(message)
        }
    };
    shared
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .error = error;
}

fn enumerate() -> Result<MidiDevices, MidiError> {
    let input = MidiInput::new("Conduction enumeration").map_err(driver)?;
    let output = MidiOutput::new("Conduction enumeration").map_err(driver)?;
    let inputs = input
        .ports()
        .iter()
        .map(|port| {
            let name = input.port_name(port).map_err(driver)?;
            Ok(MidiPortInfo {
                id: port.id(),
                name,
            })
        })
        .collect::<Result<_, MidiError>>()?;
    let outputs = output
        .ports()
        .iter()
        .map(|port| {
            let name = output.port_name(port).map_err(driver)?;
            Ok(MidiPortInfo {
                id: port.id(),
                name,
            })
        })
        .collect::<Result<_, MidiError>>()?;
    Ok(MidiDevices { inputs, outputs })
}

fn port_index(ports: &[MidiPortInfo], requested: &str) -> Result<usize, MidiError> {
    if let Some(index) = ports.iter().position(|p| p.id == requested) {
        return Ok(index);
    }
    // IDs are opaque. Never reinterpret a stale ID as an index or a name:
    // reconnecting a different same-name unit must require explicit selection.
    let indexes: Vec<_> = ports
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name == requested)
        .map(|(i, _)| i)
        .collect();
    match indexes.as_slice() {
        [index] => Ok(*index),
        [] => Err(MidiError::Configuration(format!(
            "MIDI port not found: {requested}"
        ))),
        _ => Err(MidiError::Configuration(format!(
            "Multiple MIDI ports named {requested}; choose its current port ID"
        ))),
    }
}

fn configure(
    mut config: MidiConfig,
    id: u64,
    raw_tx: &SyncSender<RawInput>,
    shared: &Arc<Shared>,
    connections: &mut HashMap<String, Connection>,
    callback: &impl Fn(ControllerAction),
) -> Result<(), MidiError> {
    let mut profile = profiles::builtin_profiles()
        .into_iter()
        .find(|p| p.id == config.profile)
        .ok_or_else(|| {
            MidiError::Configuration(format!("Unknown MIDI profile: {}", config.profile))
        })?;
    if !config.bindings.is_empty() {
        profile.bindings.clone_from(&config.bindings);
    }
    if let Some(deck) = &config.deck {
        if deck != "A" && deck != "B" {
            return Err(MidiError::Configuration("Deck must be A or B".into()));
        }
        if !profile.id.starts_with("cdj-") {
            return Err(MidiError::Configuration(
                "Deck selection is for single-player CDJ profiles".into(),
            ));
        }
        for binding in &mut profile.bindings {
            binding.control.remap_deck(deck);
        }
        for led in &mut profile.leds {
            let d = match &mut led.control {
                LedControl::Playing { deck }
                | LedControl::Cue { deck }
                | LedControl::Sync { deck }
                | LedControl::HeadphoneCue { deck }
                | LedControl::HotCue { deck, .. } => deck,
            };
            if d == "A" {
                *d = deck.clone();
            }
        }
    }
    validate_bindings(&profile.bindings).map_err(MidiError::Configuration)?;
    let mut input = MidiInput::new("Conduction controller").map_err(driver)?;
    input.ignore(Ignore::All);
    let input_ports = input.ports();
    let input_infos = input_ports
        .iter()
        .map(|p| {
            Ok(MidiPortInfo {
                id: p.id(),
                name: input.port_name(p).map_err(driver)?,
            })
        })
        .collect::<Result<Vec<_>, MidiError>>()?;
    let input_index = port_index(&input_infos, &config.input_port)?;
    let input_name = input_infos[input_index].name.clone();
    let input_id = input_infos[input_index].id.clone();
    let key = input_id.clone();
    let mut output_id = None;
    let output_request = if let Some(port) = &config.output_port {
        let output = MidiOutput::new("Conduction feedback").map_err(driver)?;
        let ports = output.ports();
        let infos = ports
            .iter()
            .map(|p| {
                Ok(MidiPortInfo {
                    id: p.id(),
                    name: output.port_name(p).map_err(driver)?,
                })
            })
            .collect::<Result<Vec<_>, MidiError>>()?;
        let index = port_index(&infos, port)?;
        output_id = Some(infos[index].id.clone());
        if connections
            .iter()
            .any(|(k, c)| k != &key && c.input.is_some() && c.output_id == output_id)
        {
            return Err(MidiError::Configuration(
                "MIDI output is already used by another connection".into(),
            ));
        }
        Some((output, ports[index].clone()))
    } else {
        None
    };
    config.input_port = input_id.clone();
    config.output_port = output_id.clone();
    // Validation and discovery finish before an existing connection is replaced.
    if let Some(mut old) = connections.remove(&key) {
        old.release(callback);
    }
    let output = output_request
        .map(|(output, port)| output.connect(&port, "Conduction LEDs").map_err(driver))
        .transpose()?;
    let sender = raw_tx.clone();
    let shared_callback = shared.clone();
    let input = input
        .connect(
            &input_ports[input_index],
            "Conduction controls",
            move |_, data, _| {
                if let Ok(data) = <[u8; 3]>::try_from(data) {
                    if sender
                        .try_send(RawInput {
                            connection: id,
                            data,
                        })
                        .is_err()
                    {
                        shared_callback.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            (),
        )
        .map_err(driver)?;
    let mut c = Connection {
        id,
        config,
        decoder: MidiDecoder::new(profile.bindings.clone()),
        profile,
        input_id,
        output_id,
        input_name,
        input: Some(input),
        output,
        previous_leds: vec![],
        error: None,
    };
    if shared.state_generation.load(Ordering::Acquire) > 0 {
        c.state(&shared.state.lock().unwrap_or_else(|e| e.into_inner()));
    }
    connections.insert(key, c);
    Ok(())
}

fn publish(connections: &HashMap<String, Connection>, shared: &Shared) {
    let mut entries: Vec<_> = connections
        .values()
        .map(|c| MidiConnectionStatus {
            config: c.config.clone(),
            profile_name: c.profile.name.clone(),
            connected: c.input.is_some(),
            error: c.error.clone(),
        })
        .collect();
    entries.sort_by(|a, b| a.config.input_port.cmp(&b.config.input_port));
    shared
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connections = entries;
}

fn driver(error: impl std::fmt::Display) -> MidiError {
    MidiError::Driver(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(id: &str, name: &str) -> MidiPortInfo {
        MidiPortInfo {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn opaque_ids_survive_enumeration_order_and_display_name_changes() {
        let ports = vec![
            port("endpoint:b", "CDJ renamed"),
            port("endpoint:a", "CDJ A"),
        ];
        assert_eq!(port_index(&ports, "endpoint:a").unwrap(), 1);
        assert_eq!(port_index(&ports, "endpoint:b").unwrap(), 0);
        assert_eq!(port_index(&ports, "CDJ A").unwrap(), 1);
        assert!(port_index(&ports, "7:CDJ A").is_err());
        assert!(port_index(&ports, "missing").is_err());
    }

    #[test]
    fn duplicate_names_require_exact_endpoint_ids() {
        let ports = vec![port("10001", "CDJ"), port("10002", "CDJ")];
        assert!(port_index(&ports, "CDJ").is_err());
        assert_eq!(port_index(&ports, "10002").unwrap(), 1);
        // A missing saved identity must not silently become the surviving CDJ.
        assert!(port_index(&ports[1..], "10001").is_err());
        assert_eq!(port_index(&ports[1..], "CDJ").unwrap(), 0);
    }

    #[test]
    fn exact_id_precedes_another_endpoints_display_name() {
        let ports = vec![port("input:a", "input:b"), port("input:b", "CDJ")];
        assert_eq!(port_index(&ports, "input:b").unwrap(), 1);
    }

    #[test]
    fn liveness_does_not_confuse_same_name_native_endpoints() {
        let devices = MidiDevices {
            inputs: vec![port("input:second", "CDJ")],
            outputs: vec![port("output:second", "CDJ")],
        };
        assert!(!endpoints_present(&devices, "input:first", None));
        assert!(!endpoints_present(
            &devices,
            "input:second",
            Some("output:first")
        ));
        assert!(endpoints_present(
            &devices,
            "input:second",
            Some("output:second")
        ));
    }

    #[test]
    fn driver_enumeration_failure_releases_held_actions_and_reports_error() {
        let shared = Shared {
            state: Mutex::new(ControllerState::default()),
            state_generation: AtomicU64::new(0),
            status: Mutex::new(MidiStatus::default()),
            dropped: AtomicU64::new(0),
        };
        let profile = profiles::builtin_profiles()
            .into_iter()
            .find(|p| p.id == "cdj-3000")
            .unwrap();
        let mut decoder = MidiDecoder::new(profile.bindings.clone());
        decoder.process(&[0x90, 0x01, 0x7f]);
        decoder.process(&[0x90, 0x17, 0x7f]);
        // No native handles are needed to exercise the fail-safe release path.
        let connection = Connection {
            id: 1,
            config: MidiConfig {
                input_port: "input:a".into(),
                output_port: None,
                profile: profile.id.clone(),
                deck: None,
                bindings: vec![],
            },
            profile,
            input_id: "input:a".into(),
            output_id: None,
            input_name: "CDJ".into(),
            input: None,
            output: None,
            decoder,
            previous_leds: vec![],
            error: None,
        };
        let mut connections = HashMap::from([("input:a".into(), connection)]);
        let actions = std::cell::RefCell::new(Vec::new());
        let callback = |action| actions.borrow_mut().push(action);
        apply_device_snapshot(
            &mut connections,
            &Err(MidiError::Driver("driver stopped".into())),
            &shared,
            &callback,
        );
        assert!(actions.borrow().contains(&ControllerAction::Cue {
            deck: "A".into(),
            pressed: false,
        }));
        assert!(actions.borrow().contains(&ControllerAction::JogTouch {
            deck: "A".into(),
            touched: false,
        }));
        let status = shared.status.lock().unwrap();
        assert!(status.error.as_ref().unwrap().contains("driver stopped"));
        drop(status);
        assert!(connections["input:a"].input.is_none());
        assert!(connections["input:a"].error.is_some());
        actions.borrow_mut().clear();
        apply_device_snapshot(
            &mut connections,
            &Ok(MidiDevices::default()),
            &shared,
            &callback,
        );
        assert!(actions.borrow().is_empty());
        assert!(shared.status.lock().unwrap().error.is_none());
        assert!(connections["input:a"].input.is_none());
    }
}

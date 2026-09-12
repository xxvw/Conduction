//! A deliberately read-only NFSv2 export of the published Link library.
//!
//! Wire formats are implemented from RFC 1057 (RPC/portmapper) and RFC 1094
//! (NFSv2/mount v1). All three RPC programs share the configured UDP port; this
//! does not register with, replace, or require the host's system rpcbind.
//! Clients see virtual paths, never host filesystem paths.
//! NXS captures additionally establish UTF-16LE path strings and a twelve-byte
//! preserved directory-handle prefix; see tests/fixtures/nfs-provenance.md.

use crate::{LibrarySnapshot, LinkTrack};
use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, UNIX_EPOCH};

const PMAP_PROGRAM: u32 = 100_000;
const MOUNT_PROGRAM: u32 = 100_005;
const NFS_PROGRAM: u32 = 100_003;
const MAX_READ: usize = 8192;
const MAX_REQUEST: usize = 16_384;
const HANDLE_SIZE: usize = 32;
const HANDLE_MAGIC: &[u8; 7] = b"CDTNFS2";
const FSID: u32 = 0x4344_5400;

const NFS_OK: u32 = 0;
const NFS_NOENT: u32 = 2;
const NFS_IO: u32 = 5;
const NFS_ACCES: u32 = 13;
const NFS_NOTDIR: u32 = 20;
const NFS_ISDIR: u32 = 21;
const NFS_FBIG: u32 = 27;
const NFS_ROFS: u32 = 30;
const NFS_STALE: u32 = 70;

/// Start the three read-only RPC programs on one UDP socket. Binding errors
/// are returned synchronously; the worker checks `stop` at least every 100 ms
/// while idle. A port of zero is useful for isolated loopback tests.
pub fn start_nfs(
    bind_ip: Ipv4Addr,
    port: u16,
    library: Arc<RwLock<LibrarySnapshot>>,
    stop: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    start_socket(UdpSocket::bind((bind_ip, port))?, library, stop)
}

pub(crate) fn start_socket(
    socket: UdpSocket,
    library: Arc<RwLock<LibrarySnapshot>>,
    stop: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let port = socket.local_addr()?.port();
    thread::Builder::new()
        .name("conduction-link-nfs".into())
        .spawn(move || {
            // One extra byte makes oversized/truncated UDP requests detectable.
            let mut packet = [0_u8; MAX_REQUEST + 1];
            while !stop.load(Ordering::Acquire) {
                match socket.recv_from(&mut packet) {
                    Ok((len, source)) if len <= MAX_REQUEST => {
                        let Ok(snapshot) = library.read() else { break };
                        if let Some(reply) = handle_datagram(&packet[..len], &snapshot, port) {
                            let _ = socket.send_to(&reply, source);
                        }
                    }
                    Ok(_) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock
                                | io::ErrorKind::TimedOut
                                | io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => break,
                }
            }
        })
}

/// The dbserver must advertise this virtual path, not `track.path`.
pub fn export_path(track: &LinkTrack) -> String {
    format!("/conduction/{}/{}", track.id, export_name(track))
}

fn export_name(track: &LinkTrack) -> String {
    track
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 255
                && *name != "."
                && *name != ".."
                && !name.contains(['/', '\\', '\0'])
        })
        .map(str::to_owned)
        .unwrap_or_else(|| format!("track-{}.audio", track.id))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Node {
    Root,
    Library,
    TrackDirectory(u32),
    Track(u32),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PathEncoding {
    #[default]
    Utf8,
    Utf16Le,
}

impl PathEncoding {
    fn encode(self, value: &str) -> Vec<u8> {
        match self {
            Self::Utf8 => value.as_bytes().to_vec(),
            Self::Utf16Le => value.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        }
    }

    fn decode(self, bytes: &[u8]) -> RpcResult<String> {
        let text = match self {
            Self::Utf8 => std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| RpcError::GarbageArguments)?,
            Self::Utf16Le => {
                if bytes.len() & 1 != 0 {
                    return Err(RpcError::GarbageArguments);
                }
                let units: Vec<_> = bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                String::from_utf16(&units).map_err(|_| RpcError::GarbageArguments)?
            }
        };
        if text.contains('\0') {
            return Err(RpcError::GarbageArguments);
        }
        Ok(text)
    }
}

impl Node {
    #[cfg(test)]
    fn handle(self) -> [u8; HANDLE_SIZE] {
        self.encoded_handle(PathEncoding::Utf8)
    }

    fn encoded_handle(self, encoding: PathEncoding) -> [u8; HANDLE_SIZE] {
        let mut bytes = [0; HANDLE_SIZE];
        bytes[..7].copy_from_slice(HANDLE_MAGIC);
        let (kind, id): (u8, u32) = match self {
            Self::Root => (0, 0),
            Self::Library => (1, 0),
            Self::TrackDirectory(id) => (2, id),
            Self::Track(id) => (3, id),
        };
        bytes[7] = kind
            | if encoding == PathEncoding::Utf16Le {
                0x80
            } else {
                0
            };
        bytes[8..12].copy_from_slice(&id.to_be_bytes());
        bytes
    }

    fn from_handle(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != HANDLE_SIZE || &bytes[..7] != HANDLE_MAGIC {
            return None;
        }
        // Authentic NXS LOOKUP calls preserve only the first twelve handle
        // bytes and overwrite the suffix. All identity and encoding therefore
        // live in that prefix; the registry still authorizes the resolved ID.
        let kind = bytes[7] & 0x7f;
        let id = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
        match (kind, id) {
            (0, 0) => Some(Self::Root),
            (1, 0) => Some(Self::Library),
            (2, id) => Some(Self::TrackDirectory(id)),
            (3, id) => Some(Self::Track(id)),
            _ => None,
        }
    }

    fn file_id(self) -> u32 {
        match self {
            Self::Root => 0,
            Self::Library => 1,
            Self::TrackDirectory(id) | Self::Track(id) => id,
        }
    }

    fn fs_id(self) -> u32 {
        // Distinct virtual filesystem namespaces keep (fsid, fileid) unique
        // for every possible u32 track ID without truncation or hash collisions.
        match self {
            Self::Root | Self::Library => FSID,
            Self::TrackDirectory(_) => FSID + 1,
            Self::Track(_) => FSID + 2,
        }
    }

    fn exists(self, library: &LibrarySnapshot) -> bool {
        match self {
            Self::Root | Self::Library => true,
            Self::TrackDirectory(id) | Self::Track(id) => find_track(library, id).is_some(),
        }
    }
}

fn find_track(library: &LibrarySnapshot, id: u32) -> Option<&LinkTrack> {
    library.tracks.iter().find(|track| track.id == id)
}

#[derive(Debug)]
enum RpcError {
    ProcedureUnavailable,
    GarbageArguments,
}

type RpcResult<T> = Result<T, RpcError>;

struct Xdr<'a> {
    bytes: &'a [u8],
    position: usize,
    path_encoding: PathEncoding,
}

impl<'a> Xdr<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            path_encoding: PathEncoding::Utf8,
        }
    }

    fn fixed(&mut self, count: usize) -> RpcResult<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(RpcError::GarbageArguments)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(RpcError::GarbageArguments)?;
        self.position = end;
        Ok(bytes)
    }

    fn u32(&mut self) -> RpcResult<u32> {
        let bytes = self.fixed(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn opaque(&mut self, maximum: usize) -> RpcResult<&'a [u8]> {
        let length = self.u32()? as usize;
        if length > maximum {
            return Err(RpcError::GarbageArguments);
        }
        let bytes = self.fixed(length)?;
        self.fixed((4 - length % 4) % 4)?;
        Ok(bytes)
    }

    fn path(&mut self, maximum: usize, encoding: PathEncoding) -> RpcResult<String> {
        let maximum_bytes = maximum
            * if encoding == PathEncoding::Utf16Le {
                2
            } else {
                1
            };
        encoding.decode(self.opaque(maximum_bytes)?)
    }

    fn mount_path(&mut self) -> RpcResult<(String, PathEncoding)> {
        let bytes = self.opaque(2048)?;
        // Export roots are absolute. Their initial slash establishes encoding
        // once; later Japanese components need no ambiguous byte heuristics.
        let encoding = if bytes.starts_with(b"/\0") {
            PathEncoding::Utf16Le
        } else {
            PathEncoding::Utf8
        };
        if encoding == PathEncoding::Utf8 && bytes.len() > 1024 {
            return Err(RpcError::GarbageArguments);
        }
        Ok((encoding.decode(bytes)?, encoding))
    }

    fn node(&mut self, library: &LibrarySnapshot) -> RpcResult<Result<Node, u32>> {
        let bytes = self.fixed(HANDLE_SIZE)?;
        self.path_encoding = if bytes[7] & 0x80 != 0 {
            PathEncoding::Utf16Le
        } else {
            PathEncoding::Utf8
        };
        Ok(Node::from_handle(bytes)
            .filter(|node| node.exists(library))
            .ok_or(NFS_STALE))
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_opaque(bytes: &mut Vec<u8>, value: &[u8]) {
    push_u32(bytes, value.len() as u32);
    bytes.extend_from_slice(value);
    bytes.resize(bytes.len() + (4 - value.len() % 4) % 4, 0);
}

fn accepted(xid: u32, status: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(128);
    for word in [xid, 1, 0, 0, 0, status] {
        push_u32(&mut bytes, word);
    }
    bytes
}

fn handle_datagram(packet: &[u8], library: &LibrarySnapshot, port: u16) -> Option<Vec<u8>> {
    if packet.len() > MAX_REQUEST {
        return None;
    }
    let mut reader = Xdr::new(packet);
    let xid = reader.u32().ok()?;
    if reader.u32().ok()? != 0 {
        return None; // Never respond to replies or other message types.
    }
    if reader.u32().ok()? != 2 {
        let mut bytes = Vec::new();
        for word in [xid, 1, 1, 0, 2, 2] {
            push_u32(&mut bytes, word);
        }
        return Some(bytes);
    }
    let header = (|| -> RpcResult<(u32, u32, u32)> {
        let program = reader.u32()?;
        let version = reader.u32()?;
        let procedure = reader.u32()?;
        // Public read-only export: AUTH_NULL and AUTH_SYS credentials do not
        // grant any extra rights. Both auth bodies have the RFC limit of 400.
        for _ in 0..2 {
            reader.u32()?;
            reader.opaque(400)?;
        }
        Ok((program, version, procedure))
    })();
    let Ok((program, version, procedure)) = header else {
        return Some(accepted(xid, 4));
    };
    let expected_version = match program {
        PMAP_PROGRAM | NFS_PROGRAM => 2,
        MOUNT_PROGRAM => 1,
        _ => return Some(accepted(xid, 1)),
    };
    if version != expected_version {
        let mut bytes = accepted(xid, 2);
        push_u32(&mut bytes, expected_version);
        push_u32(&mut bytes, expected_version);
        return Some(bytes);
    }
    let result = match program {
        PMAP_PROGRAM => portmapper(procedure, &mut reader, port),
        MOUNT_PROGRAM => mount(procedure, &mut reader),
        NFS_PROGRAM => nfs(procedure, &mut reader, library),
        _ => unreachable!(),
    };
    Some(match result {
        Ok(payload) => {
            let mut bytes = accepted(xid, 0);
            bytes.extend_from_slice(&payload);
            bytes
        }
        Err(RpcError::ProcedureUnavailable) => accepted(xid, 3),
        Err(RpcError::GarbageArguments) => accepted(xid, 4),
    })
}

fn portmapper(procedure: u32, reader: &mut Xdr<'_>, port: u16) -> RpcResult<Vec<u8>> {
    let mut bytes = Vec::new();
    match procedure {
        0 => {}
        1 | 2 => push_u32(&mut bytes, 0), // SET and UNSET never modify mappings.
        3 => {
            let program = reader.u32()?;
            let version = reader.u32()?;
            let protocol = reader.u32()?;
            reader.u32()?; // Caller-supplied port is not used by GETPORT.
            let supported = matches!(
                (program, version, protocol),
                (PMAP_PROGRAM, 2, 17) | (MOUNT_PROGRAM, 1, 17) | (NFS_PROGRAM, 2, 17)
            );
            push_u32(&mut bytes, if supported { port as u32 } else { 0 });
        }
        4 => {
            for (program, version) in [(PMAP_PROGRAM, 2), (MOUNT_PROGRAM, 1), (NFS_PROGRAM, 2)] {
                for value in [1, program, version, 17, port as u32] {
                    push_u32(&mut bytes, value);
                }
            }
            push_u32(&mut bytes, 0);
        }
        _ => return Err(RpcError::ProcedureUnavailable),
    }
    Ok(bytes)
}

fn mount(procedure: u32, reader: &mut Xdr<'_>) -> RpcResult<Vec<u8>> {
    let mut bytes = Vec::new();
    match procedure {
        0 | 4 => {}
        1 => {
            let (path, encoding) = reader.mount_path()?;
            let node = match path.as_str() {
                "/" => Some(Node::Root),
                "/conduction" | "/conduction/" => Some(Node::Library),
                _ => None,
            };
            push_u32(&mut bytes, if node.is_some() { NFS_OK } else { NFS_ACCES });
            if let Some(node) = node {
                bytes.extend_from_slice(&node.encoded_handle(encoding));
            }
        }
        2 => push_u32(&mut bytes, 0), // Stateless mounts have no tracking list.
        3 => {
            reader.mount_path()?;
        }
        5 => {
            for path in ["/", "/conduction"] {
                push_u32(&mut bytes, 1);
                push_opaque(&mut bytes, &PathEncoding::Utf16Le.encode(path));
                push_u32(&mut bytes, 0); // Empty group list: public export.
            }
            push_u32(&mut bytes, 0);
        }
        _ => return Err(RpcError::ProcedureUnavailable),
    }
    Ok(bytes)
}

fn io_status(error: io::Error) -> u32 {
    match error.kind() {
        io::ErrorKind::NotFound => NFS_NOENT,
        io::ErrorKind::PermissionDenied => NFS_ACCES,
        _ => NFS_IO,
    }
}

fn open_registered_file(track: &LinkTrack) -> Result<File, u32> {
    // The publishing adapter supplies canonical paths. On Unix the descriptor
    // walk below enforces this even if a component changes after this check.
    if !track.path.is_absolute()
        || std::fs::canonicalize(&track.path).map_err(io_status)? != track.path
    {
        return Err(NFS_ACCES);
    }

    #[cfg(unix)]
    {
        open_no_symlinks(&track.path)
    }
    #[cfg(not(unix))]
    {
        // Non-Unix hosts retain path validation but do not yet have the
        // descriptor-relative protection against concurrent ancestor changes.
        if !std::fs::symlink_metadata(&track.path)
            .map_err(io_status)?
            .is_file()
        {
            return Err(NFS_ACCES);
        }
        File::open(&track.path).map_err(io_status)
    }
}

#[cfg(unix)]
fn open_no_symlinks(path: &std::path::Path) -> Result<File, u32> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Component;

    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return Err(NFS_ACCES);
    }
    let mut components = components.peekable();
    let mut parent = File::open("/").map_err(io_status)?;
    while let Some(component) = components.next() {
        let Component::Normal(component) = component else {
            return Err(NFS_ACCES);
        };
        let name = CString::new(component.as_bytes()).map_err(|_| NFS_ACCES)?;
        let last = components.peek().is_none();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if last {
                // Opening a substituted FIFO must not block before fstat can
                // reject it. This flag does not affect regular-file reads.
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        // SAFETY: parent owns a live directory descriptor; name is a valid
        // NUL-terminated single component. No creation flag requires a mode.
        let descriptor = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            let error = io::Error::last_os_error();
            return Err(match error.raw_os_error() {
                Some(libc::ELOOP | libc::ENOTDIR) => NFS_ACCES,
                _ => io_status(error),
            });
        }
        // SAFETY: successful openat returned a new owned descriptor. File now
        // closes it exactly once, including all subsequent failure paths.
        let file = unsafe { File::from_raw_fd(descriptor) };
        if last {
            return Ok(file);
        }
        // Retain the opened directory, not its path: replacing an ancestor
        // with a symlink cannot redirect any remaining openat operation.
        parent = file;
    }
    Err(NFS_ACCES)
}

fn checked_metadata(file: &File) -> Result<Metadata, u32> {
    let metadata = file.metadata().map_err(io_status)?;
    if !metadata.is_file() {
        return Err(NFS_ACCES);
    }
    if metadata.len() > u32::MAX as u64 {
        return Err(NFS_FBIG); // NFSv2 offsets and sizes are only 32 bits.
    }
    Ok(metadata)
}

fn metadata(track: &LinkTrack) -> Result<Metadata, u32> {
    checked_metadata(&open_registered_file(track)?)
}

fn attributes(node: Node, library: &LibrarySnapshot) -> Result<Vec<u8>, u32> {
    let meta = if let Node::Track(id) = node {
        Some(metadata(find_track(library, id).ok_or(NFS_STALE)?)?)
    } else {
        None
    };
    Ok(encode_attributes(node, meta.as_ref()))
}

fn encode_attributes(node: Node, metadata: Option<&Metadata>) -> Vec<u8> {
    let is_file = matches!(node, Node::Track(_));
    let size = metadata.map_or(0, |meta| meta.len() as u32);
    let time = metadata
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    let seconds = time.map_or(0, |time| time.as_secs().min(u32::MAX as u64) as u32);
    let micros = time.map_or(0, |time| time.subsec_micros());
    let mut bytes = Vec::with_capacity(68);
    for value in [
        if is_file { 1 } else { 2 },
        if is_file { 0o100444 } else { 0o040555 },
        if is_file { 1 } else { 2 },
        0,
        0,
        size,
        MAX_READ as u32,
        0,
        (size as u64).div_ceil(512) as u32,
        node.fs_id(),
        node.file_id(),
        seconds,
        micros,
        seconds,
        micros,
        seconds,
        micros,
    ] {
        push_u32(&mut bytes, value);
    }
    bytes
}

fn lookup(directory: Node, name: &str, library: &LibrarySnapshot) -> Result<Node, u32> {
    if matches!(directory, Node::Track(_)) {
        return Err(NFS_NOTDIR);
    }
    if name.contains(['/', '\\', '\0']) || name.is_empty() {
        return Err(NFS_ACCES);
    }
    if name == "." {
        return Ok(directory);
    }
    if name == ".." {
        return Ok(match directory {
            Node::Root | Node::Library => Node::Root,
            _ => Node::Library,
        });
    }
    let node = match directory {
        Node::Root if name == "conduction" => Some(Node::Library),
        Node::Root => name.parse::<u32>().ok().map(Node::Track),
        Node::Library => name.parse::<u32>().ok().map(Node::TrackDirectory),
        Node::TrackDirectory(id) => find_track(library, id)
            .filter(|track| export_name(track) == name)
            .map(|_| Node::Track(id)),
        Node::Track(_) => unreachable!(),
    };
    node.filter(|node| node.exists(library)).ok_or(NFS_NOENT)
}

fn directory_entry(node: Node, index: usize, library: &LibrarySnapshot) -> Option<(String, Node)> {
    if index == 0 {
        return Some((".".into(), node));
    }
    if index == 1 {
        let parent = if matches!(node, Node::TrackDirectory(_)) {
            Node::Library
        } else {
            Node::Root
        };
        return Some(("..".into(), parent));
    }
    match node {
        Node::Root if index == 2 => Some(("conduction".into(), Node::Library)),
        Node::Root => library
            .tracks
            .get(index - 3)
            .map(|track| (track.id.to_string(), Node::Track(track.id))),
        Node::Library => library
            .tracks
            .get(index - 2)
            .map(|track| (track.id.to_string(), Node::TrackDirectory(track.id))),
        Node::TrackDirectory(id) if index == 2 => {
            find_track(library, id).map(|track| (export_name(track), Node::Track(id)))
        }
        _ => None,
    }
}

fn nfs(procedure: u32, reader: &mut Xdr<'_>, library: &LibrarySnapshot) -> RpcResult<Vec<u8>> {
    if matches!(procedure, 0 | 3 | 7) {
        return Ok(Vec::new()); // NULL and the two obsolete void procedures.
    }
    if matches!(procedure, 2 | 8..=15) {
        return Ok(NFS_ROFS.to_be_bytes().to_vec());
    }
    if !matches!(procedure, 1 | 4 | 5 | 6 | 16 | 17) {
        return Err(RpcError::ProcedureUnavailable);
    }
    let node = reader.node(library)?;
    let encoding = reader.path_encoding;
    // Decode the rest even when the handle is stale, so malformed arguments
    // are rejected at the RPC layer and never used to size an allocation.
    let mut payload = Vec::new();
    let result = match procedure {
        1 => node
            .and_then(|node| attributes(node, library))
            .map(|attrs| payload.extend(attrs)),
        4 => {
            let name = reader.path(255, encoding)?;
            node.and_then(|node| lookup(node, &name, library))
                .and_then(|node| attributes(node, library).map(|attrs| (node, attrs)))
                .map(|(node, attrs)| {
                    payload.extend_from_slice(&node.encoded_handle(encoding));
                    payload.extend(attrs);
                })
        }
        5 => node.and(Err(NFS_ACCES)), // There are no exported symlink nodes.
        6 => {
            let offset = reader.u32()? as u64;
            let count = (reader.u32()? as usize).min(MAX_READ);
            reader.u32()?;
            node.and_then(|node| read_file(node, offset, count, library))
                .map(|data| payload.extend(data))
        }
        16 => {
            let cookie = reader.u32()? as usize;
            let count = (reader.u32()? as usize).min(MAX_READ);
            node.and_then(|node| readdir(node, cookie, count, library, encoding))
                .map(|data| payload.extend(data))
        }
        17 => node.map(|_| {
            let blocks = library.tracks.iter().fold(0_u64, |sum, track| {
                sum.saturating_add(track.byte_size.div_ceil(4096))
            });
            for value in [
                MAX_READ as u32,
                4096,
                blocks.min(u32::MAX as u64) as u32,
                0,
                0,
            ] {
                push_u32(&mut payload, value);
            }
        }),
        _ => unreachable!(),
    };
    let mut bytes = Vec::with_capacity(payload.len() + 4);
    match result {
        Ok(()) => {
            push_u32(&mut bytes, NFS_OK);
            bytes.extend(payload);
        }
        Err(status) => push_u32(&mut bytes, status),
    }
    Ok(bytes)
}

fn read_file(
    node: Node,
    offset: u64,
    count: usize,
    library: &LibrarySnapshot,
) -> Result<Vec<u8>, u32> {
    let Node::Track(id) = node else {
        return Err(NFS_ISDIR);
    };
    let track = find_track(library, id).ok_or(NFS_STALE)?;
    let mut file = open_registered_file(track)?;
    let meta = checked_metadata(&file)?;
    let count = count
        .min(MAX_READ)
        .min(meta.len().saturating_sub(offset) as usize);
    let mut data = vec![0; count];
    file.seek(SeekFrom::Start(offset)).map_err(io_status)?;
    let mut received = 0;
    while received < count {
        match file.read(&mut data[received..]) {
            Ok(0) => break,
            Ok(size) => received += size,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io_status(error)),
        }
    }
    data.truncate(received);
    let mut bytes = encode_attributes(node, Some(&meta));
    push_opaque(&mut bytes, &data);
    Ok(bytes)
}

fn readdir(
    node: Node,
    cookie: usize,
    count: usize,
    library: &LibrarySnapshot,
    encoding: PathEncoding,
) -> Result<Vec<u8>, u32> {
    if count < 12 {
        return Err(NFS_IO);
    }
    let length = match node {
        Node::Root => library.tracks.len().saturating_add(3),
        Node::Library => library.tracks.len().saturating_add(2),
        Node::TrackDirectory(_) => 3,
        Node::Track(_) => return Err(NFS_NOTDIR),
    };
    let mut bytes = Vec::new();
    let mut next = cookie.min(length);
    // Build only entries fitting the packet, rather than allocating a listing
    // of the entire library for every request. Cookies refer to snapshot order;
    // like ordinary NFSv2 directories, a modified listing requires a new scan.
    while let Some((name, node)) = directory_entry(node, next, library) {
        let name = encoding.encode(&name);
        // Each entry: presence + fileid + string length/data/padding + cookie.
        let encoded_length = 16 + name.len().div_ceil(4) * 4;
        // Reserve status (4), end-of-list (4), and eof (4) within count.
        if bytes.len() + encoded_length + 12 > count {
            // NFSv2 has no TOOSMALL status. Do not return a successful page
            // with no entries and eof=false: clients cannot advance from it.
            if bytes.is_empty() {
                return Err(NFS_IO);
            }
            break;
        }
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, node.file_id());
        push_opaque(&mut bytes, &name);
        next += 1;
        push_u32(&mut bytes, next as u32);
    }
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, u32::from(next == length));
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Byte-for-byte hardware datagrams. Provenance, hashes, and the limited
    // handle substitution performed by replay tests are documented alongside.
    const CAPTURE_LOOKUP_REQUEST: &[u8] =
        include_bytes!("../tests/fixtures/nfs-s06-lookup-contents-request.bin");
    const CAPTURE_LOOKUP_REPLY: &[u8] =
        include_bytes!("../tests/fixtures/nfs-s06-lookup-contents-reply.bin");
    const CAPTURE_NEXT_LOOKUP: &[u8] =
        include_bytes!("../tests/fixtures/nfs-s06-lookup-artist-request.bin");
    const CAPTURE_TRACK_LOOKUP: &[u8] =
        include_bytes!("../tests/fixtures/nfs-s06-lookup-track-request.bin");
    const CAPTURE_READ: &[u8] = include_bytes!("../tests/fixtures/nfs-s06-read-8192-request.bin");
    const CAPTURE_MOUNT: &[u8] = include_bytes!("../tests/fixtures/nfs-s13-mount-usb-request.bin");
    const CAPTURE_MOUNT_REPLY: &[u8] =
        include_bytes!("../tests/fixtures/nfs-s13-mount-usb-reply.bin");
    const CAPTURE_EXPORT: &[u8] = include_bytes!("../tests/fixtures/nfs-linkinfo-export-reply.bin");

    fn request(program: u32, version: u32, procedure: u32, arguments: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [0x1234_5678, 0, 2, program, version, procedure, 0, 0, 0, 0] {
            push_u32(&mut bytes, word);
        }
        bytes.extend_from_slice(arguments);
        bytes
    }

    fn word(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn empty_library() -> LibrarySnapshot {
        LibrarySnapshot {
            tracks: Vec::new(),
            playlists: Vec::new(),
        }
    }

    fn reply(program: u32, version: u32, procedure: u32, args: &[u8]) -> Vec<u8> {
        handle_datagram(
            &request(program, version, procedure, args),
            &empty_library(),
            50111,
        )
        .unwrap()
    }

    #[test]
    fn captured_mount_export_and_lookup_use_unterminated_utf16le() {
        // AUTH_SYS has a 20-byte body, so captured call arguments begin at60.
        let mut mount_args = Xdr::new(&CAPTURE_MOUNT[60..]);
        let (path, encoding) = mount_args.mount_path().unwrap();
        assert_eq!((path.as_str(), encoding), ("/C/", PathEncoding::Utf16Le));
        assert_eq!(mount_args.position, 12);
        assert_eq!(&CAPTURE_MOUNT[70..72], &[0, 0x11]); // Nonzero XDR padding.
        assert_eq!(&CAPTURE_MOUNT_REPLY[..24], accepted(3, 0));
        assert_eq!(CAPTURE_MOUNT_REPLY.len(), 60); // status + fixed32 handle.

        let mut lookup_args = Xdr::new(&CAPTURE_LOOKUP_REQUEST[92..]);
        assert_eq!(
            lookup_args.path(255, PathEncoding::Utf16Le).unwrap(),
            "Contents"
        );
        assert_eq!(lookup_args.position, 20); // Four-byte length +16 data.
        assert_eq!(&CAPTURE_LOOKUP_REPLY[..24], accepted(4, 0));
        assert_eq!(CAPTURE_LOOKUP_REPLY.len(), 24 + 4 + 32 + 68);

        let mut exports = Xdr::new(&CAPTURE_EXPORT[24..]);
        for group in [
            "169.254.244.181/255.255.255.255",
            "169.254.192.112/255.255.255.255",
        ] {
            assert_eq!(exports.u32().unwrap(), 1);
            assert_eq!(
                exports.path(1024, PathEncoding::Utf16Le).unwrap(),
                "/C/EXPORT"
            );
            assert_eq!(exports.u32().unwrap(), 1);
            assert_eq!(exports.path(255, PathEncoding::Utf8).unwrap(), group);
            assert_eq!(exports.u32().unwrap(), 0);
        }
        assert_eq!(exports.u32().unwrap(), 0);
        assert_eq!(exports.position + 24, CAPTURE_EXPORT.len());
    }

    #[test]
    fn captured_directory_handle_suffix_is_opaque_client_context() {
        let returned = &CAPTURE_LOOKUP_REPLY[28..60];
        let subsequent = &CAPTURE_NEXT_LOOKUP[60..92];
        assert_eq!(&returned[..12], &subsequent[..12]);
        assert_ne!(&returned[12..], &subsequent[12..]);
        assert!(returned[12..].iter().all(|byte| *byte == 0));
        let mut local = Node::TrackDirectory(42).encoded_handle(PathEncoding::Utf16Le);
        local[12..].copy_from_slice(&subsequent[12..]);
        assert_eq!(Node::from_handle(&local), Some(Node::TrackDirectory(42)));
    }

    #[test]
    fn hardware_mount_encoding_is_retained_for_ambiguous_japanese_components() {
        let mut mount_args = Vec::new();
        push_opaque(
            &mut mount_args,
            &PathEncoding::Utf16Le.encode("/conduction"),
        );
        let mounted = reply(MOUNT_PROGRAM, 1, 1, &mount_args);
        assert_eq!(word(&mounted, 24), NFS_OK);
        assert_eq!(Node::from_handle(&mounted[28..60]), Some(Node::Library));
        assert_eq!(mounted[35] & 0x80, 0x80);

        // The component has no zero bytes and is also valid ASCII "n0".
        let encoded = PathEncoding::Utf16Le.encode("の");
        assert_eq!(encoded, b"n0");
        let mut args = Vec::new();
        push_opaque(&mut args, &encoded);
        assert_eq!(
            Xdr::new(&args).path(255, PathEncoding::Utf16Le).unwrap(),
            "の"
        );
        let listing =
            readdir(Node::Root, 0, 8192, &empty_library(), PathEncoding::Utf16Le).unwrap();
        let mut fields = Xdr::new(&listing);
        assert_eq!(fields.u32().unwrap(), 1);
        fields.u32().unwrap();
        assert_eq!(fields.path(255, PathEncoding::Utf16Le).unwrap(), ".");
    }

    #[test]
    fn captured_nxs_lookup_and_read_replay_against_registered_file() {
        use std::time::SystemTime;
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "conduction-nfs-replay-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let scratch = Scratch(std::fs::canonicalize(directory).unwrap());
        let filename = Xdr::new(&CAPTURE_TRACK_LOOKUP[92..])
            .path(255, PathEncoding::Utf16Le)
            .unwrap();
        let path = scratch.0.join(filename);
        let source: Vec<u8> = (0..16_384).map(|index| (index % 251) as u8).collect();
        std::fs::write(&path, &source).unwrap();
        let library = LibrarySnapshot {
            tracks: vec![LinkTrack {
                id: 42,
                path,
                byte_size: source.len() as u64,
                ..LinkTrack::default()
            }],
            playlists: Vec::new(),
        };
        let mut lookup_call = CAPTURE_TRACK_LOOKUP.to_vec();
        // Only substitute our server's identity prefix, leaving the captured
        // auth, suffix context, UTF16 name, and all other bytes untouched.
        lookup_call[60..72]
            .copy_from_slice(&Node::TrackDirectory(42).encoded_handle(PathEncoding::Utf16Le)[..12]);
        let lookup_reply = handle_datagram(&lookup_call, &library, 50111).unwrap();
        assert_eq!(word(&lookup_reply, 20), 0);
        assert_eq!(word(&lookup_reply, 24), NFS_OK);
        assert_eq!(
            Node::from_handle(&lookup_reply[28..60]),
            Some(Node::Track(42))
        );
        let mut read_call = CAPTURE_READ.to_vec();
        read_call[60..92].copy_from_slice(&lookup_reply[28..60]);
        let read_reply = handle_datagram(&read_call, &library, 50111).unwrap();
        assert_eq!(word(&read_reply, 20), 0);
        assert_eq!(word(&read_reply, 24), NFS_OK);
        assert_eq!(word(&read_reply, 96), 8192);
        assert_eq!(&read_reply[100..], &source[3213..3213 + 8192]);
    }

    #[test]
    fn handles_roundtrip_and_do_not_accept_paths_or_modified_bytes() {
        for node in [
            Node::Root,
            Node::Library,
            Node::TrackDirectory(7),
            Node::Track(u32::MAX),
        ] {
            assert_eq!(Node::from_handle(&node.handle()), Some(node));
            let mut forged = node.handle();
            forged[0] ^= 1;
            assert_eq!(Node::from_handle(&forged), None);
        }
        assert_eq!(Node::from_handle(b"/etc/passwd"), None);
        assert_ne!(Node::Track(1).handle(), Node::TrackDirectory(1).handle());
    }

    #[test]
    fn portmapper_maps_only_supported_udp_program_versions() {
        for (program, version, protocol, expected) in [
            (NFS_PROGRAM, 2, 17, 50111),
            (MOUNT_PROGRAM, 1, 17, 50111),
            (NFS_PROGRAM, 3, 17, 0),
            (NFS_PROGRAM, 2, 6, 0),
        ] {
            let mut args = Vec::new();
            for value in [program, version, protocol, 0] {
                push_u32(&mut args, value);
            }
            let response = reply(PMAP_PROGRAM, 2, 3, &args);
            assert_eq!(word(&response, 0), 0x1234_5678);
            assert_eq!(word(&response, 20), 0);
            assert_eq!(word(&response, 24), expected);
        }
    }

    #[test]
    fn mounts_only_virtual_roots_and_rejects_traversal() {
        for (path, expected) in [
            ("/", NFS_OK),
            ("/conduction", NFS_OK),
            ("/etc", NFS_ACCES),
            ("/conduction/../etc", NFS_ACCES),
        ] {
            let mut args = Vec::new();
            push_opaque(&mut args, path.as_bytes());
            let response = reply(MOUNT_PROGRAM, 1, 1, &args);
            assert_eq!(word(&response, 24), expected);
            assert_eq!(response.len(), if expected == NFS_OK { 60 } else { 28 });
        }
        let exports = reply(MOUNT_PROGRAM, 1, 5, &[]);
        let path = PathEncoding::Utf16Le.encode("/conduction");
        assert!(exports.windows(path.len()).any(|bytes| bytes == path));
    }

    #[test]
    fn malformed_packets_and_unsupported_rpc_are_bounded() {
        let library = empty_library();
        let normal = request(NFS_PROGRAM, 2, 1, &Node::Root.handle());
        for length in 0..normal.len() {
            let _ = handle_datagram(&normal[..length], &library, 50111);
        }
        let mut overflow = request(NFS_PROGRAM, 2, 0, &[]);
        overflow[28..32].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            word(&handle_datagram(&overflow, &library, 1).unwrap(), 20),
            4
        );
        assert!(handle_datagram(&vec![0; MAX_REQUEST + 1], &library, 1).is_none());
        assert_eq!(word(&reply(NFS_PROGRAM, 3, 0, &[]), 20), 2);
        assert_eq!(word(&reply(42, 1, 0, &[]), 20), 1);
        assert_eq!(word(&reply(NFS_PROGRAM, 2, 99, &[]), 20), 3);
    }

    #[test]
    fn write_operations_are_read_only_and_unregistered_handles_stale() {
        for procedure in [2, 8, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(word(&reply(NFS_PROGRAM, 2, procedure, &[]), 24), NFS_ROFS);
        }
        assert_eq!(
            word(&reply(NFS_PROGRAM, 2, 1, &Node::Track(9).handle()), 24),
            NFS_STALE
        );
        let mut args = Node::Root.handle().to_vec();
        push_opaque(&mut args, b"../../etc/passwd");
        assert_eq!(word(&reply(NFS_PROGRAM, 2, 4, &args), 24), NFS_ACCES);
    }

    #[test]
    fn readdir_respects_byte_budget_and_cookie() {
        let mut args = Node::Root.handle().to_vec();
        push_u32(&mut args, 0);
        push_u32(&mut args, 32);
        let response = reply(NFS_PROGRAM, 2, 16, &args);
        assert_eq!(response.len(), 24 + 32);
        assert_eq!(word(&response, response.len() - 4), 0);
        args[32..36].copy_from_slice(&1_u32.to_be_bytes());
        args[36..40].copy_from_slice(&8192_u32.to_be_bytes());
        let response = reply(NFS_PROGRAM, 2, 16, &args);
        assert_eq!(word(&response, response.len() - 4), 1);
        assert!(response.windows(10).any(|bytes| bytes == b"conduction"));
    }

    #[test]
    fn virtual_names_support_japanese_and_never_reveal_host_directories() {
        let track = LinkTrack {
            id: 42,
            path: "/private/music/夜の街.wav".into(),
            ..LinkTrack::default()
        };
        let library = LibrarySnapshot {
            tracks: vec![track],
            playlists: Vec::new(),
        };
        assert_eq!(export_path(&library.tracks[0]), "/conduction/42/夜の街.wav");
        assert_eq!(lookup(Node::Root, "42", &library), Ok(Node::Track(42)));
        assert_eq!(
            lookup(Node::Library, "42", &library),
            Ok(Node::TrackDirectory(42))
        );
        assert_eq!(
            lookup(Node::TrackDirectory(42), "夜の街.wav", &library),
            Ok(Node::Track(42))
        );
        assert_eq!(
            lookup(Node::TrackDirectory(42), "wrong.wav", &library),
            Err(NFS_NOENT)
        );
        assert_eq!(lookup(Node::Root, "private", &library), Err(NFS_NOENT));
        assert_eq!(lookup(Node::Root, "..", &library), Ok(Node::Root));
        assert_eq!(
            lookup(Node::TrackDirectory(42), "../private", &library),
            Err(NFS_ACCES)
        );
        assert_eq!(
            readdir(
                Node::TrackDirectory(42),
                2,
                32,
                &library,
                PathEncoding::Utf8
            ),
            Err(NFS_IO)
        );
    }

    #[test]
    fn readdir_rejects_budgets_that_cannot_make_progress() {
        for count in [0, 8, 12, 16, 31] {
            assert_eq!(
                readdir(Node::Root, 0, count, &empty_library(), PathEncoding::Utf8),
                Err(NFS_IO)
            );
        }
    }

    #[test]
    fn read_serves_only_registered_file_with_bounded_exact_offsets() {
        // Use the test executable as an existing regular file, avoiding any
        // filesystem mutation or externally installed media in this test.
        let path = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let source = std::fs::read(&path).unwrap();
        assert!(source.len() > MAX_READ);
        let library = LibrarySnapshot {
            tracks: vec![LinkTrack {
                id: 23,
                path,
                byte_size: source.len() as u64,
                ..LinkTrack::default()
            }],
            playlists: Vec::new(),
        };
        for (offset, requested, expected_length) in [
            (17, u32::MAX, MAX_READ),
            ((source.len() - 3) as u32, 100, 3),
            (source.len() as u32, 100, 0),
            (u32::MAX, 100, 0),
        ] {
            let mut args = Node::Track(23).handle().to_vec();
            for value in [offset, requested, 0] {
                push_u32(&mut args, value);
            }
            let response =
                handle_datagram(&request(NFS_PROGRAM, 2, 6, &args), &library, 50111).unwrap();
            assert_eq!(word(&response, 20), 0);
            assert_eq!(word(&response, 24), NFS_OK);
            assert_eq!(word(&response, 96), expected_length as u32);
            assert_eq!(response.len(), 100 + expected_length.div_ceil(4) * 4);
            if expected_length != 0 {
                assert_eq!(
                    &response[100..100 + expected_length],
                    &source[offset as usize..offset as usize + expected_length]
                );
            }
        }
        assert_eq!(read_file(Node::Track(24), 0, 10, &library), Err(NFS_STALE));
        assert_eq!(read_file(Node::Root, 0, 10, &library), Err(NFS_ISDIR));
    }

    #[cfg(unix)]
    #[test]
    fn replacing_registered_file_with_symlink_revokes_reads() {
        use std::os::unix::fs::symlink;
        use std::time::SystemTime;

        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("conduction-nfs-{}-{unique}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let scratch = Scratch(std::fs::canonicalize(directory).unwrap());
        let published = scratch.0.join("published.wav");
        let private = scratch.0.join("private.wav");
        std::fs::write(&published, b"audio").unwrap();
        std::fs::write(&private, b"not published").unwrap();
        let library = LibrarySnapshot {
            tracks: vec![LinkTrack {
                id: 9,
                path: published.clone(),
                ..LinkTrack::default()
            }],
            playlists: Vec::new(),
        };
        assert!(read_file(Node::Track(9), 0, 8192, &library).is_ok());
        std::fs::remove_file(&published).unwrap();
        symlink(&private, &published).unwrap();
        assert_eq!(read_file(Node::Track(9), 0, 8192, &library), Err(NFS_ACCES));
        assert_eq!(attributes(Node::Track(9), &library), Err(NFS_ACCES));
        assert_eq!(open_no_symlinks(&published).err(), Some(NFS_ACCES));
    }

    #[cfg(unix)]
    #[test]
    fn replacing_registered_parent_directory_with_symlink_revokes_reads() {
        use std::os::unix::fs::symlink;
        use std::time::SystemTime;

        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "conduction-nfs-parent-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let scratch = Scratch(std::fs::canonicalize(directory).unwrap());
        let published_directory = scratch.0.join("published");
        let private_directory = scratch.0.join("private");
        std::fs::create_dir(&published_directory).unwrap();
        std::fs::create_dir(&private_directory).unwrap();
        let published = published_directory.join("track.wav");
        std::fs::write(&published, b"audio").unwrap();
        std::fs::write(private_directory.join("track.wav"), b"not published").unwrap();
        let library = LibrarySnapshot {
            tracks: vec![LinkTrack {
                id: 10,
                path: published.clone(),
                ..LinkTrack::default()
            }],
            playlists: Vec::new(),
        };
        assert!(read_file(Node::Track(10), 0, 8192, &library).is_ok());
        assert_eq!(std::fs::canonicalize(&published).unwrap(), published);
        std::fs::rename(&published_directory, scratch.0.join("original")).unwrap();
        symlink(&private_directory, &published_directory).unwrap();
        // Invoke the descriptor walk directly to model substitution after the
        // canonical-path check, rather than relying on that earlier check.
        assert_eq!(open_no_symlinks(&published).err(), Some(NFS_ACCES));
        assert_eq!(
            read_file(Node::Track(10), 0, 8192, &library),
            Err(NFS_ACCES)
        );
        assert_eq!(attributes(Node::Track(10), &library), Err(NFS_ACCES));
    }

    #[test]
    fn loopback_socket_returns_rpc_and_stops() {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = start_socket(
            socket,
            Arc::new(RwLock::new(empty_library())),
            Arc::clone(&stop),
        )
        .unwrap();
        let client = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .send_to(&request(NFS_PROGRAM, 2, 0, &[]), address)
            .unwrap();
        let mut bytes = [0; 128];
        let (length, source) = client.recv_from(&mut bytes).unwrap();
        assert_eq!(source, address);
        assert_eq!(&bytes[..length], accepted(0x1234_5678, 0));
        stop.store(true, Ordering::Release);
        worker.join().unwrap();
    }
}

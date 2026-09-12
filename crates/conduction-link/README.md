# Conduction Pro DJ Link

This crate supplies an experimental native network service, independently
implemented from the public [DJ Link Ecosystem Analysis](https://djl-analysis.deepsymmetry.org/djl-analysis/)
and [the original NFS investigation](https://github.com/Deep-Symmetry/dysentery/issues/5).
It does not incorporate other implementations' source code.

`LinkHandle::start` receives a selected IPv4 interface, its actual MAC address,
its broadcast address, and an immutable library catalog. Network access is off
by default. Bind failures are returned before any advertisements are sent;
another application using the same ports must be closed. `stop` joins the
network tasks. Configuration changes are applied by stopping/restarting.

## Implemented protocol surface

- UDP 50000: discovery, CDJ-3000-compatible startup, player-number election in
  1–4, collision backoff, and keepalives. A conflicting rekordbox source 17
  stops the service; existing players' numbers are not defended or stolen.
- UDP 50001/50002: beat and CDJ/mixer status parsing, virtual player status,
  audio-clock-derived beat transmission, Sync commands, and coordinated tempo
  master handoff. Stale audio telemetry stops beat transmission. Remote device
  loss is reported without stopping local audio.
- Library source 17: export keepalives (`0x02`), initial hello (`0x11`),
  activation (`0x47`), status (`0x16`) and media announcements, TCP 12523 database
  port query, TCP 1051 typed dbserver framing, per-client paged menus, track
  paths/metadata, beat grids, cue lists, and legacy waveform responses.
- UDP 50111: read-only portmapper, mount v1, and NFSv2. Exported paths are
  virtual catalog paths, and each handle resolves to a registered track ID.
  The server accepts hardware UTF-16LE paths and retains the encoding in its
  handles. Handles tolerate the opaque trailing bytes rewritten by real CDJs.
  Reads are capped at 8192 bytes. Writes and arbitrary filesystem traversal
  are rejected. No system rpcbind configuration or privileged port is used.

The application feeds one-based absolute beat positions in `LocalClock`,
sampled from its audio engine. Beat events use Unix epoch microseconds for
cross-thread timestamp exchange; consumers should map each received event
once to their monotonic audio timeline. Local output latency delays outgoing
beat boundaries by `latency_ms`. The service owns master handoff state and
reports the selected master in both its snapshot and change events.

## Verification and remaining hardware work

`hardware_verified` remains false. Protocol unit tests and loopback socket
tests validate framing, bounds, discovery, collision selection, Unicode
metadata, menus, cues, file lookup/read, paging and shutdown. These tests do
not establish CDJ or mixer interoperability. Tests use ephemeral loopback
ports and never announce onto a physical LAN.

Independent hardware fixtures verify CDJ discovery/status/beat packets, root
and track menu rows, `0x2102` format and file-size metadata, beat/cue envelopes,
and NFS mounts, Unicode lookups, handle handling and 8192-byte reads. Fixture
provenance is recorded in `tests/fixtures/wire-provenance.md` and
`tests/fixtures/nfs-provenance.md`; database fixtures cite their S05/S13 captures
beside the assertions. In contrast, the complete source-17 bootstrap and
`0x3007` media-initialization acknowledgement are derived from published
reference interface observations: no independent complete export-session
capture was available. They remain part of the required hardware acceptance.

Library activation explicitly selects a full overview, legacy blue waveform,
classic key notation and a centred waveform. The activation message has no
documented neutral preference values. The service does not advertise or
provide a My Settings file.

Before a hardware-compatible release, verify library discovery, browse,
simultaneous track loads, waveform/cue display and master handoff together on
CDJ-3000 and CDJ-2000NXS2, then measure 60-minute playback stability and beat
drift with the intended interface and firmware. Confirm dedicated mixer-port
assignment behavior as well as an ordinary Ethernet switch.

Legacy blue waveforms are derived from Conduction's measured band RMS data;
resampling existing overview bins gives approximate detailed waveforms.
Enhanced color/three-band analysis-tag requests and artwork are explicitly
unavailable. Native analysis-file export, write operations, and audio files
larger than the NFSv2 32-bit file-size limit are unsupported. NFS handles are
read-only, not authenticated: enabling library publication makes the
registered catalog accessible on the selected network interface.
On Unix, file access walks each component through retained directory
descriptors with `O_NOFOLLOW`; non-Unix uses weaker path checks and requires
additional platform validation before library publication is considered stable.

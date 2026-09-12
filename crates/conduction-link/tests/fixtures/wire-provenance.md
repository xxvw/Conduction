# Wire fixture provenance

These vectors have independent expected bytes. They are not produced by the
Rust encoders under test. They do not establish Conduction hardware support.

## Public hardware captures

The following are unchanged UDP payloads extracted from the first matching
packet in [Deep Symmetry's LinkInfo.pcapng](https://github.com/Deep-Symmetry/dysentery/blob/main/doc/assets/LinkInfo.pcapng):

- `wire-cdj-final-claim.hex`: CDJ-2000nexus, type 4, **38 bytes**. Both the
  datagram and its length word are 0x26. The startup document's prose says
  0x2a, which disagrees with this packet and its own diagram.
- `wire-cdj-keepalive.hex`: CDJ-2000nexus, player 2. It reports class 1 at
  0x34 and first-on-network value 2 at 0x25; those fields have different roles.
- `wire-mixer-assignment.hex`: DJM-2000nexus assigns player number 3. The
  final byte is 0, distinguishing it from an ID-use refusal.
- `wire-mixer-beat.hex`: DJM-2000nexus, 120 BPM, fourth beat of the bar.
- `wire-cdj-status.hex`: player 2, paused at cue, track 50, 128 BPM, synced.

`wire-xdj-id-refusal.hex` is the unchanged XDJ-1000 refusal payload published
in [dysentery issue 31](https://github.com/Deep-Symmetry/dysentery/issues/31).
Its final byte is 1 and its number is already occupied; it is not an assignment.

`wire-source-media-capture.hex` is the unchanged 192-byte source-17 media
payload in [dysentery issue 5](https://github.com/Deep-Symmetry/dysentery/issues/5).
It advertises the rekordbox collection slot (4). Its My Settings flag is set;
Conduction intentionally leaves that flag clear because it does not serve
My Settings. Its media label is likewise different from Conduction's label.

## Published protocol observations, not public hardware captures

Files ending in `-reference.hex` are independently transcribed protocol
conformance vectors. No matching downloadable source-17 discovery, activation,
or status capture was found. Do not describe them as recorded hardware packets or interoperability
evidence. They use a fixed source number 17, test IP 192.168.1.9, and test MAC
02:03:04:05:06:07 where those fields exist.

The packet-family, field-offset, constant-value, and sequence facts come from
the [published vynull protocol interfaces](https://pkg.go.dev/github.com/vynulldev/vynull@v0.4.0/proto):

- `wire-source-keepalive-reference.hex`: export-source discovery is recurring
  type 2, length 50, with IP at 0x24, MAC at 0x28, source number at 0x2e and
  source class 4 at 0x30. It is not a player type-6 packet.
- `wire-source-hello-prefix-reference.hex`: first 40 bytes of the 296-byte
  type-0x11 source hello. The type is corroborated by the independent
  [OPUS-QUAD captured-protocol analysis](https://github.com/kyleawayan/opus-quad-pro-dj-link-analysis).
  The old issue-5 hello dump has an odd number of hexadecimal digits before
  the device name and cannot be used to establish type 0x17.
- `wire-source-activation-reference.hex`: type 0x47, length 72. Its six
  DEVSETTING bytes are explicit test preferences, not documented universal
  defaults: full overview, RGB waveforms, alphanumeric keys, centered waveforms.
- `wire-source-status-reference.hex`: type 0x16, length 48, with a zero length
  word; this format does not follow the usual remaining-length convention.

These are network interface facts used for independent interoperability work;
no third-party implementation source has been incorporated. The reference
author attributes the activation/status sequence to an unpublished capture.
The OPUS-QUAD lighting source has different keepalive fields and source number
23; its complete packet must not be substituted for the export source.

The MIT [prolink-connect DeviceType interface](https://github.com/evanpurkhiser/prolink-connect/blob/main/src/types.ts)
also identifies rekordbox class as 4. Its generic type-6 builder alone is not
evidence for the export-source packet family or the meaning of byte 0x25.

## Remaining evidence required

Capture Conduction and rekordbox export against CDJ-2000NXS2 and CDJ-3000,
checking discovery, the initial 0x11/0x47 exchange, source-17 media access,
NFS mount and playback. Record OS, device firmware, and source software
version. In particular, do not infer that periodically transmitting settings
notifications is necessary: the reference's earlier full-session capture did
not contain them, and Conduction does not advertise My Settings.

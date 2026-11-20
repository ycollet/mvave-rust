# mwave-rust

A native Rust terminal editor for the **M-VAVE Chocolate Plus**
(`FootCtrlPlus`, FootControl2 configuration layout). Runs on macOS and Linux,
without CubeSuite, Wine, or a browser.

**Status:** implements the publicly documented configuration layout and protocol.
Automated coverage uses a simulated pedal, not physical hardware. Real Bluetooth
communication, firmware-specific behavior, and persistence across power cycles
still need verification on a Chocolate Plus. Do not treat this as vendor-supported.

## Does the original Go project configure everything on the Plus?

**No.** [mvave-chocolate-tui](https://github.com/WilsonNet/mvave-chocolate-tui)
targets the original Chocolate. Its current editor writes CC numbers and latching
for 16 positions using the original device's MIDI SysEx protocol. It does not
implement the Plus's complete configuration layout, and some README features
do not match its current editor.

The Plus reference supplied for this project,
[chocolate-plus-re](https://github.com/dtnprt/chocolate-plus-re/blob/main/CubeSuite_analysis.md),
documents a different **23,646-byte** configuration and a custom Bluetooth GATT
protocol. This application independently implements those protocol/layout facts.
It does not incorporate the reference project's firmware or application code.
The [published web editor](https://dtnprt.github.io/chocolate/webconfig/) provides
additional field semantics and a compatible `.fcp` format.

## Configuration coverage

| Area | Editable settings |
|---|---|
| General | All 13 operating modes, global channel, TRS expression/MIDI, polarity, bank MIDI, PC display, group limits and active groups |
| Custom control | Four switch commands/toggles and expression command |
| Expression | 16 enabled/channel/CC/minimum/maximum entries |
| Advanced custom | 8 groups × 4 switches, five trigger behaviors, A/B lists of up to 16 PC/CC/Note On/Note Off messages |
| SysEx | Separate A/B buffers in every advanced and mixed MIDI switch; maximum **127 bytes** per buffer in the documented layout |
| Keyboard | 6 groups × 4 switches, two modifiers and a key |
| Mixed | 6 groups × 4 switches, MIDI lists, keyboard shortcuts or page-turner commands |
| Files | Lossless raw `.fcp` import/export, compatible with the documented CubeSuite/web-editor format |
| Monitoring | Optional native CoreMIDI/ALSA input monitor |

This covers the **documented configuration**, not every advertised or firmware
function. In particular, firmware flashing, Bluetooth pairing/renaming, USB host
routing internals, and firmware modifications are not provided. Some marketing
material mentions longer SysEx messages; this editor deliberately follows the
documented 128-byte storage buffer (one length byte plus 127 data bytes).
The original (non-Plus) Chocolate is **not supported**; its memory layout differs.
GAM-10 identification is accepted because the reference reports the same layout,
but that hardware is also unverified.

## Build and run

Install a current stable Rust toolchain.

On macOS, install Apple's command-line developer tools if necessary. Allow your
terminal Bluetooth access under **System Settings → Privacy & Security → Bluetooth**.
On Debian/Ubuntu, install the native build dependencies:

```sh
sudo apt install build-essential pkg-config libdbus-1-dev libasound2-dev bluez
```

```sh
cargo run -- --demo                     # safe simulated pedal
cargo run                              # TUI; press c to connect/read a real Plus
cargo run -- --file my-pedal.fcp        # offline editing; no connection on startup
cargo run -- --check my-pedal.fcp       # validate known fields/lists without hardware
cargo run -- --device FootCtrlPlus     # optional advertised-name filter
cargo run -- --list-midi
cargo run -- --midi-port 0             # optional real-time MIDI input log (l)
```

Close CubeSuite and other configuration clients before connecting. Enable the
pedal's Bluetooth; configuration uses Sinco **ae40 / ae41 / ae42**, **not BLE-MIDI
or USB MIDI**. Native MIDI ports are used only for the optional monitor.
Bluetooth pairing, when required, is managed by the operating system.
Multiple matching pedals are refused unless `--device` selects one uniquely.

The default BLE write size is 188 bytes (173 data bytes plus protocol overhead),
as used by the reference. If the negotiated link rejects that size,
`--ble-packet-size 20` enables experimental packet fragmentation. Acceptance of
fragmented writes by physical firmware is unverified; errors are surfaced and
frames are not blindly retried.

## Keys

| Key | Action |
|---|---|
| `Tab` / `Shift-Tab` | Next / previous configuration section |
| `[` / `]` | Previous / next group |
| `1`–`4` | Select switch A–D |
| `a` / `b` | Select message list A/B; single-step and long-press triggers use A only |
| `↑` / `↓`, `j` / `k` | Select setting |
| `PageUp` / `PageDown`, `Home` / `End` | Navigate long message lists |
| `Enter` / `e` | Edit; use decimal numbers or hex bytes for SysEx |
| `↑` / `↓` while editing | Browse choices or adjust a number |
| `Ctrl-U`, `Backspace` | Clear input / delete last character |
| `Enter` / `Esc` while editing | Apply local edit / cancel |
| `c` | Connect or reconnect and read the complete device configuration |
| `r` | Read device settings; asks before discarding local edits |
| `s` | Confirm write by typing `WRITE` |
| `i` / `o` | Import / export `.fcp` (enter a path) |
| `l` | Toggle device/MIDI log |
| `q` | Quit; asks about unsent edits |

Channels are displayed as **1–16**, while group indices and stored enum values
are **zero-based**. The editor shows names for enum values. Enable message slots
contiguously from slot 1; a disabled slot terminates a list. A SysEx entry must
be the only enabled message in its list; enter the actual `F0 ... F7` bytes in
the list's SysEx buffer at the bottom. PC uses the program/data1 field; data2 is
unused. Expression entries use CC/minimum/maximum rather than MIDI message types.
Inactive settings are preserved rather than automatically normalized.

## Write safety

Connecting never writes settings. Local imports/edits remain local until explicitly
confirmed. On connect, existing local edits are retained, and the device read
becomes the comparison baseline.

Before writing, the app validates known fields and active lists, checks that the
pedal has not changed since its last read, and creates a complete `.fcp` backup
under `backups/` (override with `--backup-dir`). It sends only changed byte ranges,
checks each acknowledgement, then rereads the **entire** configuration and
compares it byte-for-byte. Existing export/backup files are never overwritten.
Unknown and unused bytes survive import/export and unrelated edits.

Writes are **not atomic**: a rejected/timed-out operation may leave some changes
on the pedal. Errors close the session; reconnect/read before retrying and keep
the backup. Readback confirms current stored values, not persistence after reboot.
Keep-alive queries run every ten seconds. An idle disconnect is reported; `c`
reconnects, without automatically sending local settings.

## Development

```sh
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Tests cover framing/checksums, fragmented notifications, malformed replies and
device identification, full simulated read/write, mandatory backups, configuration
offsets, byte-preserving files, validation and terminal interaction/rendering.
CI runs these on macOS and Linux. Hardware checks must be performed separately.

MIT licensed; see [LICENSE](LICENSE).

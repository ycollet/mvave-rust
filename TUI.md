# Chocolate Plus TUI guide

This guide explains the six sections of `mwave-rust`, their settings, and how
to use them together. For installation and platform requirements, see
[README.md](README.md#build-and-run).

**Hardware status:** the editor follows the publicly documented Chocolate Plus
configuration layout. Physical Bluetooth communication, firmware-specific
behavior, and persistence after switching the pedal off still need testing.
Explanations below describe the intended behavior from that reference, not
results verified on a physical pedal.

## Contents

- [Start safely](#start-safely)
- [Understanding the screen](#understanding-the-screen)
- [Navigation and editing](#navigation-and-editing)
- [General](#general)
- [Custom](#custom)
- [Expression](#expression)
- [Advanced](#advanced)
- [Keyboard](#keyboard)
- [Mixed](#mixed)
- [Reading, writing, importing and exporting](#reading-writing-importing-and-exporting)
- [Device and MIDI log](#device-and-midi-log)
- [Comparing with CubeSuite](#comparing-with-cubesuite)
- [Troubleshooting](#troubleshooting)

## Start safely

Try the editor without a pedal:

```sh
cargo run -- --demo
```

Demo mode connects to a simulated pedal. Reads and writes affect only that
simulation, and reconnecting starts it again with its initial configuration.
The demo starts in Advanced custom mode with empty message lists; it is **not**
a copy of the factory presets.

To use a real Chocolate Plus:

```sh
cargo run
```

Press `c` to connect and read the complete configuration. Connecting does not
write anything. Before making changes, press `o` and export an original backup
to a new filename such as `chocolate-original.fcp`.

Configuration uses the pedal's custom Bluetooth service, not its USB MIDI or
BLE-MIDI port. Close CubeSuite before connecting. The original, non-Plus
Chocolate has a different layout and is not supported.

## Understanding the screen

The top line shows the connection, change status, and editing location.

| Item | Meaning |
|---|---|
| Device identity | The pedal's reported name/version, or `Disconnected`. Demo identity contains `simulated`. |
| `UNSENT` | The local configuration differs from the last device read/write, or no comparison baseline exists yet. It does not necessarily mean the file has not been exported. |
| `unchanged` | No difference from the current device baseline. This is not a power-cycle persistence check. |
| Group | Which group you are browsing in Advanced, Keyboard or Mixed. |
| Switch | Which physical switch you are editing: A, B, C or D. |
| List A/B | Which MIDI message list you are editing, when the trigger supports two lists. |

The six tabs are configuration editors. **Changing tabs does not change the
pedal's operating mode.** Select the actual mode under General.

Each table has a setting name, its value, and an `Offset`. The offset is the
byte's location in the configuration file, useful for debugging or comparing
backups. It is not a MIDI controller number or a setting you need to change.

The bottom of the screen shows the last status/error and available shortcuts.
The optional log is opened with `l`.

### Numbers that look similar but mean different things

| Number | Convention |
|---|---|
| MIDI channel | Displayed and entered as **1-16**. |
| Browsed group in the header | Displayed as **1-8** for Advanced, **1-6** for Keyboard/Mixed. |
| Active group fields in General | Entered as **zero-based indices**: `0` means group 1. |
| Maximum group index | Also zero-based: `7` means eight available groups. |
| Switch shortcut | `1`, `2`, `3`, `4` select physical switches A, B, C, D. |
| Message slot | Displayed as `01` through `16` within one list. |
| MIDI data | CC numbers, program values, note numbers, velocities and values use **0-127**. |
| Enum value | `3: Advanced custom`, for example, is a choice ID plus its name. |

Some receiving devices display MIDI program value `0` as preset **1**.
Check the receiver's documentation before assuming an off-by-one error.

## Navigation and editing

| Key | Action |
|---|---|
| `Tab` / `Shift-Tab` | Next / previous section. Changing sections resets the browsed group to group 1. |
| `[` / `]` | Previous / next browsed group, wrapping at the ends. |
| `1`-`4` | Select switch A-D. Relevant in Advanced, Keyboard and Mixed. |
| `a` / `b` | Select MIDI list A/B. |
| Up / Down, `j` / `k` | Move between settings. |
| `PageUp` / `PageDown` | Move by 15 settings in long lists. |
| `Home` / `End` | First / last setting. In Advanced, `End` reaches the SysEx buffer. |
| `Enter` / `e` | Open the selected setting's editor. |
| `l` | Show/hide the device and MIDI log. |

Inside an editor, type a decimal value, or use Up/Down to browse choices or
adjust a number. The nearby choices and their meanings appear in the dialog.
Use `Ctrl-U` to clear the existing input and `Backspace` to delete a character.
Press `Enter` to apply the **local** edit or `Esc` to cancel.

There is no general undo command for accepted edits. You can edit a value back,
import a saved backup, or use `r` to replace local settings with the device read.
Importing replaces the entire local configuration, not just the current tab.

## General

This section controls the overall operating mode and shared settings.

### Operating mode

Only the selected mode determines what the pedal does. You may edit settings
for other modes in advance without activating them.

| Value | Mode | Intended purpose |
|---|---|---|
| `0` | Program change A | Built-in program selection using A-D, with group changes. |
| `1` | Program change B | The reference describes CC-based selection, despite the mode's name. Verify the receiver's expectations. |
| `2` | Custom control | Simple per-switch commands and toggling, edited in Custom. |
| `3` | Advanced custom | Per-switch MIDI lists and trigger behaviors, edited in Advanced. |
| `4` | Manufacturer control | Built-in control for supported manufacturer equipment; consult that equipment's manual. |
| `5` | Touch screen | Built-in Android touch/swipe control; host support is required. |
| `6` | Video | Built-in video controls; compatible host software/extension may be required. |
| `7` | Keyboard A | Fixed Up, Down, Left, Right shortcuts. |
| `8` | Keyboard B | Fixed Page up, Page down, Space, Enter shortcuts. |
| `9` | Multimedia | Fixed previous/next track and volume controls. |
| `10` | Custom keyboard | Your keyboard shortcuts, edited in Keyboard. |
| `11` | Mixed key | MIDI, keyboard and page-turner actions mixed per switch, edited in Mixed. |
| `12` | Speaker | Built-in speaker control; exact supported equipment/behavior needs verification. |

Selecting a built-in mode does not install host software or expose undocumented
device-specific options. This application edits configuration, not firmware.

### Shared settings

| Field | Values | Explanation |
|---|---|---|
| TRS jack | `0: Expression`, `1: MIDI out` | Choose whether the jack is used for an expression pedal or TRS MIDI output. Use compatible cables/equipment. |
| Global MIDI channel | `1-16` | Shared channel setting. Advanced and Expression entries also have individual channels; changing this field does not rewrite those entries. |
| Reverse polarity | `0: Off`, `1: On` | Expression polarity setting, intended to reverse pedal direction. Exact behavior depends on pedal/firmware and needs hardware verification. |
| Bank MIDI | `0: Off`, `1: On` | The documented bank-MIDI flag. The precise messages/behavior it enables are not established here; retain the device's value until compared with CubeSuite. |
| PC display | `0: Off`, `1: On` | Program-change display option, exposed by the reference for Program change A. Exact display formatting needs hardware verification. |

The TUI shows these flags even when they are not relevant to the selected mode.
It does not erase their stored values when modes change.

### Group limits and active groups

| Field | Values | Meaning |
|---|---|---|
| PC A max group index | `0-31` | Last group in Program change A. Each group has four positions; the last raw program value is `4 * index + 3`. |
| PC B max group index | `0-31` | Corresponding limit for Program change B's built-in selection scheme. |
| Advanced max group index | `0-7` | Last Advanced group available when cycling groups on the pedal. |
| Active advanced group | `0-7` | Selected Advanced group on the pedal. |
| Active keyboard group | `0-5` | Selected custom Keyboard group on the pedal. |
| Active mixed group | `0-5` | Selected Mixed group on the pedal. |

For four Advanced groups, set the maximum index to `3`. To start in the second
group, set the active Advanced group to `1`. Keep the active group within the
chosen maximum; do not rely on the firmware to repair inconsistent settings.

Browsing with `[` and `]` only changes your **editing location**. It does not
update these active-group fields. Groups beyond a configured limit can still
be edited locally.

## Custom

Use this section for simple assignments in General mode **`2: Custom control`**.
All four switches are shown together; the header's selected switch does not
filter this table.

| Field | Meaning |
|---|---|
| Switch A-D toggle | `0: Momentary` or `1: Latching`. Momentary follows press/release; latching alternates a state across presses. The exact emitted values are not configurable in this section. |
| Switch A-D command | CC number `0-127`, or one of the special commands below. |
| Pedal command | Expression command: CC `0-127`, `128: Program Change`, or `129: Channel pressure`. |

Special switch command IDs are:

| Value | Command |
|---|---|
| `128` | Channel pressure |
| `129` | Start |
| `130` | Stop |
| `131` | Continue |
| `132` | Active sensing |
| `133` | Reset |

These are **selection IDs**, not CC numbers above 127. In particular, command
`128` means Channel pressure for a switch but Program Change for the expression pedal.
Avoid transport/reset commands unless the receiving equipment is meant to
respond to them.

For example, select command `80` to assign CC 80 to switch A. Choose its toggle
behavior to match the receiver. If you need explicit on/off values, different
channels, several messages, or note messages, use Advanced instead.

## Expression

This section is the reference's expression-pedal **Interface MIDI** list. It
contains up to 16 entries, allowing one pedal movement to control multiple
CC parameters with separate channels and ranges.

For an expression pedal connected to the TRS jack, select **TRS jack =
Expression** in General. Which expression settings are used in each operating
mode, including interaction with Custom's Pedal command, should be compared
with CubeSuite on your firmware; the editor exposes both stored settings.

Each numbered entry has:

| Field | Values | Meaning |
|---|---|---|
| Enabled | `0: Off`, `1: On` | Include the entry in the list. |
| MIDI channel | `1-16` | Receiving channel for this entry. |
| CC number | `0-127` | Parameter to control, as defined by the receiving device. |
| Minimum | `0-127` | Value at the low end of the pedal's travel. |
| Maximum | `0-127` | Value at the high end of the pedal's travel. |

An illustrative volume assignment is entry 01 enabled, channel `1`, CC `7`,
minimum `0`, maximum `127`. Confirm that your receiver actually uses CC 7 for
volume. Leave entry 02 and subsequent entries disabled for a one-message list.
Reversed ranges and their interaction with the polarity flag need hardware
verification; do not assume how the firmware combines them.

**Enabled entries must be contiguous:** enable 01, then 02, then 03, and so on.
Do not leave 02 disabled while 03 is enabled. Disabling an entry does not delete
or shift later entries; disable the rest as well when shortening the list.

## Advanced

Use this section with General mode **`3: Advanced custom`**.

There are **8 groups**, each with **4 switches**. Each switch has its own trigger
mode, up to two MIDI lists, and separate SysEx storage for each list.
Use `[`/`]` for the group and `1`-`4` for the switch.

### Trigger modes and lists A/B

| Value | Trigger mode | List A | List B |
|---|---|---|---|
| `0` | Single step | Messages for the single-step action | Not used |
| `1` | Alternating A/B | First bank of messages | Alternate bank of messages |
| `2` | Press / release | Messages on press | Messages on release |
| `3` | Long press | Messages for the long-press action | Not used |
| `4` | Short / long press | Messages for a short press | Messages for a long press |

Use `a` and `b` to edit the lists. In modes 0 and 3, the editor always shows
list A's fields even if the header's list indicator says B. The unused B data
is retained. Exact long-press timing and alternating-state behavior after
boot/group changes depend on firmware and are not configurable here.

Switching trigger modes can make a previously unused list active. Review both
lists before writing an Alternating, Press/release, or Short/long assignment.

### MIDI message entries

Each list has 16 numbered message slots. Enable them consecutively from 01;
the first disabled slot terminates the list. Disabling slots does not shift
or clear later slots automatically.

| Field | Meaning |
|---|---|
| Enabled | Include this message. |
| MIDI channel | Channel `1-16`; used for channel messages, not the contents of a SysEx buffer. |
| Message type | PC, CC, Note On, Note Off, or SysEx. |
| Program / CC / note | Program value for PC, controller number for CC, note number for Note On/Off. |
| Value / velocity | CC value or note velocity. Not used for PC. |

The message-type IDs are `0: PC`, `1: CC`, `2: Note On`, `3: Note Off`,
`4: SysEx`. Data values are decimal `0-127`.

For PC messages, only the program/data1 field is used; leave the unused
value/data2 field at a valid value such as `0`. Note On assignments should
have a corresponding Note Off action if the receiving instrument needs one,
otherwise notes may remain sounding.

### Example: explicit CC on press and off on release

Suppose your receiver enables an effect with CC 80 value 127, and disables it
with CC 80 value 0, on channel 1:

1. In General, select operating mode `3: Advanced custom`.
2. Go to Advanced; select group 1 and switch A.
3. Set Trigger mode to `2: Press / release`.
4. Select list A with `a`. Enable entry 01, set channel `1`, type `1: CC`,
   CC number `80`, and value `127`. Keep entries 02-16 disabled.
5. Select list B with `b`. Configure entry 01 the same way, but use value `0`.
   Keep entries 02-16 disabled.
6. Export a file if desired. Only after reviewing the assignment, press `s`
   and confirm with `WRITE`.

For a toggle across successive presses instead, use `1: Alternating A/B` with
the same two lists. The receiver's actual CC assignment and toggle behavior
must match; the example is not a universal mapping.

### SysEx

SysEx is a device-specific message format. Use a command documented by your
**receiving** device; arbitrary SysEx may change its settings.

To configure a SysEx action:

1. Select the appropriate switch, trigger mode and list.
2. Enable entry 01 and select type `4: SysEx`.
3. Disable entries 02-16. SysEx must be the **only enabled message** in that list.
4. Press `End` to reach the SysEx buffer, open it, and enter the command as hex
   byte pairs beginning with `F0` and ending with `F7`.

Spaces between byte pairs are allowed. Intermediate bytes must be `00-7F`;
any manufacturer-specific encoding or checksum must already be included.
The editor validates basic syntax, not the receiver's command semantics.
The slot's channel/data1/data2 fields do not construct the SysEx message;
leave them at valid values and supply the complete message in the buffer.

The documented storage permits **127 total message bytes**, including `F0`
and `F7`. A and B have separate buffers. Clearing a buffer does not disable
its message entry: either enter a valid command or disable that entry before
writing. Mixing SysEx with other enabled slots is rejected, not automatically
converted or rearranged.

## Keyboard

Use this section with General mode **`10: Custom keyboard`**.
There are **6 groups of 4 switches**. Each switch sends a shortcut consisting
of up to two modifier keys plus one normal key.

| Field | Values |
|---|---|
| Modifier 1 / Modifier 2 | `0: None`, `63: Ctrl`, `64: Shift`, `65: Alt`, `66: Win/Cmd` |
| Keyboard key | A named key selected from the table below |

The same nonzero modifier cannot occupy both modifier fields. For example,
Ctrl + Ctrl is rejected; Ctrl + Shift is allowed.

| Key value | Key |
|---|---|
| `0` | None |
| `1-26` | A-Z |
| `27-30` | Up, Down, Left, Right |
| `31`, `32` | Page up, Page down |
| `33`, `34`, `35` | Space, Enter, Escape |
| `36`, `37`, `38` | Delete, Caps lock, Tab |
| `39-48` | Digits 0-9 |
| `49`, `50` | Plus, minus |
| `51-62` | F1-F12 |

These are the pedal configuration's **key-selection IDs**, not ASCII values,
MIDI notes, or raw USB HID key codes. Prefer browsing the named choices.

For a simple page-turning assignment, set both modifiers to None and select
Page down (`32`). For Ctrl + Shift + A, use modifiers `63` and `64`, and key
`1: A`. The focused host application must recognize the shortcut, and the host
must have the pedal's keyboard connection available; configuration Bluetooth
alone does not prove that keyboard input is connected.

## Mixed

Use this section with General mode **`11: Mixed key`**.
There are **6 groups of 4 switches**. Unlike Advanced, each switch can have a
different kind of action.

| Mixed switch type | Settings shown |
|---|---|
| `0: MIDI` | The same trigger modes, A/B MIDI lists and SysEx buffers as Advanced |
| `1: Keyboard` | Two modifiers plus a key, as in Keyboard |
| `2: Page turner` | One predefined page-turner/media command |

Page-turner choices are:

| Value | Command |
|---|---|
| `0-3` | Up, Down, Left, Right |
| `4`, `5` | Page up, Page down |
| `6`, `7` | Previous track, Next track |
| `8`, `9` | Volume down, Volume up |

For example, one group could use A for a MIDI preset, B for a CC effect toggle,
C for a keyboard shortcut and D for Page down. Actual host/receiver connections
must support the chosen action types.

Changing the switch type changes which fields are displayed. It does **not**
erase hidden MIDI, keyboard or page-turner settings. Review those settings when
switching back to a previously used type.

## Reading, writing, importing and exporting

| Action | Key | Effect |
|---|---|---|
| Connect/reconnect | `c` | Reads the full pedal configuration. Existing local edits/imports are retained rather than sent. |
| Read | `r` | Replaces the local configuration with a full device read. If local changes exist, type `READ` to discard them. |
| Write | `s` | Type `WRITE` to request a device update. |
| Import | `i` | Enter a `.fcp` path. Replaces all local settings; sends nothing. |
| Export | `o` | Enter a new `.fcp` path. Saves all local settings; sends nothing. |
| Quit | `q` | Asks for `QUIT` if local settings differ from the device baseline or no baseline exists. |

An `.fcp` file is the raw **23,646-byte** Plus configuration, compatible with
the documented CubeSuite/web-editor format. It includes all sections, not just
the visible group. Import/export preserves unknown and inactive bytes.
Exports never overwrite existing files; choose a different filename.
Exporting does not clear `UNSENT`, because it does not send anything to the pedal.

Before writing, the app validates known settings and relevant MIDI lists,
rereads the pedal to check for external changes, and saves a full pre-write
backup under `backups/before-write-<timestamp>.fcp`. Override the directory with:

```sh
cargo run -- --backup-dir /path/to/my/backups
```

It then writes changed bytes, checks acknowledgements, reads the complete
configuration again, and compares it with the requested configuration.
Validation includes configured MIDI lists even if their operating mode is not
currently selected; imported inconsistencies elsewhere may therefore block a
write.

**Writes are not atomic.** An error may leave some changes applied. Keep the
backup and reconnect/read before deciding whether to retry or restore.
Successful readback does not establish that settings survive a power cycle.

To restore a backup, connect/read first, import the backup, then confirm a
write. Reconnecting with imported local settings preserves them; an explicit
`r` read, once confirmed, replaces them.

## Device and MIDI log

Press `l` to show recent connection/read/write/error messages. MIDI input is
optional and must be selected when launching:

```sh
cargo run -- --list-midi
cargo run -- --midi-port 0
```

Port indices come from the first command and may change when devices reconnect.
The log displays received MIDI bytes and timestamps; it does not decode them
into parameter names or automatically select a switch.

For example, `B0 50 7F` is CC 80, value 127, on channel 1. MIDI log bytes are
**hexadecimal**, whereas the channel/CC/value fields in the editor are
**decimal**. The first status byte `B0` identifies channel 1; `B1` identifies
channel 2, and so on through `BF` for channel 16.

The monitor observes the selected computer MIDI input, not every output of the
pedal. Messages sent only to another piece of hardware may not appear there.
Monitoring and Bluetooth configuration are separate connections.

## Comparing with CubeSuite

The intended correspondences are:

| TUI section/control | Reference/CubeSuite concept |
|---|---|
| General | Operating mode and shared settings |
| Custom | Custom control |
| Expression | Interface / expression-pedal MIDI list |
| Advanced | Advanced custom mode |
| Keyboard | Custom keyboard mode |
| Mixed | Mixed key mode |
| Group | Page/bank of four switch assignments |
| Alternating A/B lists | Bank 1 / Bank 2 message sets within one switch |
| Press/release lists | Press / Release |
| Short/long lists | Short press / Long press |

Labels can differ by CubeSuite version or language. In particular, an
alternating switch's two message "banks" are **not** the same thing as the
groups of four physical switch assignments.

A cautious comparison workflow is:

1. Save an original `.fcp` backup and note the pedal's firmware and CubeSuite
   versions.
2. Record one setting's name/value in CubeSuite, then close/disconnect CubeSuite.
3. Connect with this TUI and compare the corresponding value, accounting for
   channel, group and program-number conventions.
4. For uncertain settings, change only that setting in CubeSuite, disconnect
   it, then read/export another configuration in the TUI. Keep local edits
   exported before using `r`.
5. When trying a TUI write, verify the result in CubeSuite and on the receiver.
   Check power-cycle persistence separately, with your backups retained.

Do not use both configuration clients simultaneously. For an unclear control,
the useful comparison evidence is its screenshot, mode/group/switch/list,
before/after values and firmware version. Bank MIDI, PC display, special
manufacturer modes and firmware timing are particularly worth checking.

## Troubleshooting

| Symptom | Explanation or action |
|---|---|
| A tab's edits have no effect | Confirm General's operating mode, the active group, receiver channel and relevant physical connection. |
| The wrong group seems selected | The header's browsed group is not the active-group field. General uses zero-based indices. |
| List B looks identical to A | Single step and Long press use only A; the editor intentionally shows A's fields for those triggers. |
| Later messages do not run | Check for a disabled slot before an enabled slot; lists must be contiguous. |
| A SysEx write is rejected | Enable only slot 01, select SysEx, and enter a complete `F0 ... F7` command of at most 127 bytes in the selected list's buffer. |
| A modifier value is rejected | Choose None or a listed modifier ID, and do not duplicate a nonzero modifier. |
| Export reports an existing file | Use a new filename; overwriting is deliberately refused. |
| `UNSENT` remains after export | Expected: exporting a file does not update the pedal. |
| The pedal changed since the last read | Another client or the pedal changed stored settings. Export local edits, read/review the current device state, then decide what to apply. |
| The MIDI log stays empty | Confirm the selected MIDI input and where the pedal is sending messages; configuration BLE is not the MIDI input. |
| Connection fails or drops | Enable Bluetooth, allow terminal Bluetooth permission, close other clients and reconnect with `c`. Pair through the OS if required. |
| A larger BLE write fails | The negotiated packet size may be too small. `--ble-packet-size 20` enables experimental fragmentation, whose firmware support remains unverified. |

For the underlying protocol/layout references and remaining limitations, see
[README.md](README.md).

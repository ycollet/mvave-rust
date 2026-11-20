use std::{fs, path::Path};

use anyhow::{Context, Result, bail, ensure};

pub const SIZE: usize = 23_646;
pub const ADV_SIZE: usize = 417;
pub const MIX_SIZE: usize = 422;
pub const MODES: &[&str] = &[
    "Program change A",
    "Program change B",
    "Custom control",
    "Advanced custom",
    "Manufacturer control",
    "Touch screen",
    "Video",
    "Keyboard A",
    "Keyboard B",
    "Multimedia",
    "Custom keyboard",
    "Mixed key",
    "Speaker",
];
pub const TRIGGERS: &[&str] = &[
    "Single step",
    "Alternating A/B",
    "Press / release",
    "Long press",
    "Short / long press",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config(Vec<u8>);

impl Config {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            bytes.len() == SIZE,
            "Expected {SIZE} bytes, got {}",
            bytes.len()
        );
        Ok(Self(bytes))
    }

    pub fn demo() -> Self {
        let mut bytes = vec![0; SIZE];
        bytes[0] = 3;
        bytes[23639] = 7;
        Self(bytes)
    }

    pub fn load(path: &Path) -> Result<Self> {
        Self::from_bytes(fs::read(path).with_context(|| format!("Reading {}", path.display()))?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        // Refuse to overwrite an existing backup.
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| {
                format!(
                    "Creating {} (choose a new filename if it exists)",
                    path.display()
                )
            })?;
        file.write_all(&self.0)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn get(&self, addr: usize) -> u8 {
        self.0[addr]
    }

    pub fn differences(&self, other: &Self) -> Vec<std::ops::Range<usize>> {
        let mut ranges = Vec::new();
        let mut i = 0;
        while i < SIZE {
            if self.0[i] == other.0[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < SIZE && self.0[i] != other.0[i] {
                i += 1;
            }
            ranges.push(start..i);
        }
        ranges
    }

    pub fn value(&self, field: &Field) -> String {
        match &field.kind {
            Kind::Byte {
                choices,
                display_offset,
                ..
            } => {
                let value = self.get(field.addr);
                let label = choices.iter().find(|(v, _)| *v == value);
                match label {
                    Some((_, name)) => format!("{value}: {name}"),
                    None => (u16::from(value) + u16::from(*display_offset)).to_string(),
                }
            }
            Kind::SysEx => {
                let len = usize::from(self.get(field.addr));
                if len > 127 {
                    return format!("Invalid stored length {len}; edit to repair");
                }
                hex(&self.0[field.addr + 1..field.addr + 1 + len])
            }
        }
    }

    pub fn edit_value(&self, field: &Field) -> String {
        match field.kind {
            Kind::Byte { display_offset, .. } => {
                (u16::from(self.get(field.addr)) + u16::from(display_offset)).to_string()
            }
            Kind::SysEx => self.value(field),
        }
    }

    pub fn edit(&mut self, field: &Field, text: &str) -> Result<()> {
        match &field.kind {
            Kind::Byte {
                max,
                choices,
                display_offset,
            } => {
                let displayed: u16 = text.trim().parse().context("Enter a decimal number")?;
                let raw = displayed
                    .checked_sub(u16::from(*display_offset))
                    .context("Value below minimum")?;
                ensure!(
                    raw <= u16::from(*max),
                    "Maximum value is {}",
                    u16::from(*max) + u16::from(*display_offset)
                );
                let value = u8::try_from(raw)?;
                ensure!(
                    choices.is_empty() || choices.iter().any(|(v, _)| *v == value),
                    "Choose a listed value"
                );
                self.validate_related(field.addr, value)?;
                self.0[field.addr] = value;
            }
            Kind::SysEx => {
                let data = parse_hex(text)?;
                ensure!(
                    data.len() <= 127,
                    "The documented Plus layout allows at most 127 SysEx bytes"
                );
                if !data.is_empty() {
                    ensure!(
                        data.len() >= 2 && data[0] == 0xf0 && data.last() == Some(&0xf7),
                        "SysEx must start with F0 and end with F7"
                    );
                    ensure!(
                        data[1..data.len() - 1].iter().all(|b| *b < 128),
                        "SysEx payload must be 7-bit"
                    );
                }
                self.0[field.addr..field.addr + 128].fill(0);
                self.0[field.addr] = u8::try_from(data.len())?;
                self.0[field.addr + 1..field.addr + 1 + data.len()].copy_from_slice(&data);
            }
        }
        Ok(())
    }

    fn validate_related(&self, addr: usize, value: u8) -> Result<()> {
        let key_base = if (13437..13509).contains(&addr) {
            Some(13437 + (addr - 13437) / 3 * 3)
        } else if (13509..23637).contains(&addr)
            && (addr - 13509) % MIX_SIZE >= 2
            && (addr - 13509) % MIX_SIZE < 5
        {
            Some(13509 + (addr - 13509) / MIX_SIZE * MIX_SIZE + 2)
        } else {
            None
        };
        if let Some(base) = key_base
            && addr < base + 2
            && value != 0
        {
            let other = base + usize::from(addr == base);
            ensure!(self.get(other) != value, "Keyboard modifiers must differ");
        }
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_hex(text: &str) -> Result<Vec<u8>> {
    let digits: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    ensure!(digits.len().is_multiple_of(2), "Hex needs pairs of digits");
    ensure!(
        digits.is_ascii() && digits.bytes().all(|b| b.is_ascii_hexdigit()),
        "Use hex byte pairs separated by spaces"
    );
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).context("Invalid hex"))
        .collect()
}

#[derive(Clone, Debug)]
pub enum Kind {
    Byte {
        max: u8,
        choices: Vec<(u8, String)>,
        display_offset: u8,
    },
    SysEx,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub label: String,
    pub addr: usize,
    pub kind: Kind,
}

impl Field {
    fn number(label: impl Into<String>, addr: usize, max: u8) -> Self {
        Self {
            label: label.into(),
            addr,
            kind: Kind::Byte {
                max,
                choices: Vec::new(),
                display_offset: 0,
            },
        }
    }

    fn choices(label: impl Into<String>, addr: usize, names: &[&str]) -> Self {
        let choices = names
            .iter()
            .enumerate()
            .map(|(i, s)| (i as u8, s.to_string()))
            .collect();
        Self {
            label: label.into(),
            addr,
            kind: Kind::Byte {
                max: (names.len() - 1) as u8,
                choices,
                display_offset: 0,
            },
        }
    }

    fn channel(label: impl Into<String>, addr: usize) -> Self {
        Self {
            label: label.into(),
            addr,
            kind: Kind::Byte {
                max: 15,
                choices: Vec::new(),
                display_offset: 1,
            },
        }
    }

    pub fn help(&self, input: &str) -> String {
        match &self.kind {
            Kind::SysEx => "Hex bytes: F0 ... F7, maximum 127 bytes; empty clears".to_string(),
            Kind::Byte {
                max,
                choices,
                display_offset,
            } => {
                if choices.is_empty() {
                    format!(
                        "{}..{}",
                        display_offset,
                        u16::from(*max) + u16::from(*display_offset)
                    )
                } else {
                    let current = input.parse::<u8>().ok();
                    let selected = choices
                        .iter()
                        .position(|(v, _)| Some(*v) == current)
                        .unwrap_or(0);
                    let start = selected.saturating_sub(3);
                    let options = choices
                        .iter()
                        .skip(start)
                        .take(9)
                        .map(|(v, s)| format!("{v}={s}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "Up/Down: browse {} choices (or type a value)\n{options}",
                        choices.len()
                    )
                }
            }
        }
    }

    pub fn step(&self, input: &str, forward: bool) -> Option<String> {
        let Kind::Byte {
            max,
            choices,
            display_offset,
        } = &self.kind
        else {
            return None;
        };
        let value = input
            .parse::<u16>()
            .ok()
            .unwrap_or(u16::from(*display_offset));
        if choices.is_empty() {
            let min = u16::from(*display_offset);
            let max = u16::from(*max) + min;
            Some(
                if forward {
                    value.saturating_add(1).min(max)
                } else {
                    value.saturating_sub(1).max(min)
                }
                .to_string(),
            )
        } else {
            let index = choices
                .iter()
                .position(|(v, _)| u16::from(*v) == value)
                .unwrap_or(0);
            let next = (index + if forward { 1 } else { choices.len() - 1 }) % choices.len();
            Some(choices[next].0.to_string())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    General,
    Custom,
    Expression,
    Advanced,
    Keyboard,
    Mixed,
}

impl Section {
    pub const ALL: [Self; 6] = [
        Self::General,
        Self::Custom,
        Self::Expression,
        Self::Advanced,
        Self::Keyboard,
        Self::Mixed,
    ];
    pub const NAMES: [&str; 6] = [
        "General",
        "Custom",
        "Expression",
        "Advanced",
        "Keyboard",
        "Mixed",
    ];
    pub fn pages(self) -> usize {
        match self {
            Self::Advanced => 8,
            Self::Keyboard | Self::Mixed => 6,
            _ => 1,
        }
    }
}

pub fn fields(
    config: &Config,
    section: Section,
    page: usize,
    switch: usize,
    list_b: bool,
) -> Vec<Field> {
    let page = page.min(section.pages() - 1);
    let switch = switch.min(3);
    let mut out = Vec::new();
    match section {
        Section::General => {
            out.push(Field::choices("Operating mode", 0, MODES));
            out.push(Field::choices("TRS jack", 1, &["Expression", "MIDI out"]));
            out.push(Field::channel("Global MIDI channel", 2));
            for (addr, label, max) in [
                (23637, "PC A max group index (0..31 = PC 3..127)", 31),
                (23638, "PC B max group index (0..31 = CC 3..127)", 31),
                (23639, "Advanced max group index (0..7)", 7),
                (23640, "Active advanced group (0-based)", 7),
                (23641, "Active keyboard group (0-based)", 5),
                (23645, "Active mixed group (0-based)", 5),
            ] {
                out.push(Field::number(label, addr, max));
            }
            for (addr, label) in [
                (23642, "Reverse polarity"),
                (23643, "Bank MIDI"),
                (23644, "PC display"),
            ] {
                out.push(Field::choices(label, addr, &["Off", "On"]));
            }
        }
        Section::Custom => {
            for i in 0..5 {
                let name = if i == 4 {
                    "Pedal".to_string()
                } else {
                    format!("Switch {}", (b'A' + i as u8) as char)
                };
                if i < 4 {
                    out.push(Field::choices(
                        format!("{name} toggle"),
                        3 + i * 2,
                        &["Momentary", "Latching"],
                    ));
                }
                let mut field = Field::number(
                    format!("{name} command"),
                    4 + i * 2,
                    if i == 4 { 129 } else { 133 },
                );
                if let Kind::Byte { choices, .. } = &mut field.kind {
                    *choices = (0..128).map(|v| (v, format!("CC {v}"))).collect();
                    let extra: &[&str] = if i == 4 {
                        &["Program Change", "Channel pressure"]
                    } else {
                        &[
                            "Channel pressure",
                            "Start",
                            "Stop",
                            "Continue",
                            "Active sensing",
                            "Reset",
                        ]
                    };
                    choices.extend(
                        extra
                            .iter()
                            .enumerate()
                            .map(|(i, name)| (128 + i as u8, name.to_string())),
                    );
                }
                out.push(field);
            }
        }
        Section::Expression => midi_fields(&mut out, 13, true),
        Section::Advanced => advanced_fields(
            &mut out,
            config,
            93 + (page * 4 + switch) * ADV_SIZE,
            list_b,
        ),
        Section::Keyboard => keyboard_fields(&mut out, 13437 + (page * 4 + switch) * 3),
        Section::Mixed => {
            let base = 13509 + (page * 4 + switch) * MIX_SIZE;
            out.push(Field::choices(
                "Mixed switch type",
                base,
                &["MIDI", "Keyboard", "Page turner"],
            ));
            match config.get(base) {
                0 => advanced_fields(&mut out, config, base + 5, list_b),
                1 => keyboard_fields(&mut out, base + 2),
                2 => out.push(Field::choices(
                    "Page turner command",
                    base + 1,
                    &[
                        "Up",
                        "Down",
                        "Left",
                        "Right",
                        "Page up",
                        "Page down",
                        "Previous track",
                        "Next track",
                        "Volume down",
                        "Volume up",
                    ],
                )),
                _ => {}
            }
        }
    }
    out
}

fn keyboard_fields(out: &mut Vec<Field>, base: usize) {
    for i in 0..2 {
        out.push(Field {
            label: format!("Modifier {}", i + 1),
            addr: base + i,
            kind: Kind::Byte {
                max: 66,
                choices: vec![
                    (0, "None".into()),
                    (63, "Ctrl".into()),
                    (64, "Shift".into()),
                    (65, "Alt".into()),
                    (66, "Win/Cmd".into()),
                ],
                display_offset: 0,
            },
        });
    }
    let mut names = vec!["None".to_string()];
    names.extend(('A'..='Z').map(|c| c.to_string()));
    names.extend(
        [
            "Up",
            "Down",
            "Left",
            "Right",
            "Page up",
            "Page down",
            "Space",
            "Enter",
            "Escape",
            "Delete",
            "Caps lock",
            "Tab",
        ]
        .map(str::to_string),
    );
    names.extend((0..10).map(|i| i.to_string()));
    names.extend(["+".to_string(), "-".to_string()]);
    names.extend((1..=12).map(|i| format!("F{i}")));
    let refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    out.push(Field::choices("Keyboard key", base + 2, &refs));
}

fn advanced_fields(out: &mut Vec<Field>, config: &Config, base: usize, list_b: bool) {
    out.push(Field::choices("Trigger mode", base, TRIGGERS));
    let b = list_b && !matches!(config.get(base), 0 | 3);
    midi_fields(out, base + if b { 81 } else { 1 }, false);
    out.push(Field {
        label: "SysEx buffer (select SysEx in message 1)".into(),
        addr: base + if b { 289 } else { 161 },
        kind: Kind::SysEx,
    });
}

fn midi_fields(out: &mut Vec<Field>, base: usize, expression: bool) {
    for i in 0..16 {
        let a = base + i * 5;
        let prefix = format!("{:02}", i + 1);
        out.push(Field::choices(
            format!("{prefix} enabled (contiguous from 1)"),
            a,
            &["Off", "On"],
        ));
        out.push(Field::channel(format!("{prefix} MIDI channel"), a + 1));
        if expression {
            out.push(Field::number(format!("{prefix} CC number"), a + 2, 127));
        } else {
            out.push(Field::choices(
                format!("{prefix} message type"),
                a + 2,
                &["PC", "CC", "Note On", "Note Off", "SysEx"],
            ));
        }
        out.push(Field::number(
            format!(
                "{prefix} {}",
                if expression {
                    "Minimum"
                } else {
                    "Program / CC / note"
                }
            ),
            a + 3,
            127,
        ));
        out.push(Field::number(
            format!(
                "{prefix} {}",
                if expression {
                    "Maximum"
                } else {
                    "Value / velocity (unused for PC)"
                }
            ),
            a + 4,
            127,
        ));
    }
}

pub fn validate_lists(config: &Config) -> Result<()> {
    fn scalar(config: &Config, field: &Field) -> Result<()> {
        if let Kind::Byte { max, choices, .. } = &field.kind {
            let value = config.get(field.addr);
            ensure!(
                value <= *max && (choices.is_empty() || choices.iter().any(|(v, _)| *v == value)),
                "Invalid {} at 0x{:04X}: {value}",
                field.label,
                field.addr
            );
            config.validate_related(field.addr, value)?;
        }
        Ok(())
    }
    for section in [
        Section::General,
        Section::Custom,
        Section::Keyboard,
        Section::Mixed,
    ] {
        for page in 0..section.pages() {
            for switch in 0..if matches!(section, Section::Keyboard | Section::Mixed) {
                4
            } else {
                1
            } {
                let fields = fields(config, section, page, switch, false);
                for field in &fields {
                    if section == Section::Mixed
                        && config.get(13509 + (page * 4 + switch) * MIX_SIZE) == 0
                        && field.addr > 13509 + (page * 4 + switch) * MIX_SIZE
                    {
                        break;
                    }
                    scalar(config, field)?;
                }
            }
        }
    }
    fn validate(config: &Config, base: usize, sysex: bool) -> Result<()> {
        let mut ended = false;
        let mut count = 0;
        let mut has_sysex = false;
        for i in 0..16 {
            let a = base + i * 5;
            let enabled = config.get(a);
            ensure!(enabled <= 1, "Invalid enable flag at {a}");
            if enabled == 0 {
                ended = true;
                continue;
            }
            ensure!(!ended, "Message lists must be contiguous (offset {a})");
            count += 1;
            ensure!(config.get(a + 1) < 16, "Invalid MIDI channel at {}", a + 1);
            ensure!(
                config.get(a + 3) < 128 && config.get(a + 4) < 128,
                "Invalid MIDI data at {a}"
            );
            if sysex {
                ensure!(config.get(a + 2) <= 4, "Invalid MIDI type at {a}");
                has_sysex |= config.get(a + 2) == 4;
            } else {
                ensure!(config.get(a + 2) < 128, "Invalid expression CC at {a}");
            }
        }
        if has_sysex && count != 1 {
            bail!("SysEx must be the only message in its list (offset {base})");
        }
        Ok(())
    }
    validate(config, 13, false)?;
    for (start, groups, stride, shift) in [(93, 8, ADV_SIZE, 0), (13509, 6, MIX_SIZE, 5)] {
        for i in 0..groups * 4 {
            let outer = start + i * stride;
            if shift != 0 && config.get(outer) != 0 {
                continue;
            }
            let base = outer + shift;
            ensure!(config.get(base) <= 4, "Invalid trigger mode at {base}");
            for b in [false, true] {
                if b && matches!(config.get(base), 0 | 3) {
                    continue;
                }
                let messages = base + if b { 81 } else { 1 };
                validate(config, messages, true)?;
                if config.get(messages) == 1 && config.get(messages + 2) == 4 {
                    let addr = base + if b { 289 } else { 161 };
                    let field = Field {
                        label: String::new(),
                        addr,
                        kind: Kind::SysEx,
                    };
                    ensure!(
                        config.get(addr) >= 2 && config.get(addr) <= 127,
                        "Invalid SysEx length at {addr}"
                    );
                    let text = config.value(&field);
                    let mut copy = config.clone();
                    copy.edit(&field, &text)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_roundtrip_preserves_every_byte() -> Result<()> {
        let bytes = (0..SIZE).map(|i| i as u8).collect::<Vec<_>>();
        let config = Config::from_bytes(bytes.clone())?;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("backup.fcp");
        config.save(&path)?;
        assert_eq!(Config::load(&path)?.bytes(), bytes);
        assert!(config.save(&path).is_err());
        assert!(Config::from_bytes(vec![0; SIZE - 1]).is_err());
        Ok(())
    }

    #[test]
    fn last_group_switch_and_list_have_correct_offsets() {
        let mut config = Config::demo();
        config.0[93 + 31 * ADV_SIZE] = 2;
        let advanced = fields(&config, Section::Advanced, 7, 3, true);
        assert_eq!(advanced[1].addr, 93 + 31 * ADV_SIZE + 81);
        assert_eq!(advanced.last().unwrap().addr + 128, 13437);
        config.0[13509 + 23 * MIX_SIZE + 5] = 2;
        let mixed = fields(&config, Section::Mixed, 5, 3, true);
        assert_eq!(mixed.last().unwrap().addr + 128, 23637);
        let keys = fields(&config, Section::Keyboard, 5, 3, false);
        assert_eq!(keys.last().unwrap().addr, 13508);
    }

    #[test]
    fn edits_validate_and_preserve_unrelated_bytes() -> Result<()> {
        let mut config = Config::demo();
        let field = fields(&config, Section::General, 0, 0, false).remove(2);
        let original = config.clone();
        config.edit(&field, "16")?;
        assert_eq!(config.get(2), 15);
        assert_eq!(config.differences(&original), vec![2..3]);
        assert!(config.edit(&field, "17").is_err());
        assert!(config.edit(&field, "0").is_err());
        let keys = fields(&config, Section::Keyboard, 0, 0, false);
        config.edit(&keys[0], "63")?;
        assert!(config.edit(&keys[1], "63").is_err());
        assert!(config.edit(&keys[1], "1").is_err());
        Ok(())
    }

    #[test]
    fn sysex_is_strict_and_bounded() -> Result<()> {
        let mut config = Config::demo();
        let field = fields(&config, Section::Advanced, 0, 0, false)
            .pop()
            .unwrap();
        for invalid in ["F0 1 F7", "F0 XX F7", "F0 80 F7", "01 02", "F0"] {
            assert!(config.edit(&field, invalid).is_err(), "{invalid}");
        }
        config.edit(&field, "f0 01 02 f7")?;
        assert_eq!(config.get(field.addr), 4);
        assert_eq!(config.value(&field), "F0 01 02 F7");
        let too_long = format!("F0 {}F7", "01 ".repeat(126));
        assert!(config.edit(&field, &too_long).is_err());
        Ok(())
    }

    #[test]
    fn list_holes_and_sysex_mixtures_are_rejected() -> Result<()> {
        let mut config = Config::demo();
        config.0[99] = 1;
        assert!(validate_lists(&config).is_err());
        config.0[94] = 1;
        validate_lists(&config)?;
        config.0[96] = 4;
        assert!(validate_lists(&config).is_err());
        Ok(())
    }

    #[test]
    fn validates_mixed_keyboard_and_general_but_preserves_inactive_bytes() -> Result<()> {
        let mut config = Config::demo();
        config.0[11] = 255; // Unused expression toggle.
        config.0[93 + 81] = 255; // Inactive B list for a single-step trigger.
        validate_lists(&config)?;
        config.0[13509] = 1;
        config.0[13511] = 63;
        config.0[13512] = 63;
        assert!(validate_lists(&config).is_err());
        config.0[13512] = 64;
        validate_lists(&config)?;
        config.0[2] = 16;
        assert!(validate_lists(&config).is_err());
        Ok(())
    }

    #[test]
    fn validates_last_mixed_sysex_buffer_and_sixth_keyboard_group() -> Result<()> {
        let mut config = Config::demo();
        let base = 13509 + 23 * MIX_SIZE + 5;
        config.0[base] = 2;
        config.0[base + 81] = 1;
        config.0[base + 83] = 4;
        config.0[base + 289..base + 293].copy_from_slice(&[3, 0xf0, 1, 0xf7]);
        validate_lists(&config)?;
        config.0[base + 292] = 0xff;
        assert!(validate_lists(&config).is_err());
        config.0[base + 292] = 0xf7;
        config.0[13508] = 63;
        assert!(validate_lists(&config).is_err());
        Ok(())
    }
}

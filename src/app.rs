use std::{collections::VecDeque, path::PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::{Config, Field, Section, fields},
    device::{Command, Event},
};

enum Prompt {
    Edit(Field),
    Import,
    Export,
    ConfirmWrite,
    ConfirmRead,
    ConfirmQuit,
}

pub struct App {
    pub config: Option<Config>,
    baseline: Option<Config>,
    pub busy: bool,
    pub quit: bool,
    section: usize,
    page: usize,
    switch: usize,
    list_b: bool,
    selected: usize,
    table: TableState,
    prompt: Option<Prompt>,
    input: String,
    status: String,
    identity: String,
    log: VecDeque<String>,
    show_log: bool,
    commands: UnboundedSender<Command>,
}

impl App {
    pub fn new(config: Option<Config>, commands: UnboundedSender<Command>) -> Self {
        Self {
            config,
            baseline: None,
            busy: false,
            quit: false,
            section: 0,
            page: 0,
            switch: 0,
            list_b: false,
            selected: 0,
            table: TableState::default(),
            prompt: None,
            input: String::new(),
            status: "c: connect and read; i: import .fcp; no automatic device writes".into(),
            identity: "Disconnected".into(),
            log: VecDeque::new(),
            show_log: false,
            commands,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        self.send(
            Command::Connect,
            "Scanning/connecting and reading all settings...",
        )
    }

    fn send(&mut self, command: Command, status: &str) -> Result<()> {
        self.commands
            .send(command)
            .context("Device worker stopped")?;
        self.busy = true;
        self.status = status.to_string();
        Ok(())
    }

    fn dirty(&self) -> bool {
        match (&self.config, &self.baseline) {
            (Some(current), Some(original)) => current != original,
            (Some(_), None) => true,
            _ => false,
        }
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::Connected(identity, config) => {
                self.busy = false;
                self.identity = identity;
                let keep_local = self.config.is_some() && self.dirty();
                if !keep_local {
                    self.config = Some(config.clone());
                }
                self.baseline = Some(config);
                self.status = if keep_local {
                    "Connected; device read, local edits/import retained (not sent)"
                } else {
                    "Connected; full configuration read (no writes)"
                }
                .into();
            }
            Event::Read(config) => {
                self.busy = false;
                self.config = Some(config.clone());
                self.baseline = Some(config);
                self.status = "Full configuration read".into();
            }
            Event::Written(config, backup) => {
                self.busy = false;
                self.config = Some(config.clone());
                self.baseline = Some(config);
                self.status = format!(
                    "Write acknowledged and readback matched. Backup: {}",
                    backup.display()
                );
            }
            Event::Error(error) => {
                self.busy = false;
                self.identity = "Disconnected".into();
                self.baseline = None;
                self.status = format!("ERROR: {error}");
            }
            Event::Midi(message) => {
                self.log.push_back(message);
                while self.log.len() > 200 {
                    self.log.pop_front();
                }
                return;
            }
        }
        self.log.push_back(self.status.clone());
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    fn current_fields(&self) -> Vec<Field> {
        self.config
            .as_ref()
            .map(|config| {
                fields(
                    config,
                    Section::ALL[self.section],
                    self.page,
                    self.switch,
                    self.list_b,
                )
            })
            .unwrap_or_default()
    }

    pub fn key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return Ok(());
        }
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        if self.busy {
            if key.code == KeyCode::Char('q') {
                self.status =
                    "Operation in progress; wait before quitting, or Ctrl-C to request shutdown"
                        .into();
            }
            return Ok(());
        }
        let count = self.current_fields().len();
        match key.code {
            KeyCode::Char('q') => {
                if self.dirty() {
                    self.open(Prompt::ConfirmQuit, String::new());
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Char('c') => self.start()?,
            KeyCode::Char('r') => {
                if self.dirty() {
                    self.open(Prompt::ConfirmRead, String::new());
                } else {
                    self.send(Command::Read, "Reading full device configuration...")?;
                }
            }
            KeyCode::Char('s') if self.config.is_some() => {
                self.open(Prompt::ConfirmWrite, String::new())
            }
            KeyCode::Char('i') => self.open(Prompt::Import, String::new()),
            KeyCode::Char('o') if self.config.is_some() => {
                self.open(Prompt::Export, "chocolate-plus.fcp".into())
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.section = (self.section + if key.code == KeyCode::Tab { 1 } else { 5 }) % 6;
                self.page = 0;
                self.selected = 0;
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let pages = Section::ALL[self.section].pages();
                self.page = (self.page
                    + if key.code == KeyCode::Char(']') {
                        1
                    } else {
                        pages - 1
                    })
                    % pages;
                self.selected = 0;
            }
            KeyCode::Char(n @ '1'..='4') => {
                self.switch = n as usize - '1' as usize;
                self.selected = 0;
            }
            KeyCode::Char('a') => {
                self.list_b = false;
                self.selected = 0;
            }
            KeyCode::Char('b') => {
                self.list_b = true;
                self.selected = 0;
            }
            KeyCode::Char('l') => self.show_log = !self.show_log,
            KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                self.selected = (self.selected + 1) % count
            }
            KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                self.selected = (self.selected + count - 1) % count
            }
            KeyCode::PageDown if count > 0 => self.selected = (self.selected + 15).min(count - 1),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(15),
            KeyCode::Home => self.selected = 0,
            KeyCode::End if count > 0 => self.selected = count - 1,
            KeyCode::Enter | KeyCode::Char('e') if count > 0 => {
                let field = self.current_fields().remove(self.selected.min(count - 1));
                let text = self
                    .config
                    .as_ref()
                    .context("No configuration")?
                    .edit_value(&field);
                self.open(Prompt::Edit(field), text);
            }
            _ => {}
        }
        Ok(())
    }

    fn open(&mut self, prompt: Prompt, text: String) {
        self.prompt = Some(prompt);
        self.input = text;
    }

    fn prompt_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.input.clear();
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.prompt = None;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Up | KeyCode::Down => {
                if let Some(Prompt::Edit(field)) = &self.prompt
                    && let Some(value) = field.step(&self.input, key.code == KeyCode::Down)
                {
                    self.input = value;
                }
            }
            KeyCode::Enter => {
                let prompt = self.prompt.take().context("Missing prompt")?;
                let result = self.apply_prompt(&prompt);
                if let Err(error) = result {
                    self.status = format!("ERROR: {error:#}");
                    self.prompt = Some(prompt);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_prompt(&mut self, prompt: &Prompt) -> Result<()> {
        match prompt {
            Prompt::Edit(field) => {
                self.config
                    .as_mut()
                    .context("No configuration")?
                    .edit(field, &self.input)?;
                self.status = "Local edit only; s: review/write, o: export".into();
            }
            Prompt::Import => {
                self.config = Some(Config::load(&PathBuf::from(self.input.trim()))?);
                self.selected = 0;
                self.status = "Imported complete .fcp; not sent to pedal".into();
            }
            Prompt::Export => {
                self.config
                    .as_ref()
                    .context("No configuration")?
                    .save(&PathBuf::from(self.input.trim()))?;
                self.status = format!("Exported {}", self.input);
            }
            Prompt::ConfirmWrite => {
                anyhow::ensure!(
                    self.input.trim() == "WRITE",
                    "Type WRITE to confirm, or Esc to cancel"
                );
                let config = self.config.clone().context("No configuration")?;
                self.send(
                    Command::Write(config),
                    "Backing up, writing differences and verifying readback...",
                )?;
            }
            Prompt::ConfirmRead => {
                anyhow::ensure!(
                    self.input.trim() == "READ",
                    "Type READ to discard unsent edits, or Esc to cancel"
                );
                self.send(Command::Read, "Reading full device configuration...")?;
            }
            Prompt::ConfirmQuit => {
                anyhow::ensure!(
                    self.input.trim() == "QUIT",
                    "Type QUIT to discard unsaved changes, or Esc to cancel"
                );
                self.quit = true;
            }
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(if self.show_log { 8 } else { 0 }),
                Constraint::Length(4),
            ])
            .split(frame.area());
        let title = format!(
            "Chocolate Plus | {} | {} | group {} switch {} list {}",
            self.identity,
            if self.dirty() { "UNSENT" } else { "unchanged" },
            self.page + 1,
            (b'A' + self.switch as u8) as char,
            if self.list_b {
                "B (ignored for single/long trigger)"
            } else {
                "A"
            }
        );
        frame.render_widget(
            Paragraph::new(title).style(Style::default().fg(Color::Cyan)),
            areas[0],
        );
        frame.render_widget(
            Tabs::new(Section::NAMES.to_vec())
                .select(self.section)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().fg(Color::Yellow)),
            areas[1],
        );
        let fields = self.current_fields();
        self.selected = self.selected.min(fields.len().saturating_sub(1));
        self.table.select(if fields.is_empty() {
            None
        } else {
            Some(self.selected)
        });
        let rows = fields.iter().map(|field| {
            Row::new(vec![
                Cell::from(field.label.clone()),
                Cell::from(
                    self.config
                        .as_ref()
                        .map(|c| c.value(field))
                        .unwrap_or_default(),
                ),
                Cell::from(format!("0x{:04X}", field.addr)),
            ])
        });
        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(45),
                    Constraint::Min(12),
                    Constraint::Length(8),
                ],
            )
            .header(
                Row::new(["Setting", "Value", "Offset"])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Enter/e: edit (decimal values; channels 1..16)"),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("> "),
            areas[2],
            &mut self.table,
        );
        if self.show_log {
            let lines = self
                .log
                .iter()
                .rev()
                .take(6)
                .rev()
                .cloned()
                .map(Line::from)
                .collect::<Vec<_>>();
            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Device / MIDI log"),
                ),
                areas[3],
            );
        }
        let footer = format!(
            "{}\nTab: section  [/]: group  1-4: switch  a/b: list  arrows/PgUp/PgDn: navigate\nc: connect  r: read  s: write  i/o: import/export  l: log  q: quit",
            self.status
        );
        frame.render_widget(Paragraph::new(footer).wrap(Wrap { trim: false }), areas[4]);
        if let Some(prompt) = &self.prompt {
            let (title, help) = match prompt {
                Prompt::Edit(field) => (field.label.clone(), field.help(&self.input)),
                Prompt::Import => (
                    "Import .fcp (replaces local edits)".into(),
                    "Path to a raw 23646-byte CubeSuite backup".into(),
                ),
                Prompt::Export => (
                    "Export .fcp".into(),
                    "New file path; existing files are never overwritten".into(),
                ),
                Prompt::ConfirmWrite => (
                    "Confirm device write".into(),
                    "Type WRITE. Full backup first, then changed bytes only. No firmware updates."
                        .into(),
                ),
                Prompt::ConfirmRead => (
                    "Discard local changes?".into(),
                    "Type READ to replace local settings with device settings".into(),
                ),
                Prompt::ConfirmQuit => (
                    "Unsaved / unsent changes".into(),
                    "Type QUIT to exit without exporting or sending".into(),
                ),
            };
            let area = centered(frame.area());
            frame.render_widget(Clear, area);
            let content = format!(
                "> {}\nEnter: apply  Esc: cancel  Ctrl-U: clear\n{}\n\n{help}",
                self.input, self.status
            );
            frame.render_widget(
                Paragraph::new(content)
                    .wrap(Wrap { trim: false })
                    .block(Block::default().title(title).borders(Borders::ALL)),
                area,
            );
        }
    }
}

fn centered(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(100);
    let height = area.height.saturating_sub(2).min(18);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn offline_navigation_edit_cancel_and_confirmation() -> Result<()> {
        let (tx, mut rx) = unbounded_channel();
        let mut app = App::new(Some(Config::demo()), tx);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.key(key(KeyCode::Enter))?;
        app.key(key(KeyCode::Backspace))?;
        app.key(key(KeyCode::Char('1')))?;
        app.key(key(KeyCode::Enter))?;
        assert_eq!(app.config.as_ref().unwrap().get(0), 1);
        assert!(rx.try_recv().is_err());
        app.key(key(KeyCode::Char('s')))?;
        app.key(key(KeyCode::Esc))?;
        assert!(rx.try_recv().is_err());
        app.key(key(KeyCode::Char('s')))?;
        for c in "WRITE".chars() {
            app.key(key(KeyCode::Char(c)))?;
        }
        app.key(key(KeyCode::Enter))?;
        assert!(matches!(rx.try_recv()?, Command::Write(_)));
        Ok(())
    }

    #[test]
    fn renders_all_sections_and_small_terminal() -> Result<()> {
        let (tx, _) = unbounded_channel();
        let mut app = App::new(Some(Config::demo()), tx);
        for (width, height) in [(100, 30), (30, 8)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            for section in 0..6 {
                app.section = section;
                terminal.draw(|frame| app.draw(frame))?;
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .any(|cell| cell.symbol() == "C")
                );
            }
        }
        Ok(())
    }
}

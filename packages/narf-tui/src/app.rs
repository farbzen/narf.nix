//! Application state and key handling.

use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    widgets::ListState,
};

use crate::{
    EXEC_FLAG,
    runner::Runner,
    status::{self, Hosts, Status},
    ui,
};

pub enum Value {
    Toggle(bool),
    Text(String),
}

/// One narf option.
pub struct Field {
    pub flag: char,
    pub label: &'static str,
    pub help: &'static str,
    pub value: Value,
}

impl Field {
    fn toggle(flag: char, label: &'static str, help: &'static str) -> Self {
        Self {
            flag,
            label,
            help,
            value: Value::Toggle(false),
        }
    }

    fn text(flag: char, label: &'static str, help: &'static str) -> Self {
        Self {
            flag,
            label,
            help,
            value: Value::Text(String::new()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Options,
    Inputs,
}

pub enum Mode {
    Form,
    /// Editing a text field; holds the value to restore on Esc.
    Edit {
        backup: String,
    },
    /// Choosing the host from the flake's `nixosConfigurations`.
    PickHost(ListState),
    Run(Runner),
}

/// One entry of the host picker.
pub enum HostChoice {
    /// No `-H`, narf uses the hostname.
    Default,
    Host(String),
    /// Type the host by hand.
    Custom,
}

pub struct Input {
    pub name: String,
    pub selected: bool,
}

pub struct App {
    pub fields: Vec<Field>,
    pub inputs: Vec<Input>,
    pub focus: Focus,
    pub options_state: ListState,
    pub inputs_state: ListState,
    pub mode: Mode,
    pub status: Status,
    pub hosts: Hosts,
    pub command: String,
    /// Shown in the footer until the next key press.
    pub error: Option<String>,
    quit: bool,
}

impl App {
    pub fn new() -> Self {
        let fields = vec![
            Field::toggle('p', "Pull", "Pull new commits from origin before running"),
            Field::toggle(
                'u',
                "Update all inputs",
                "Update all flake inputs before running",
            ),
            Field::toggle(
                'b',
                "Build only",
                "Build the new generation without switching",
            ),
            Field::toggle('d', "Dry run", "Only evaluate and state what WOULD be done"),
            Field::toggle('t', "Show trace", "Show trace of the rebuild"),
            Field::toggle(
                'n',
                "Skip unchanged",
                "Do not rebuild if no changes were found",
            ),
            Field::toggle(
                'y',
                "Yes to all",
                "Answer all prompts with yes (non-interactive)",
            ),
            Field::toggle('P', "Push", "Push new commits to origin"),
            Field::toggle('S', "Don't squash", "Don't squash commits before pushing"),
            Field::text('c', "Comment", "Add a comment to the resulting commit"),
            Field::text(
                'f',
                "Flake",
                "Path to the flake (default: ~/.config/narf/flake)",
            ),
            Field::text(
                'H',
                "Host",
                "NixOS host configuration of the flake (default: hostname)",
            ),
        ];

        let mut app = Self {
            fields,
            inputs: Vec::new(),
            focus: Focus::Options,
            options_state: ListState::default().with_selected(Some(0)),
            inputs_state: ListState::default(),
            mode: Mode::Form,
            status: status::load(""),
            hosts: Hosts::default(),
            command: String::new(),
            error: None,
            quit: false,
        };
        app.hosts.load(app.status.flake.as_deref().ok());
        app.sync_inputs();
        app.update_command();
        app
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            if let Mode::Run(runner) = &mut self.mode {
                runner.poll();
            }
            if self.hosts.loading() {
                self.hosts.poll();
                self.clamp_selection();
            }

            terminal.draw(|frame| ui::draw(frame, self))?;

            if !event::poll(Duration::from_millis(30))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    self.on_key(key, terminal)?
                }
                Event::Resize(cols, rows) => {
                    if let Mode::Run(runner) = &mut self.mode {
                        let area = ui::terminal_area(ratatui::layout::Rect::new(0, 0, cols, rows));
                        runner.resize(area.height, area.width);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn field(&self, flag: char) -> &Field {
        self.fields
            .iter()
            .find(|f| f.flag == flag)
            .expect("unknown flag")
    }

    pub fn toggled(&self, flag: char) -> bool {
        matches!(self.field(flag).value, Value::Toggle(true))
    }

    pub fn text(&self, flag: char) -> &str {
        match &self.field(flag).value {
            Value::Text(text) => text.trim(),
            Value::Toggle(_) => "",
        }
    }

    pub fn selected_field(&self) -> Option<&Field> {
        self.options_state
            .selected()
            .and_then(|i| self.fields.get(i))
    }

    /// The arguments narf will be called with.
    pub fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for field in &self.fields {
            match &field.value {
                Value::Toggle(true) => args.push(format!("-{}", field.flag)),
                Value::Text(text) if !text.trim().is_empty() => {
                    args.push(format!("-{}", field.flag));
                    let text = text.trim();
                    args.push(if field.flag == 'f' {
                        narf::flake::expand_tilde(text)
                    } else {
                        text.to_owned()
                    });
                }
                _ => {}
            }
        }
        if !self.toggled('u') {
            for input in self.inputs.iter().filter(|i| i.selected) {
                args.push("-U".to_owned());
                args.push(input.name.clone());
            }
        }
        args
    }

    fn update_command(&mut self) {
        let mut parts = vec!["narf".to_owned()];
        parts.extend(self.args().iter().map(|arg| shell_quote(arg)));
        self.command = parts.join(" ");
    }

    /// Combinations narf accepts but silently ignores, and an unknown host.
    pub fn hints(&self) -> Vec<String> {
        let mut hints = Vec::new();
        if self.toggled('S') && !self.toggled('P') {
            hints.push("Don't squash has no effect without Push".to_owned());
        }
        if self.toggled('P') && (self.toggled('d') || self.toggled('b')) {
            hints.push("Push is skipped in dry/build-only mode".to_owned());
        }
        if !self.text('c').is_empty() && (self.toggled('d') || self.toggled('b')) {
            hints.push("No commit (and no comment) in dry/build-only mode".to_owned());
        }
        if self.toggled('u') && self.inputs.iter().any(|i| i.selected) {
            hints.push("Update all inputs overrides the input selection".to_owned());
        }
        let host = self.host();
        if !self.hosts.names.is_empty() && !self.hosts.names.iter().any(|name| name == host) {
            hints.push(format!(
                "Host '{host}' is not in the flake's nixosConfigurations"
            ));
        }
        hints
    }

    /// The host narf will build for.
    pub fn host(&self) -> &str {
        match self.text('H') {
            "" => &self.status.default_host,
            host => host,
        }
    }

    /// Entries of the host picker: the hostname default, the flake's hosts
    /// and typing one by hand.
    pub fn host_choices(&self) -> Vec<HostChoice> {
        std::iter::once(HostChoice::Default)
            .chain(self.hosts.names.iter().cloned().map(HostChoice::Host))
            .chain(std::iter::once(HostChoice::Custom))
            .collect()
    }

    fn reload_status(&mut self) {
        self.status = status::load(self.text('f'));
        self.hosts.load(self.status.flake.as_deref().ok());
        self.sync_inputs();
    }

    fn host_field(&mut self) -> &mut String {
        match &mut self
            .fields
            .iter_mut()
            .find(|f| f.flag == 'H')
            .expect("unknown flag")
            .value
        {
            Value::Text(text) => text,
            Value::Toggle(_) => unreachable!("host is a text field"),
        }
    }

    fn open_host_picker(&mut self) {
        let current = self.text('H');
        let choices = self.host_choices();
        let index = choices
            .iter()
            .position(|choice| match choice {
                HostChoice::Default => current.is_empty(),
                HostChoice::Host(name) => name == current,
                HostChoice::Custom => true,
            })
            .unwrap_or(0);
        self.mode = Mode::PickHost(ListState::default().with_selected(Some(index)));
    }

    fn on_host_picker_key(&mut self, key: KeyEvent) {
        let Mode::PickHost(state) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Form,
            KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
            KeyCode::Down | KeyCode::Char('j') => state.select_next(),
            KeyCode::Home | KeyCode::Char('g') => state.select_first(),
            KeyCode::End | KeyCode::Char('G') => state.select_last(),
            KeyCode::Char(' ') | KeyCode::Enter => {
                let selected = state.selected().unwrap_or(0);
                let choice = self.host_choices().into_iter().nth(selected);
                let text = self.host_field();
                self.mode = match choice {
                    Some(HostChoice::Default) => {
                        text.clear();
                        Mode::Form
                    }
                    Some(HostChoice::Host(name)) => {
                        *text = name;
                        Mode::Form
                    }
                    Some(HostChoice::Custom) => Mode::Edit {
                        backup: text.clone(),
                    },
                    None => Mode::Form,
                };
            }
            _ => {}
        }
        self.clamp_selection();
        self.update_command();
    }

    /// Rebuilds the input list from the lock file, keeping the selection.
    fn sync_inputs(&mut self) {
        let previous = std::mem::take(&mut self.inputs);
        self.inputs = self
            .status
            .inputs
            .iter()
            .map(|name| Input {
                name: name.clone(),
                selected: previous.iter().any(|i| i.selected && &i.name == name),
            })
            .collect();
        let selected = self.inputs_state.selected().unwrap_or(0);
        self.inputs_state
            .select((!self.inputs.is_empty()).then(|| selected.min(self.inputs.len() - 1)));
    }

    fn on_key(&mut self, key: KeyEvent, terminal: &mut DefaultTerminal) -> Result<()> {
        match &mut self.mode {
            Mode::Run(runner) => {
                if runner.running() {
                    match key.code {
                        KeyCode::PageUp => runner.scroll(1),
                        KeyCode::PageDown => runner.scroll(-1),
                        _ => runner.send_key(key),
                    }
                } else {
                    match key.code {
                        KeyCode::PageUp => runner.scroll(1),
                        KeyCode::PageDown => runner.scroll(-1),
                        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                            self.mode = Mode::Form;
                            self.reload_status();
                        }
                        _ => {}
                    }
                }
            }
            Mode::Edit { backup } => {
                let backup = backup.clone();
                let Some(Field {
                    value: Value::Text(text),
                    flag,
                    ..
                }) = self
                    .options_state
                    .selected()
                    .and_then(|i| self.fields.get_mut(i))
                else {
                    self.mode = Mode::Form;
                    return Ok(());
                };
                let flag = *flag;
                match key.code {
                    KeyCode::Enter => {
                        self.mode = Mode::Form;
                        if flag == 'f' {
                            self.reload_status();
                        }
                    }
                    KeyCode::Esc => {
                        *text = backup;
                        self.mode = Mode::Form;
                    }
                    KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let keep = text.trim_end().rfind(' ').map_or(0, |i| i + 1);
                        text.truncate(keep);
                    }
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        text.clear()
                    }
                    KeyCode::Char(c) => text.push(c),
                    _ => {}
                }
                self.update_command();
            }
            Mode::PickHost(_) => self.on_host_picker_key(key),
            Mode::Form => {
                self.error = None;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.quit = true
                    }
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Char('l') => {
                        if !self.inputs.is_empty() {
                            self.focus = match self.focus {
                                Focus::Options => Focus::Inputs,
                                Focus::Inputs => Focus::Options,
                            };
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.list_state().select_previous(),
                    KeyCode::Down | KeyCode::Char('j') => self.list_state().select_next(),
                    KeyCode::Home | KeyCode::Char('g') => self.list_state().select_first(),
                    KeyCode::End | KeyCode::Char('G') => self.list_state().select_last(),
                    KeyCode::Char(' ') | KeyCode::Enter => self.activate(),
                    KeyCode::Char('R') => self.reload_status(),
                    KeyCode::Char('r') => self.start(terminal)?,
                    _ => {}
                }
                self.clamp_selection();
                self.update_command();
            }
        }
        Ok(())
    }

    fn list_state(&mut self) -> &mut ListState {
        match self.focus {
            Focus::Options => &mut self.options_state,
            Focus::Inputs => &mut self.inputs_state,
        }
    }

    /// `select_next`/`select_last` may point past the end until the next render.
    fn clamp_selection(&mut self) {
        let clamp = |state: &mut ListState, len: usize| {
            if let Some(i) = state.selected() {
                state.select((len > 0).then(|| i.min(len - 1)));
            }
        };
        clamp(&mut self.options_state, self.fields.len());
        clamp(&mut self.inputs_state, self.inputs.len());
        let host_choices = self.host_choices().len();
        if let Mode::PickHost(state) = &mut self.mode {
            clamp(state, host_choices);
        }
    }

    fn activate(&mut self) {
        match self.focus {
            Focus::Options => {
                if self.selected_field().is_some_and(|f| f.flag == 'H') {
                    self.open_host_picker();
                    return;
                }
                let Some(field) = self
                    .options_state
                    .selected()
                    .and_then(|i| self.fields.get_mut(i))
                else {
                    return;
                };
                match &mut field.value {
                    Value::Toggle(on) => *on = !*on,
                    Value::Text(text) => {
                        self.mode = Mode::Edit {
                            backup: text.clone(),
                        }
                    }
                }
            }
            Focus::Inputs => {
                if let Some(input) = self
                    .inputs_state
                    .selected()
                    .and_then(|i| self.inputs.get_mut(i))
                {
                    input.selected = !input.selected;
                }
            }
        }
    }

    fn start(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let area = ui::terminal_area(terminal.size()?.into());
        let exe = std::env::current_exe().context("could not locate the narf-tui binary")?;
        let mut args = vec![EXEC_FLAG.to_owned()];
        args.extend(self.args());
        match Runner::spawn(&exe, &args, area.height, area.width) {
            Ok(runner) => self.mode = Mode::Run(runner),
            Err(err) => self.error = Some(format!("{err:#}")),
        }
        Ok(())
    }
}

/// Quotes an argument for display, only where the shell would need it.
fn shell_quote(arg: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "-_./=:@+%,".contains(c);
    if !arg.is_empty() && arg.chars().all(safe) {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

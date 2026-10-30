//! Rendering of the option form and the run view.

use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, HighlightSpacing, List, ListItem, Paragraph, Wrap},
};
use tui_term::widget::{Cursor, PseudoTerminal};

use crate::{
    app::{App, Focus, HostChoice, Mode, Value},
    runner::Runner,
    status,
};

const HIGHLIGHT: &str = "› ";
const LABEL_WIDTH: usize = 9;

pub fn draw(frame: &mut Frame, app: &mut App) {
    if let Mode::Run(runner) = &app.mode {
        draw_run(frame, runner, &app.command);
    } else {
        draw_form(frame, app);
    }
}

/// Area of the embedded terminal inside the run view.
pub fn terminal_area(area: Rect) -> Rect {
    let [body, _] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    Block::bordered().inner(body)
}

fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (key, action)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            key.to_string(),
            Style::new().bold().fg(Color::Cyan),
        ));
        spans.push(Span::raw(format!(" {action}")));
    }
    Line::from(spans)
}

fn panel(title: &str, focused: bool) -> Block<'_> {
    let block = Block::bordered().title(format!(" {title} "));
    if focused {
        block.border_style(Style::new().fg(Color::Cyan))
    } else {
        block
    }
}

fn draw_form(frame: &mut Frame, app: &mut App) {
    let editing = matches!(app.mode, Mode::Edit { .. });
    let picking_host = matches!(app.mode, Mode::PickHost(_));
    let hints = app.hints();

    let [main, command, footer] = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [left, right] =
        Layout::horizontal([Constraint::Length(46), Constraint::Min(30)]).areas(main);
    let [options_area, inputs_area] = Layout::vertical([
        Constraint::Length(app.fields.len() as u16 + 2),
        Constraint::Min(3),
    ])
    .areas(left);
    let hint_height = if hints.is_empty() {
        0
    } else {
        hints.len() as u16 + 2
    };
    let [status_area, hints_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(hint_height)]).areas(right);

    // Options
    let placeholder = |flag: char| match flag {
        'f' => "saved flake".to_owned(),
        'H' => app.status.default_host.clone(),
        _ => "none".to_owned(),
    };
    let items: Vec<ListItem> = app
        .fields
        .iter()
        .map(|field| {
            let flag = Span::styled(format!("-{}", field.flag), Style::new().dark_gray());
            let line = match &field.value {
                Value::Toggle(on) => Line::from(vec![
                    if *on {
                        Span::styled("[x] ", Style::new().green().bold())
                    } else {
                        Span::raw("[ ] ")
                    },
                    Span::raw(format!("{:<width$}", field.label, width = LABEL_WIDTH + 13)),
                    flag,
                ]),
                Value::Text(text) => Line::from(vec![
                    Span::raw(format!("{:<LABEL_WIDTH$}", field.label)),
                    if text.is_empty() {
                        Span::styled(
                            format!("({})", placeholder(field.flag)),
                            Style::new().dark_gray(),
                        )
                    } else {
                        Span::styled(text.clone(), Style::new().yellow())
                    },
                ]),
            };
            ListItem::new(line)
        })
        .collect();
    let options_focused = app.focus == Focus::Options;
    let list = List::new(items)
        .block(panel("Options", options_focused))
        .highlight_symbol(HIGHLIGHT)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(if options_focused {
            Style::new().reversed()
        } else {
            Style::new().bold()
        });
    frame.render_stateful_widget(list, options_area, &mut app.options_state);

    if editing
        && let Some(index) = app.options_state.selected()
        && let Value::Text(text) = &app.fields[index].value
    {
        let inner = Block::bordered().inner(options_area);
        let x = inner.x + (HIGHLIGHT.chars().count() + LABEL_WIDTH + text.chars().count()) as u16;
        let y = inner.y + (index - app.options_state.offset()) as u16;
        frame.set_cursor_position(Position::new(x.min(inner.right().saturating_sub(1)), y));
    }

    // Inputs
    let update_all = app.toggled('u');
    let items: Vec<ListItem> = app
        .inputs
        .iter()
        .map(|input| {
            let check = if input.selected && !update_all {
                Span::styled("[x] ", Style::new().green().bold())
            } else {
                Span::raw("[ ] ")
            };
            ListItem::new(Line::from(vec![check, Span::raw(input.name.clone())]))
        })
        .collect();
    let inputs_focused = app.focus == Focus::Inputs;
    let title = if update_all {
        "Update inputs -U (all via -u)"
    } else {
        "Update inputs -U"
    };
    let list = List::new(items)
        .block(panel(title, inputs_focused))
        .style(if update_all {
            Style::new().dark_gray()
        } else {
            Style::new()
        })
        .highlight_symbol(HIGHLIGHT)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(if inputs_focused {
            Style::new().reversed()
        } else {
            Style::new()
        });
    frame.render_stateful_widget(list, inputs_area, &mut app.inputs_state);

    // Status
    frame.render_widget(
        Paragraph::new(status_lines(app))
            .block(panel("Status", false))
            .wrap(Wrap { trim: false }),
        status_area,
    );

    if !hints.is_empty() {
        let lines: Vec<Line> = hints
            .iter()
            .map(|h| Line::from(format!("! {h}")).yellow())
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(panel("Hints", false)),
            hints_area,
        );
    }

    // Command preview
    let help = match app.focus {
        Focus::Options => app.selected_field().map_or("", |f| f.help),
        Focus::Inputs => "Update only the selected inputs before running",
    };
    let lines = vec![
        Line::from(help).dark_gray(),
        Line::from(vec![
            Span::styled("$ ", Style::new().green()),
            Span::raw(app.command.clone()),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(panel("Command", false)),
        command,
    );

    let footer_keys = if let Some(error) = &app.error {
        Line::from(format!("Error: {error}")).red().bold()
    } else if editing {
        keys(&[
            ("Enter", "apply"),
            ("Esc", "cancel"),
            ("C-u", "clear"),
            ("C-⌫", "delete word"),
        ])
    } else if picking_host {
        keys(&[("j/k", "move"), ("Enter", "select"), ("Esc", "cancel")])
    } else {
        keys(&[
            ("j/k", "move"),
            ("Space", "toggle/edit"),
            ("Tab", "options/inputs"),
            ("r", "run narf"),
            ("R", "reload"),
            ("q", "quit"),
        ])
    };
    frame.render_widget(footer_keys, footer);

    if picking_host {
        draw_host_picker(frame, app, main);
    }
}

/// Popup listing the flake's hosts, centered over `area`.
fn draw_host_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    let dim = Style::new().dark_gray();
    let default_host = &app.status.default_host;
    let items: Vec<ListItem> = app
        .host_choices()
        .into_iter()
        .map(|choice| {
            ListItem::new(match choice {
                HostChoice::Default => Line::from(vec![
                    Span::raw("hostname "),
                    Span::styled(format!("({default_host})"), dim),
                ]),
                HostChoice::Host(name) if &name == default_host => {
                    Line::from(vec![Span::raw(name), Span::styled(" (this machine)", dim)])
                }
                HostChoice::Host(name) => Line::from(name),
                HostChoice::Custom => Line::from("custom…").italic(),
            })
        })
        .collect();

    let mut block = panel("Host -H", true);
    let mut width = 40;
    if app.hosts.loading() {
        block = block.title_bottom(Line::from(" evaluating flake… ").yellow());
    } else if let Some(err) = &app.hosts.error {
        width = width.max(err.chars().count() + 4);
        block = block.title_bottom(Line::from(format!(" {err} ")).red());
    }

    let height = (items.len() as u16 + 2).min(area.height);
    let [popup] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::horizontal([Constraint::Length(width as u16)])
        .flex(Flex::Center)
        .areas(popup);

    let list = List::new(items)
        .block(block)
        .highlight_symbol(HIGHLIGHT)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(Style::new().reversed());
    frame.render_widget(Clear, popup);
    if let Mode::PickHost(state) = &mut app.mode {
        frame.render_stateful_widget(list, popup, state);
    }
}

fn status_lines(app: &App) -> Vec<Line<'static>> {
    let status = &app.status;
    let row = |label: &str, value: Span<'static>| {
        Line::from(vec![
            Span::styled(format!("{label:<10}"), Style::new().bold()),
            value,
        ])
    };
    let dim = |text: String| Span::styled(text, Style::new().dark_gray());

    let mut lines = Vec::new();

    match &status.flake {
        Ok(path) => lines.push(row("Flake", Span::raw(status::display_path(path)))),
        Err(err) => {
            lines.push(row("Flake", Span::styled(err.clone(), Style::new().red())));
            return lines;
        }
    }

    let host = app.text('H');
    lines.push(row(
        "Host",
        if host.is_empty() {
            Span::raw(format!("{} (hostname)", status.default_host))
        } else {
            Span::raw(host.to_owned())
        },
    ));
    lines.push(row(
        "Upstream",
        match &status.upstream {
            Some(upstream) => Span::raw(upstream.clone()),
            None => Span::styled("none - pushing will fail", Style::new().yellow()),
        },
    ));
    if let Some(count) = status.unpushed {
        let style = if count > 0 {
            Style::new().yellow()
        } else {
            Style::new()
        };
        lines.push(row(
            "Unpushed",
            Span::styled(format!("{count} commit(s)"), style),
        ));
    }
    if let Some(head) = &status.head {
        lines.push(row("HEAD", Span::raw(head.clone())));
    }

    lines.push(Line::default());
    if status.changes.is_empty() {
        lines.push(row(
            "Changes",
            dim("none - narf will ask before rebuilding anyway".to_owned()),
        ));
    } else {
        lines.push(row(
            "Changes",
            Span::raw(format!(
                "{} file(s), will be staged and committed",
                status.changes.len()
            )),
        ));
        for change in &status.changes {
            let (code, file) = change.split_at(change.len().min(3));
            let color = match code.trim() {
                "??" | "A" => Color::Green,
                "D" => Color::Red,
                _ => Color::Yellow,
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {code:<3}"), Style::new().fg(color)),
                Span::raw(file.to_owned()),
            ]));
        }
    }
    lines
}

fn draw_run(frame: &mut Frame, runner: &Runner, command: &str) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    let (color, state) = match &runner.exit {
        None => (Color::Yellow, "running".to_owned()),
        Some(exit) if exit.success() => (Color::Green, "finished".to_owned()),
        Some(exit) => (
            Color::Red,
            format!("failed (exit code {})", exit.exit_code()),
        ),
    };

    let parser = runner.parser.lock().unwrap();
    let screen = parser.screen();
    let scrolled = screen.scrollback();

    let mut title = vec![
        Span::raw(" "),
        Span::styled(state, Style::new().fg(color).add_modifier(Modifier::BOLD)),
    ];
    if scrolled > 0 {
        title.push(Span::raw(format!(" · scrolled back {scrolled} lines")));
    }
    title.push(Span::raw(" "));

    let block = Block::bordered()
        .border_style(Style::new().fg(color))
        .title(Line::from(title))
        .title_bottom(Line::from(format!(" $ {command} ")).dark_gray());
    let cursor = Cursor::default().visibility(runner.running() && scrolled == 0);
    frame.render_widget(
        PseudoTerminal::new(screen).block(block).cursor(cursor),
        body,
    );

    let footer_keys = if runner.running() {
        keys(&[
            ("PgUp/PgDn", "scroll"),
            ("keys", "go to narf"),
            ("C-c", "abort narf"),
        ])
    } else {
        keys(&[("PgUp/PgDn", "scroll"), ("q/Enter", "back to options")])
    };
    frame.render_widget(footer_keys, footer);
}

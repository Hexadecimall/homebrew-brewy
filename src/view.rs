use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::{
    app::{App, Overlay, Tab},
    model::PackageKind,
};

const BG: Color = Color::Rgb(26, 27, 38);
const SURFACE: Color = Color::Rgb(30, 32, 48);
const SELECTED: Color = Color::Rgb(40, 52, 82);
const BORDER: Color = Color::Rgb(59, 66, 97);
const FG: Color = Color::Rgb(192, 202, 245);
const MUTED: Color = Color::Rgb(86, 95, 137);
const BLUE: Color = Color::Rgb(122, 162, 247);
const CYAN: Color = Color::Rgb(125, 207, 255);
const GREEN: Color = Color::Rgb(158, 206, 106);
const YELLOW: Color = Color::Rgb(224, 175, 104);
const MAGENTA: Color = Color::Rgb(187, 154, 247);

pub fn draw(frame: &mut Frame, app: &App) {
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(FG)),
        frame.area(),
    );
    let page = frame.area().inner(Margin {
        vertical: 0,
        horizontal: if frame.area().width > 90 { 2 } else { 1 },
    });
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(2),
    ])
    .split(page);

    draw_wordmark(frame, app, rows[0]);
    draw_prompt(frame, app, rows[1]);
    if page.width >= 105 {
        let columns = if app.preview_visible {
            Layout::horizontal([
                Constraint::Length(22),
                Constraint::Length(1),
                Constraint::Percentage(49),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(rows[3])
        } else {
            Layout::horizontal([
                Constraint::Length(22),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(rows[3])
        };
        draw_sidebar(frame, app, columns[0]);
        draw_divider(frame, columns[1]);
        draw_results(frame, app, columns[2]);
        if app.preview_visible {
            draw_divider(frame, columns[3]);
            draw_preview(frame, app, columns[4], true);
        }
        frame.render_widget(
            Paragraph::new(Span::styled(
                "full catalog  ·  type to search  ·  ←/→ switch section",
                Style::default().fg(MUTED),
            )),
            rows[2],
        );
    } else {
        draw_filters(frame, app, rows[2]);
        let compact = Layout::vertical([
            Constraint::Min(5),
            Constraint::Length(if app.preview_visible { 7 } else { 0 }),
        ])
        .split(rows[3]);
        draw_results(frame, app, compact[0]);
        if app.preview_visible {
            draw_preview(frame, app, compact[1], false);
        }
    }
    draw_status(frame, app, rows[4]);

    match app.overlay {
        Overlay::Help => draw_help(frame),
        Overlay::None => {}
    }
}

fn draw_divider(frame: &mut Frame, area: Rect) {
    let divider = "│\n".repeat(area.height as usize);
    frame.render_widget(
        Paragraph::new(divider).style(Style::default().fg(BORDER)),
        area,
    );
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let sections = Layout::vertical([
        Constraint::Length(9),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(7),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "PACKAGES",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        sections[0],
    );
    let nav_area = Rect {
        y: sections[0].y.saturating_add(2),
        height: sections[0].height.saturating_sub(2),
        ..sections[0]
    };
    let nav: Vec<ListItem> = Tab::ALL
        .iter()
        .map(|tab| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {:<12}", tab.label().to_ascii_lowercase()),
                    Style::default().fg(if *tab == app.tab { FG } else { MUTED }),
                ),
                Span::styled(
                    tab_count(app, *tab).to_string(),
                    Style::default().fg(if *tab == app.tab { BLUE } else { BORDER }),
                ),
            ]))
        })
        .collect();
    let selected = Tab::ALL.iter().position(|tab| *tab == app.tab).unwrap_or(0);
    let mut nav_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(
        List::new(nav)
            .highlight_symbol("▌")
            .highlight_style(Style::default().fg(FG).add_modifier(Modifier::BOLD)),
        nav_area,
        &mut nav_state,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("SELECTION  {}", app.queue.len()),
            Style::default()
                .fg(if app.queue.is_empty() { MUTED } else { MAGENTA })
                .add_modifier(Modifier::BOLD),
        )),
        sections[1],
    );
    let queue_lines: Vec<Line> = if app.queue.is_empty() {
        vec![Line::from(Span::styled(
            "Nothing selected",
            Style::default().fg(BORDER),
        ))]
    } else {
        app.queue
            .iter()
            .take(sections[2].height as usize)
            .map(|operation| {
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(MAGENTA)),
                    Span::styled(
                        truncate(&sanitize_text(&operation.summary()), 18),
                        Style::default().fg(FG),
                    ),
                ])
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(queue_lines), sections[2]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![key("enter"), hint("  run now")]),
            Line::from(vec![key("tab"), hint("    select")]),
            Line::from(vec![key("ctrl-r"), hint(" refresh")]),
            Line::from(vec![key("alt-p"), hint("  preview")]),
        ]),
        sections[3],
    );
}

fn draw_wordmark(frame: &mut Frame, app: &App, area: Rect) {
    let state = if app.loading {
        Span::styled("syncing  ◌", Style::default().fg(YELLOW))
    } else if app.running {
        Span::styled("running  ●", Style::default().fg(YELLOW))
    } else {
        Span::styled("ready  ●", Style::default().fg(GREEN))
    };
    let mut title = vec![
        Span::styled(
            "  BREWY",
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ──", Style::default().fg(BLUE)),
    ];
    if area.width >= 62 {
        title.push(Span::styled(
            "  homebrew package manager",
            Style::default().fg(MUTED),
        ));
    }
    let logo = Text::from(vec![
        Line::from(title),
        Line::from(Span::styled(
            "  search · select · install",
            Style::default().fg(MUTED),
        )),
    ]);
    frame.render_widget(Paragraph::new(logo), area);
    frame.render_widget(
        Paragraph::new(Line::from(state)).alignment(Alignment::Right),
        area,
    );
}

fn draw_prompt(frame: &mut Frame, app: &App, area: Rect) {
    let input = if app.query.is_empty() {
        vec![
            Span::styled(
                "  › ",
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Search formulae and casks", Style::default().fg(MUTED)),
            Span::styled("█", Style::default().fg(BLUE)),
        ]
    } else {
        vec![
            Span::styled(
                "  › ",
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.query,
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(BLUE)),
        ]
    };
    frame.render_widget(
        Paragraph::new(Line::from(input)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .border_style(Style::default().fg(if app.query.is_empty() { BORDER } else { BLUE }))
                .style(Style::default().bg(SURFACE)),
        ),
        area,
    );
}

fn draw_filters(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (index, tab) in Tab::ALL.iter().enumerate() {
        let active = *tab == app.tab;
        spans.push(Span::styled(
            format!(
                " {}  {} ",
                tab.label().to_ascii_lowercase(),
                tab_count(app, *tab)
            ),
            Style::default()
                .fg(if active { BG } else { MUTED })
                .bg(if active { BLUE } else { BG })
                .add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
        if index + 1 < Tab::ALL.len() {
            spans.push(Span::raw("  "));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn tab_count(app: &App, tab: Tab) -> usize {
    match tab {
        Tab::Browse => app.catalog.packages.len(),
        Tab::Installed => app.catalog.installed_count(),
        Tab::Outdated => app.catalog.outdated_count(),
        Tab::Casks => app
            .catalog
            .packages
            .iter()
            .filter(|package| package.kind == PackageKind::Cask)
            .count(),
        Tab::Taps => app.catalog.taps.len(),
    }
}

fn draw_results(frame: &mut Frame, app: &App, area: Rect) {
    if app.tab == Tab::Taps {
        draw_taps(frame, app, area);
        return;
    }
    let indices = app.filtered_indices();
    let staged = app.staged_names();
    let name_width = area.width.saturating_sub(20).clamp(12, 52) as usize;
    let items: Vec<ListItem> = indices
        .iter()
        .map(|index| {
            let package = &app.catalog.packages[*index];
            let queued = staged.contains(package.name.as_str());
            let status = if package.outdated {
                "upgrade"
            } else if package.installed {
                "installed"
            } else {
                package.kind.label()
            };
            let status_color = if package.outdated {
                YELLOW
            } else if package.installed {
                GREEN
            } else {
                MUTED
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    if queued { " ● " } else { "   " },
                    Style::default().fg(MAGENTA),
                ),
                Span::styled(
                    format!(
                        "{:<name_width$}",
                        truncate(&sanitize_text(&package.name), name_width.saturating_sub(2))
                    ),
                    Style::default().fg(FG),
                ),
                Span::styled(format!("{status:<12}"), Style::default().fg(status_color)),
                Span::styled(
                    package
                        .installed_version
                        .as_deref()
                        .or(package.latest_version.as_deref())
                        .map(sanitize_text)
                        .unwrap_or_default(),
                    Style::default().fg(MUTED),
                ),
            ]))
        })
        .collect();
    let title = Line::from(vec![
        Span::styled(
            format!("  {} matches", indices.len()),
            Style::default().fg(MUTED),
        ),
        Span::styled(
            format!("  ·  sorted by {}", app.sort.label()),
            Style::default().fg(BORDER),
        ),
    ]);
    let list = List::new(items)
        .block(
            Block::default()
                .title_bottom(title)
                .padding(Padding::vertical(1)),
        )
        .highlight_symbol("›")
        .highlight_style(
            Style::default()
                .bg(SELECTED)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    let mut state =
        ListState::default().with_selected((!indices.is_empty()).then_some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_taps(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .catalog
        .taps
        .iter()
        .map(|tap| {
            ListItem::new(Line::from(vec![
                Span::styled("   ◉  ", Style::default().fg(MAGENTA)),
                Span::styled(sanitize_text(tap), Style::default().fg(FG)),
            ]))
        })
        .collect();
    let mut state = ListState::default().with_selected((!items.is_empty()).then_some(app.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().padding(Padding::vertical(1)))
            .highlight_symbol("›")
            .highlight_style(Style::default().bg(SELECTED).add_modifier(Modifier::BOLD)),
        area,
        &mut state,
    );
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect, wide: bool) {
    let block = Block::default()
        .borders(if wide { Borders::NONE } else { Borders::TOP })
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(
                if wide {
                    "PACKAGE INFO"
                } else {
                    " package info "
                },
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " alt-p hide  ·  alt-j/k scroll ",
                Style::default().fg(MUTED),
            ),
        ]))
        .padding(Padding::horizontal(if wide { 2 } else { 1 }));
    if app.tab == Tab::Taps {
        let selected = app
            .catalog
            .taps
            .get(app.selected)
            .map(String::as_str)
            .unwrap_or("No tap selected");
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    sanitize_text(selected),
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "Third-party Homebrew package repository",
                    Style::default().fg(MUTED),
                )),
            ])
            .block(block),
            area,
        );
        return;
    }
    let Some(package) = app.selected_package() else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                if app.loading {
                    "Reading Homebrew metadata…"
                } else {
                    "No matching package"
                },
                Style::default().fg(MUTED),
            ))
            .block(block),
            area,
        );
        return;
    };
    let state = if package.outdated {
        "update available"
    } else if package.installed {
        "installed"
    } else {
        "available"
    };
    let state_color = if package.outdated {
        YELLOW
    } else if package.installed {
        GREEN
    } else {
        BLUE
    };
    let version = package
        .latest_version
        .as_deref()
        .or(package.installed_version.as_deref())
        .unwrap_or("version loading…");
    let description = package
        .description
        .as_deref()
        .unwrap_or("Loading package description…");
    let action = if package.outdated {
        "upgrade"
    } else if package.installed {
        "uninstall"
    } else {
        "install"
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                sanitize_text(&package.name),
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {state}"), Style::default().fg(state_color)),
            Span::styled(
                format!("  {}  {}", package.kind.label(), sanitize_text(version)),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(""),
        Line::from(sanitize_text(description)),
        Line::from(""),
        Line::from(Span::styled(
            "DETAILS",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("version     ", Style::default().fg(MUTED)),
            Span::styled(sanitize_text(version), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            Span::styled("type        ", Style::default().fg(MUTED)),
            Span::styled(package.kind.label(), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            Span::styled("license     ", Style::default().fg(MUTED)),
            Span::styled(
                sanitize_text(package.license.as_deref().unwrap_or("—")),
                Style::default().fg(FG),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "HOMEPAGE",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![Span::styled(
            sanitize_text(package.homepage.as_deref().unwrap_or("—")),
            Style::default().fg(CYAN),
        )]),
        Line::from(""),
        Line::from(Span::styled(
            "ACTION",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            key("enter"),
            Span::styled(format!("  {action} now"), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            key("tab"),
            Span::styled("    toggle selection", Style::default().fg(FG)),
        ]),
    ];
    if !app.log.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            if app.running {
                "ACTIVITY  ●"
            } else {
                "LAST ACTIVITY"
            },
            Style::default()
                .fg(if app.running { YELLOW } else { MUTED })
                .add_modifier(Modifier::BOLD),
        )));
        lines.extend(app.log.iter().rev().take(6).rev().map(|line| {
            Line::from(Span::styled(
                truncate(&sanitize_log(line), area.width.saturating_sub(6) as usize),
                Style::default().fg(MUTED),
            ))
        }));
    }
    let details = Text::from(lines);
    frame.render_widget(
        Paragraph::new(details)
            .scroll((app.preview_scroll, 0))
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let controls = Line::from(vec![
        key("type"),
        hint(" search  "),
        key("tab"),
        hint(" select  "),
        key("enter"),
        hint(" apply  "),
        key("←→"),
        hint(" filter  "),
        key("?"),
        hint(" help  "),
        key("ctrl-q"),
        hint(" quit"),
    ]);
    frame.render_widget(Paragraph::new(controls), area);
    let right = if app.queue.is_empty() {
        app.status.clone()
    } else {
        format!("{} selected", app.queue.len())
    };
    let status_area = Rect {
        x: area.x,
        y: area.y.saturating_add(1),
        width: area.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(truncate(&sanitize_text(&right), 38))
            .style(Style::default().fg(if app.queue.is_empty() { MUTED } else { MAGENTA }))
            .alignment(Alignment::Right),
        status_area,
    );
}

fn key(value: &'static str) -> Span<'static> {
    Span::styled(value, Style::default().fg(FG).add_modifier(Modifier::BOLD))
}

fn hint(value: &'static str) -> Span<'static> {
    Span::styled(value, Style::default().fg(MUTED))
}

fn dialog(title: &'static str) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE).fg(FG))
        .padding(Padding::horizontal(2))
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height.min(area.height)),
        Constraint::Fill(1),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width.min(area.width)),
        Constraint::Fill(1),
    ])
    .split(vertical[1])[1]
}

fn draw_help(frame: &mut Frame) {
    let area = centered(frame.area(), 72, 22);
    frame.render_widget(Clear, area);
    let lines = vec![
        help("type", "search immediately"),
        help("backspace / esc", "edit or clear search"),
        help("↑ / ↓", "move selection"),
        help("tab / space", "select several packages"),
        help("enter", "run the selected package actions"),
        help("←/→", "change package filter"),
        help("alt-p", "show or hide package preview"),
        help("alt-j / alt-k", "scroll package preview"),
        help("ctrl-a", "run the selection queue"),
        help("ctrl-r", "refresh package state"),
        help("ctrl-u", "update Homebrew metadata"),
        help("ctrl-s", "cycle sorting"),
        help("ctrl-p", "pin or unpin an installed formula"),
        help("ctrl-q", "quit after active operations finish"),
        Line::from(""),
        Line::from(Span::styled(
            "Package output stays inline; no operation dialogs appear.",
            Style::default().fg(YELLOW),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(dialog("keyboard")), area);
}

fn help(key: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{key:<22}"),
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        ),
        Span::styled(description.to_string(), Style::default().fg(FG)),
    ])
}

fn sanitize_log(line: &str) -> String {
    sanitize_text(line)
}

fn sanitize_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| match ch {
            '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect()
}

fn truncate(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut result: String = value.chars().take(max.saturating_sub(1)).collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn terminal_control_characters_are_removed_from_external_text() {
        assert_eq!(
            sanitize_text("safe\u{1b}]52;secret\u{7} text\r"),
            "safe]52;secret text"
        );
    }

    #[test]
    fn layout_renders_at_small_standard_and_wide_sizes() {
        let (tx, _) = mpsc::channel();
        let app = App::new(tx);
        for (width, height) in [(20, 8), (80, 24), (120, 35), (200, 60)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    #[test]
    fn zero_width_truncation_is_empty() {
        assert_eq!(truncate("brewy", 0), "");
    }
}

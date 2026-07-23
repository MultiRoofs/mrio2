use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::path::Path;

use mrio_core::io;
use mrio_core::model::{CityJsonDocument, OutputFormat};
use mrio_core::ops;
use mrio_core::stats::{compute_stats, FileStats};

const OPERATION_NAMES: &[&str] = &[
    "Attribute: add roof area",
    "Attribute: add volume",
    "Attribute: delete",
    "Attribute: rename",
    "Attributes: add from CSV",
    "CRS: set EPSG",
    "Roofer → MultiRoofs",
    "Validate schema",
    "Save",
];

#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    RemoveAttr {
        attrs: Vec<String>,
        selected: usize,
    },
    RenamePick {
        attrs: Vec<String>,
        selected: usize,
    },
    RenameInput {
        old_name: String,
        input: String,
        cursor: usize,
    },
    AddCsv {
        input: String,
        cursor: usize,
    },
    Save {
        input: String,
        cursor: usize,
        format: OutputFormat,
    },
    Message {
        text: String,
        is_error: bool,
    },
    Validation {
        text: String,
        has_errors: bool,
        has_warnings: bool,
    },
    ConfirmQuit,
    ConfirmOverwrite {
        path: String,
        format: OutputFormat,
    },
    EpsgInput {
        input: String,
        cursor: usize,
    },
}

pub struct App {
    doc: CityJsonDocument,
    stats: FileStats,
    input_path: String,
    output_format: OutputFormat,
    selected_operation: usize,
    dialog: Option<Dialog>,
    should_quit: bool,
    modified: bool,
    focus: usize,
    right_scroll: usize,
}

impl App {
    fn new(doc: CityJsonDocument, input_path: &str, output_format: OutputFormat) -> Self {
        let stats = compute_stats(&doc);
        App {
            doc,
            stats,
            input_path: input_path.to_string(),
            output_format,
            selected_operation: 0,
            dialog: None,
            should_quit: false,
            modified: false,
            focus: 0,
            right_scroll: 0,
        }
    }

    fn default_output_path(&self) -> String {
        let p = Path::new(&self.input_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let ext = self.output_format.extension();
        if let Some(parent) = p.parent() {
            format!("{}/{}.modified.{}", parent.display(), stem, ext)
        } else {
            format!("{}.modified.{}", stem, ext)
        }
    }

    fn refresh_stats(&mut self) {
        self.stats = compute_stats(&self.doc);
    }
}

pub fn run(
    doc: CityJsonDocument,
    input_path: &str,
    output_format: OutputFormat,
    output_path: Option<String>,
) -> Result<(), String> {
    let mut terminal = ratatui::init();
    let mut app = App::new(doc, input_path, output_format);

    if let Some(path) = output_path {
        let fmt = app.output_format;
        match io::write_file(&path, &app.doc, fmt) {
            Ok(()) => {
                app.dialog = Some(Dialog::Message {
                    text: format!("Saved to '{}'", path),
                    is_error: false,
                });
            }
            Err(e) => {
                app.dialog = Some(Dialog::Message {
                    text: format!("Save failed: {}", e),
                    is_error: true,
                });
            }
        }
    }

    while !app.should_quit {
        terminal
            .draw(|f| render(f, &mut app))
            .map_err(|e| format!("Render error: {}", e))?;

        if let Err(e) = handle_events(&mut app) {
            return Err(e);
        }
    }

    ratatui::restore();
    Ok(())
}

fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let top_rect = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    render_title_bar(frame, top_rect[0], app);

    let middle = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(top_rect[1]);

    render_left_panel(frame, middle[0], app);
    render_right_panel(frame, middle[1], app);

    render_bottom_bar(frame, top_rect[2], app);

    if let Some(ref dialog) = app.dialog.clone() {
        render_dialog(frame, area, dialog, app);
    }
}

fn render_title_bar(frame: &mut Frame, area: Rect, app: &App) {
    let fmt = app.output_format.label();
    let modified = if app.modified { " *" } else { "" };
    let title = format!(
        " mrio v{} — {}{}  [{}]  (CityJSON v{})",
        env!("CARGO_PKG_VERSION"),
        Path::new(&app.input_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&app.input_path),
        modified,
        fmt,
        app.stats.version,
    );
    let block = Block::default()
        .style(Style::default().fg(Color::White).bg(Color::Blue))
        .borders(Borders::NONE);
    let text = Paragraph::new(Text::styled(title, Style::default().fg(Color::White)))
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(text, area);
}

fn render_bottom_bar(frame: &mut Frame, area: Rect, app: &App) {
    let help = if app.dialog.is_some() {
        " [Enter] confirm  [Esc] cancel  [↑↓] navigate"
    } else if app.focus == 0 {
        " [↑↓] nav ops  [Enter] select  [Tab] to overview  [s] save  [q] quit"
    } else {
        " [↑↓] scroll overview  [PgUp/PgDn] jump  [Tab] to ops  [s] save  [q] quit"
    };
    let block = Block::default()
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .borders(Borders::NONE);
    let text = Paragraph::new(Text::styled(help, Style::default().fg(Color::White)))
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(text, area);
}

fn render_left_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Operations ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let block = if app.focus == 0 {
        block.border_style(Style::default().fg(Color::Yellow))
    } else {
        block
    };

    let items: Vec<ListItem> = OPERATION_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let prefix = if i == app.selected_operation {
                "► "
            } else {
                "  "
            };
            let style = if i == app.selected_operation {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, name),
                style,
            )))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(list, area);
}

fn render_right_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" File Overview ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let active_style = if app.focus == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let block = block.border_style(active_style);

    let stats = &app.stats;
    let mut items: Vec<ListItem> = vec![];

    macro_rules! bold {
        ($s:expr) => {
            Span::styled(
                $s.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            )
        };
    }
    macro_rules! gray {
        ($s:expr) => {
            Span::styled($s.to_string(), Style::default().fg(Color::Gray))
        };
    }

    items.push(ListItem::new(Line::from(vec![
        bold!("Format:     "),
        Span::raw(format!("{} v{}", stats.format_name, stats.version)),
    ])));
    items.push(ListItem::new(Line::from("")));

    items.push(ListItem::new(Line::from(vec![
        bold!("CRS:        "),
        Span::raw(&stats.crs),
    ])));

    if stats.extensions.is_empty() {
        items.push(ListItem::new(Line::from(bold!("Extensions:"))));
        items.push(ListItem::new(Line::from("  none")));
    } else {
        items.push(ListItem::new(Line::from(bold!(format!(
            "Extensions ({}):",
            stats.extensions.len()
        )))));
        for (name, url) in &stats.extensions {
            items.push(ListItem::new(Line::from(format!("  \u{2022} {}", name))));
            items.push(ListItem::new(Line::from(vec![
                gray!("    "),
                Span::raw(url),
            ])));
        }
    }
    items.push(ListItem::new(Line::from("")));

    items.push(ListItem::new(Line::from(vec![
        bold!("Objects:    "),
        Span::raw(stats.total_objects.to_string()),
    ])));
    items.push(ListItem::new(Line::from(vec![
        bold!("Vertices:   "),
        Span::raw(stats.total_vertices.to_string()),
    ])));
    items.push(ListItem::new(Line::from("")));

    if !stats.object_type_counts.is_empty() {
        items.push(ListItem::new(Line::from(bold!("Object types:"))));
        for (ty, count) in &stats.object_type_counts {
            items.push(ListItem::new(Line::from(format!(
                "  \u{2022} {}: {}",
                ty, count
            ))));
        }
        if !stats.other_object_types.is_empty() {
            for (ty, count) in &stats.other_object_types {
                items.push(ListItem::new(Line::from(format!(
                    "  \u{2022} {}: {}",
                    ty, count
                ))));
            }
        }
        items.push(ListItem::new(Line::from("")));
    }

    if !stats.attribute_inventory.is_empty() {
        items.push(ListItem::new(Line::from(bold!(format!(
            "Attributes ({}):",
            stats.attribute_inventory.len()
        )))));
        for (attr, count, sample) in &stats.attribute_inventory {
            let pct = if stats.total_objects > 0 {
                (*count as f64 / stats.objects_with_attrs.max(1) as f64 * 100.0) as usize
            } else {
                0
            };
            items.push(ListItem::new(Line::from(vec![
                Span::raw(format!("  \u{2022} {}: ", attr)),
                gray!(sample),
                Span::raw(format!("  ({} obj, {}%)", count, pct)),
            ])));
        }
        items.push(ListItem::new(Line::from("")));
    }

    let max_scroll = items.len().saturating_sub(1);
    if app.right_scroll > max_scroll {
        app.right_scroll = max_scroll;
    }
    let scroll = app.right_scroll;
    let mut list_state = ratatui::widgets::ListState::default().with_selected(Some(scroll));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_height = (area.height * percent_y) / 100;
    let popup_layout = Layout::vertical([
        Constraint::Length((area.height.saturating_sub(popup_height)) / 2),
        Constraint::Length(popup_height),
        Constraint::Min(0),
    ])
    .split(area);
    let popup_width = (area.width * percent_x) / 100;
    Layout::horizontal([
        Constraint::Length((area.width.saturating_sub(popup_width)) / 2),
        Constraint::Length(popup_width),
        Constraint::Min(0),
    ])
    .split(popup_layout[1])[1]
}

fn render_dialog(frame: &mut Frame, area: Rect, dialog: &Dialog, _app: &App) {
    let dialog_area = centered_rect(area, 60, 50);
    frame.render_widget(Clear, dialog_area);

    match dialog {
        Dialog::RemoveAttr { attrs, selected } => {
            let block = Block::default()
                .title(" Remove Attribute ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let items: Vec<ListItem> = attrs
                .iter()
                .map(|name| ListItem::new(format!("  {}", name)))
                .collect();
            let mut state = ratatui::widgets::ListState::default().with_selected(Some(*selected));
            let list = List::new(items)
                .block(block)
                .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black));
            frame.render_stateful_widget(list, dialog_area, &mut state);
        }
        Dialog::RenamePick { attrs, selected } => {
            let block = Block::default()
                .title(" Pick Attribute to Rename ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let items: Vec<ListItem> = attrs
                .iter()
                .map(|name| ListItem::new(format!("  {}", name)))
                .collect();
            let mut state = ratatui::widgets::ListState::default().with_selected(Some(*selected));
            let list = List::new(items)
                .block(block)
                .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black));
            frame.render_stateful_widget(list, dialog_area, &mut state);
        }
        Dialog::RenameInput {
            old_name,
            input,
            cursor,
        } => {
            let block = Block::default()
                .title(format!(" New name for '{}' ", old_name))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let display = if input.is_empty() {
                " (type new name and press Enter)".to_string()
            } else {
                input.clone()
            };
            let text = Paragraph::new(Text::styled(
                format!("> {}", display),
                Style::default().fg(Color::White),
            ))
            .block(block);
            frame.render_widget(text, dialog_area);
            frame.set_cursor_position((
                dialog_area.x + 3 + (*cursor as u16).min(input.len() as u16),
                dialog_area.y + 1,
            ));
        }
        Dialog::AddCsv { input, cursor } => {
            let block = Block::default()
                .title(" CSV File Path ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let display = if input.is_empty() {
                " (type path and press Enter)".to_string()
            } else {
                input.clone()
            };
            let text = Paragraph::new(Text::styled(
                format!("> {}", display),
                Style::default().fg(Color::White),
            ))
            .block(block);
            frame.render_widget(text, dialog_area);
            frame.set_cursor_position((
                dialog_area.x + 3 + (*cursor as u16).min(input.len() as u16),
                dialog_area.y + 1,
            ));
        }
        Dialog::Save {
            input,
            cursor,
            format,
        } => {
            let block = Block::default()
                .title(" Save As ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let fmt_str = format!("[{}]", format.label());
            let display = if input.is_empty() {
                " (type path and press Enter)".to_string()
            } else {
                input.clone()
            };
            let text = Paragraph::new(Text::from(vec![
                Line::from(format!("Path: {}", display)),
                Line::from(Span::styled(fmt_str, Style::default().fg(Color::Cyan))),
                Line::from(" [f] toggle format"),
            ]))
            .block(block);
            frame.render_widget(text, dialog_area);
            frame.set_cursor_position((
                dialog_area.x + 7 + (*cursor as u16).min(input.len() as u16),
                dialog_area.y + 1,
            ));
        }
        Dialog::ConfirmOverwrite {
            ref path,
            format: _,
        } => {
            let block = Block::default()
                .title(" File Exists ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Red));
            let text = Text::from(vec![
                Line::from(format!("'{}'", path)),
                Line::from("already exists. Overwrite?"),
                Line::from(""),
                Line::from(Span::styled(
                    "[Enter] overwrite   [Esc] cancel",
                    Style::default().fg(Color::Gray),
                )),
            ]);
            let paragraph = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, dialog_area);
        }
        Dialog::EpsgInput { input, cursor } => {
            let block = Block::default()
                .title(" Set EPSG Code ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Yellow));
            let display = if input.is_empty() {
                " (type EPSG code and press Enter, e.g. 28992)".to_string()
            } else {
                input.clone()
            };
            let text = Paragraph::new(Text::styled(
                format!("> {}", display),
                Style::default().fg(Color::White),
            ))
            .block(block);
            frame.render_widget(text, dialog_area);
            frame.set_cursor_position((
                dialog_area.x + 3 + (*cursor as u16).min(input.len() as u16),
                dialog_area.y + 1,
            ));
        }
        Dialog::ConfirmQuit => {
            let block = Block::default()
                .title(" Quit? ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(Color::Red));
            let text = Text::from(vec![
                Line::from("Unsaved changes will be lost."),
                Line::from(""),
                Line::from(Span::styled(
                    "[Enter] discard & quit   [Esc] cancel",
                    Style::default().fg(Color::Gray),
                )),
            ]);
            let paragraph = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, dialog_area);
        }
        Dialog::Message { text, is_error } => {
            let color = if *is_error { Color::Red } else { Color::Green };
            let block = Block::default()
                .title(if *is_error { " Error " } else { " Success " })
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(color));
            let paragraph = Paragraph::new(Text::styled(
                text.clone(),
                Style::default().fg(Color::White),
            ))
            .block(block)
            .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, dialog_area);
        }
        Dialog::Validation {
            text,
            has_errors,
            has_warnings,
        } => {
            let (title, color) = if *has_errors {
                (" Validation — Errors ", Color::Red)
            } else if *has_warnings {
                (" Validation — Warnings ", Color::Yellow)
            } else {
                (" Validation — OK ", Color::Green)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .style(Style::default().fg(color));
            let paragraph = Paragraph::new(Text::styled(
                text.clone(),
                Style::default().fg(Color::White),
            ))
            .block(block)
            .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, dialog_area);
        }
    }
}

fn handle_events(app: &mut App) -> Result<(), String> {
    if let Event::Key(key) = event::read().map_err(|e| format!("Event error: {}", e))? {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if let Some(ref dialog) = app.dialog.clone() {
            return handle_dialog_key(app, dialog.clone(), key);
        }

        match key.code {
            KeyCode::Char('q') => {
                if app.modified {
                    app.dialog = Some(Dialog::ConfirmQuit);
                } else {
                    app.should_quit = true;
                }
            }
            KeyCode::Esc => {
                if !app.modified {
                    app.should_quit = true;
                }
            }
            KeyCode::Char('s') => {
                let default = app.default_output_path();
                app.dialog = Some(Dialog::Save {
                    input: default,
                    cursor: 0,
                    format: app.output_format,
                });
            }
            KeyCode::Tab => {
                app.focus = (app.focus + 1) % 2;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.focus == 0 {
                    app.selected_operation = app.selected_operation.saturating_sub(1);
                } else {
                    app.right_scroll = app.right_scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.focus == 0 {
                    app.selected_operation =
                        (app.selected_operation + 1).min(OPERATION_NAMES.len() - 1);
                } else {
                    app.right_scroll = app.right_scroll.saturating_add(1);
                }
            }
            KeyCode::PageDown => {
                if app.focus == 1 {
                    app.right_scroll = app.right_scroll.saturating_add(10);
                }
            }
            KeyCode::PageUp => {
                if app.focus == 1 {
                    app.right_scroll = app.right_scroll.saturating_sub(10);
                }
            }
            KeyCode::Enter => match app.selected_operation {
                0 => {
                    let report = ops::add_roof_area(&mut app.doc);
                    app.modified = true;
                    app.refresh_stats();
                    app.dialog = Some(Dialog::Message {
                        text: report.summary,
                        is_error: report.is_error,
                    });
                }
                1 => {
                    let report = ops::add_volume(&mut app.doc);
                    app.modified = true;
                    app.refresh_stats();
                    app.dialog = Some(Dialog::Message {
                        text: report.summary,
                        is_error: report.is_error,
                    });
                }
                2 => {
                    let attrs = collect_attribute_names(app);
                    if attrs.is_empty() {
                        app.dialog = Some(Dialog::Message {
                            text: "No attributes found in any CityObject.".to_string(),
                            is_error: true,
                        });
                    } else {
                        app.dialog = Some(Dialog::RemoveAttr { attrs, selected: 0 });
                    }
                }
                3 => {
                    let attrs = collect_attribute_names(app);
                    if attrs.is_empty() {
                        app.dialog = Some(Dialog::Message {
                            text: "No attributes found in any CityObject.".to_string(),
                            is_error: true,
                        });
                    } else {
                        app.dialog = Some(Dialog::RenamePick { attrs, selected: 0 });
                    }
                }
                4 => {
                    app.dialog = Some(Dialog::AddCsv {
                        input: String::new(),
                        cursor: 0,
                    });
                }
                5 => {
                    app.dialog = Some(Dialog::EpsgInput {
                        input: String::new(),
                        cursor: 0,
                    });
                }
                6 => {
                    let report = ops::roofer2multiroofs(&mut app.doc);
                    app.modified = true;
                    app.refresh_stats();
                    app.dialog = Some(Dialog::Message {
                        text: report.summary,
                        is_error: report.is_error,
                    });
                }
                7 => {
                    let report = ops::validate_schema(&app.doc);
                    let has_warnings = report.summary.contains("[warning]");
                    app.dialog = Some(Dialog::Validation {
                        text: report.summary,
                        has_errors: report.is_error,
                        has_warnings,
                    });
                }
                8 => {
                    let default = app.default_output_path();
                    app.dialog = Some(Dialog::Save {
                        input: default,
                        cursor: 0,
                        format: app.output_format,
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}

fn handle_dialog_key(app: &mut App, dialog: Dialog, key: event::KeyEvent) -> Result<(), String> {
    match (dialog, key.code) {
        (Dialog::RemoveAttr { attrs, selected }, KeyCode::Up) => {
            let new_sel = selected.saturating_sub(1);
            app.dialog = Some(Dialog::RemoveAttr {
                attrs,
                selected: new_sel,
            });
        }
        (Dialog::RemoveAttr { attrs, selected }, KeyCode::Down) => {
            let new_sel = (selected + 1).min(attrs.len().saturating_sub(1));
            app.dialog = Some(Dialog::RemoveAttr {
                attrs,
                selected: new_sel,
            });
        }
        (
            Dialog::RemoveAttr {
                ref attrs,
                selected,
            },
            KeyCode::Enter,
        ) => {
            let name = attrs[selected].clone();
            let report = ops::remove_attribute(&mut app.doc, &name);
            app.modified = true;
            app.refresh_stats();
            app.dialog = Some(Dialog::Message {
                text: report.summary,
                is_error: report.is_error,
            });
        }
        (Dialog::RemoveAttr { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::RenamePick { attrs, selected }, KeyCode::Up) => {
            let new_sel = selected.saturating_sub(1);
            app.dialog = Some(Dialog::RenamePick {
                attrs,
                selected: new_sel,
            });
        }
        (Dialog::RenamePick { attrs, selected }, KeyCode::Down) => {
            let new_sel = (selected + 1).min(attrs.len().saturating_sub(1));
            app.dialog = Some(Dialog::RenamePick {
                attrs,
                selected: new_sel,
            });
        }
        (
            Dialog::RenamePick {
                ref attrs,
                selected,
            },
            KeyCode::Enter,
        ) => {
            let old_name = attrs[selected].clone();
            app.dialog = Some(Dialog::RenameInput {
                old_name,
                input: String::new(),
                cursor: 0,
            });
        }
        (Dialog::RenamePick { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::RenameInput { .. }, KeyCode::Char(c)) => {
            if c == '\t' {
                return Ok(());
            }
            if c.is_ascii_graphic() || c == ' ' || c == '_' || c == '-' {
                if let Some(Dialog::RenameInput {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.dialog
                {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
        (Dialog::RenameInput { .. }, KeyCode::Backspace) => {
            if let Some(Dialog::RenameInput {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor > 0 {
                    *cursor -= 1;
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::RenameInput { .. }, KeyCode::Delete) => {
            if let Some(Dialog::RenameInput {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor < input.len() {
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::RenameInput { .. }, KeyCode::Left) => {
            if let Some(Dialog::RenameInput { ref mut cursor, .. }) = app.dialog {
                *cursor = cursor.saturating_sub(1);
            }
        }
        (Dialog::RenameInput { .. }, KeyCode::Right) => {
            if let Some(Dialog::RenameInput {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                *cursor = (*cursor + 1).min(input.len());
            }
        }
        (
            Dialog::RenameInput {
                ref old_name,
                ref input,
                ..
            },
            KeyCode::Enter,
        ) => {
            if input.is_empty() {
                app.dialog = Some(Dialog::Message {
                    text: "New name cannot be empty.".to_string(),
                    is_error: true,
                });
            } else {
                let new_name = input.clone();
                let report = ops::rename_attribute(&mut app.doc, old_name, &new_name);
                app.modified = true;
                app.refresh_stats();
                app.dialog = Some(Dialog::Message {
                    text: report.summary,
                    is_error: report.is_error,
                });
            }
        }
        (Dialog::RenameInput { .. }, KeyCode::Esc) => {
            let attrs = collect_attribute_names(app);
            app.dialog = Some(Dialog::RenamePick { attrs, selected: 0 });
        }

        (Dialog::AddCsv { .. }, KeyCode::Char(c)) => {
            if c == 'f' || c == '\t' {
                return Ok(());
            }
            if c.is_ascii_graphic()
                || c == ' '
                || c == '_'
                || c == '-'
                || c == '.'
                || c == '/'
                || c == '\\'
                || c == ':'
            {
                if let Some(Dialog::AddCsv {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.dialog
                {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
        (Dialog::AddCsv { .. }, KeyCode::Backspace) => {
            if let Some(Dialog::AddCsv {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor > 0 {
                    *cursor -= 1;
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::AddCsv { .. }, KeyCode::Delete) => {
            if let Some(Dialog::AddCsv {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor < input.len() {
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::AddCsv { .. }, KeyCode::Left) => {
            if let Some(Dialog::AddCsv { ref mut cursor, .. }) = app.dialog {
                *cursor = cursor.saturating_sub(1);
            }
        }
        (Dialog::AddCsv { .. }, KeyCode::Right) => {
            if let Some(Dialog::AddCsv {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                *cursor = (*cursor + 1).min(input.len());
            }
        }
        (Dialog::AddCsv { ref input, .. }, KeyCode::Enter) => {
            if input.is_empty() {
                app.dialog = Some(Dialog::Message {
                    text: "CSV path cannot be empty.".to_string(),
                    is_error: true,
                });
            } else {
                let path = input.clone();
                let csv_content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        app.dialog = Some(Dialog::Message {
                            text: format!("Failed to read CSV: {}", e),
                            is_error: true,
                        });
                        return Ok(());
                    }
                };
                let report = ops::add_attributes_from_csv(&mut app.doc, &csv_content);
                app.modified = true;
                app.refresh_stats();
                app.dialog = Some(Dialog::Message {
                    text: report.summary,
                    is_error: report.is_error,
                });
            }
        }
        (Dialog::AddCsv { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::Save { .. }, KeyCode::Char('f')) => {
            let current = app.dialog.as_ref().and_then(|d| {
                if let Dialog::Save { format, .. } = d {
                    Some(*format)
                } else {
                    None
                }
            });
            if let Some(old_format) = current {
                let new_format = match old_format {
                    OutputFormat::CityJSON => OutputFormat::CityJSONSeq,
                    OutputFormat::CityJSONSeq => OutputFormat::CityJSON,
                };
                let old_ext = if new_format == OutputFormat::CityJSON {
                    ".city.jsonl"
                } else {
                    ".city.json"
                };
                let new_ext = new_format.extension();
                if let Some(Dialog::Save { format, input, .. }) = &mut app.dialog {
                    *format = new_format;
                    if input.ends_with(old_ext) {
                        let new_input = format!("{}.{}", input.trim_end_matches(old_ext), new_ext);
                        *input = new_input;
                    }
                }
            }
        }
        (Dialog::Save { .. }, KeyCode::Char(c)) => {
            if c.is_ascii_graphic()
                || c == ' '
                || c == '_'
                || c == '-'
                || c == '.'
                || c == '/'
                || c == '\\'
                || c == ':'
            {
                if let Some(Dialog::Save {
                    ref mut input,
                    ref mut cursor,
                    ..
                }) = app.dialog
                {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
        (Dialog::Save { .. }, KeyCode::Backspace) => {
            if let Some(Dialog::Save {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor > 0 {
                    *cursor -= 1;
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::Save { .. }, KeyCode::Delete) => {
            if let Some(Dialog::Save {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                if *cursor < input.len() {
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::Save { .. }, KeyCode::Left) => {
            if let Some(Dialog::Save { ref mut cursor, .. }) = app.dialog {
                *cursor = cursor.saturating_sub(1);
            }
        }
        (Dialog::Save { .. }, KeyCode::Right) => {
            if let Some(Dialog::Save {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                *cursor = (*cursor + 1).min(input.len());
            }
        }
        (
            Dialog::Save {
                ref input, format, ..
            },
            KeyCode::Enter,
        ) => {
            if input.is_empty() {
                app.dialog = Some(Dialog::Message {
                    text: "Path cannot be empty.".to_string(),
                    is_error: true,
                });
            } else if std::path::Path::new(input).exists() {
                app.dialog = Some(Dialog::ConfirmOverwrite {
                    path: input.clone(),
                    format,
                });
            } else {
                match io::write_file(input, &app.doc, format) {
                    Ok(()) => {
                        app.modified = false;
                        app.dialog = Some(Dialog::Message {
                            text: format!("Saved to '{}'", input),
                            is_error: false,
                        });
                    }
                    Err(e) => {
                        app.dialog = Some(Dialog::Message {
                            text: format!("Save failed: {}", e),
                            is_error: true,
                        });
                    }
                }
            }
        }
        (Dialog::Save { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::EpsgInput { .. }, KeyCode::Char(c)) => {
            if c == '\t' {
                return Ok(());
            }
            if c.is_ascii_digit() {
                if let Some(Dialog::EpsgInput {
                    ref mut input,
                    ref mut cursor,
                }) = app.dialog
                {
                    input.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
        (Dialog::EpsgInput { .. }, KeyCode::Backspace) => {
            if let Some(Dialog::EpsgInput {
                ref mut input,
                ref mut cursor,
            }) = app.dialog
            {
                if *cursor > 0 {
                    *cursor -= 1;
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::EpsgInput { .. }, KeyCode::Delete) => {
            if let Some(Dialog::EpsgInput {
                ref mut input,
                ref mut cursor,
            }) = app.dialog
            {
                if *cursor < input.len() {
                    input.remove(*cursor);
                }
            }
        }
        (Dialog::EpsgInput { .. }, KeyCode::Left) => {
            if let Some(Dialog::EpsgInput { ref mut cursor, .. }) = app.dialog {
                *cursor = cursor.saturating_sub(1);
            }
        }
        (Dialog::EpsgInput { .. }, KeyCode::Right) => {
            if let Some(Dialog::EpsgInput {
                ref mut input,
                ref mut cursor,
                ..
            }) = app.dialog
            {
                *cursor = (*cursor + 1).min(input.len());
            }
        }
        (Dialog::EpsgInput { ref input, .. }, KeyCode::Enter) => {
            if input.is_empty() {
                app.dialog = Some(Dialog::Message {
                    text: "EPSG code cannot be empty.".to_string(),
                    is_error: true,
                });
            } else {
                let epsg = input.clone();
                let report = ops::set_crs(&mut app.doc, &epsg);
                app.modified = true;
                app.refresh_stats();
                app.dialog = Some(Dialog::Message {
                    text: report.summary,
                    is_error: report.is_error,
                });
            }
        }
        (Dialog::EpsgInput { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::ConfirmOverwrite { ref path, format }, KeyCode::Enter) => {
            match io::write_file(path, &app.doc, format) {
                Ok(()) => {
                    app.modified = false;
                    app.dialog = Some(Dialog::Message {
                        text: format!("Saved to '{}'", path),
                        is_error: false,
                    });
                }
                Err(e) => {
                    app.dialog = Some(Dialog::Message {
                        text: format!("Save failed: {}", e),
                        is_error: true,
                    });
                }
            }
        }
        (Dialog::ConfirmOverwrite { .. }, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::ConfirmQuit, KeyCode::Enter | KeyCode::Char('q')) => {
            app.should_quit = true;
        }
        (Dialog::ConfirmQuit, KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::Message { .. }, KeyCode::Enter | KeyCode::Esc) => {
            app.dialog = None;
        }

        (Dialog::Validation { .. }, KeyCode::Enter | KeyCode::Esc) => {
            app.dialog = None;
        }

        _ => {}
    }
    Ok(())
}

fn collect_attribute_names(app: &App) -> Vec<String> {
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (_id, obj) in io::get_all_city_objects(&app.doc) {
        if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
            for key in attrs.keys() {
                names.insert(key.clone());
            }
        }
    }
    names.into_iter().collect()
}

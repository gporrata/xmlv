use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{App, Mode};
use crate::tree::NodeKind;

const INDENT: &str = "  ";

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let viewport_height = chunks[0].height as usize;
    app.viewport_height = viewport_height.saturating_sub(2); // border top/bottom

    // Build list items
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .enumerate()
        .map(|(vis_pos, &node_idx)| {
            let node = &app.nodes[node_idx];
            let indent = INDENT.repeat(node.depth);
            let is_cursor = vis_pos == app.cursor;
            let is_match = app.search_matches.contains(&vis_pos);

            let collapse_icon = if node.has_children {
                if node.collapsed { "▶ " } else { "▼ " }
            } else {
                "  "
            };


            let (tag_style, val_style) = node_colors(&node.kind);

            let mut spans = vec![
                Span::raw(indent),
                Span::styled(collapse_icon, Style::default().fg(Color::DarkGray)),
            ];

            match &node.kind {
                NodeKind::Element { name, attrs } => {
                    spans.push(Span::styled("<", tag_style));
                    spans.push(Span::styled(name.clone(), tag_style.add_modifier(Modifier::BOLD)));
                    for (k, v) in attrs {
                        spans.push(Span::styled(" ", Style::default()));
                        spans.push(Span::styled(k.clone(), Style::default().fg(Color::Cyan)));
                        spans.push(Span::styled("=\"", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(v.clone(), Style::default().fg(Color::Yellow)));
                        spans.push(Span::styled("\"", Style::default().fg(Color::DarkGray)));
                    }
                    spans.push(Span::styled(">", tag_style));
                    if node.collapsed {
                        spans.push(Span::styled(" …", Style::default().fg(Color::DarkGray)));
                    }
                }
                NodeKind::CloseElement { name } => {
                    spans.push(Span::styled(format!("</{name}>"), tag_style));
                }
                NodeKind::Text(t) => {
                    spans.push(Span::styled(t.clone(), val_style));
                }
                NodeKind::Comment(t) => {
                    spans.push(Span::styled(format!("<!-- {t} -->"), val_style));
                }
                NodeKind::CData(t) => {
                    spans.push(Span::styled(format!("<![CDATA[{t}]]>"), val_style));
                }
            }

            let line = Line::from(spans);
            let mut item = ListItem::new(line);

            if is_cursor {
                item = item.style(Style::default().bg(Color::Rgb(40, 40, 60)));
            } else if is_match {
                item = item.style(Style::default().bg(Color::Rgb(60, 50, 20)));
            }

            item
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.cursor));

    let block = Block::default()
        .title(" xmlv ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default()); // highlight handled manually

    // We scroll manually via scroll_offset using ListState offset
    // ratatui ListState doesn't have set_offset in all versions; use select + manual offset
    *list_state.offset_mut() = app.scroll_offset;
    list_state.select(Some(app.cursor));

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Status bar
    let status = match app.mode {
        Mode::Search => {
            format!("/ {}", app.search_query)
        }
        Mode::Normal => {
            let pos = if app.visible.is_empty() {
                "0/0".to_string()
            } else {
                format!("{}/{}", app.cursor + 1, app.visible.len())
            };
            let matches_info = if !app.search_matches.is_empty() {
                format!(
                    "  [{}/{}] \"{}\"",
                    app.search_match_pos + 1,
                    app.search_matches.len(),
                    app.search_query
                )
            } else if !app.search_query.is_empty() {
                format!("  (no matches) \"{}\"", app.search_query)
            } else {
                String::new()
            };
            format!(" {pos}{matches_info}   j/k:move  h/l:collapse/expand  /:search  n/N:next/prev  q:quit")
        }
    };

    let status_style = match app.mode {
        Mode::Search => Style::default().fg(Color::Black).bg(Color::Yellow),
        Mode::Normal => Style::default().fg(Color::DarkGray).bg(Color::Rgb(20, 20, 30)),
    };

    let status_bar = Paragraph::new(status).style(status_style);
    f.render_widget(status_bar, chunks[1]);
}

fn node_colors(kind: &NodeKind) -> (Style, Style) {
    match kind {
        NodeKind::Element { .. } => (
            Style::default().fg(Color::Rgb(100, 180, 255)),
            Style::default(),
        ),
        NodeKind::CloseElement { .. } => (
            Style::default().fg(Color::Rgb(80, 140, 200)),
            Style::default(),
        ),
        NodeKind::Text(_) => (
            Style::default().fg(Color::White),
            Style::default().fg(Color::White),
        ),
        NodeKind::Comment(_) => (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::DarkGray),
        ),
        NodeKind::CData(_) => (
            Style::default().fg(Color::Magenta),
            Style::default().fg(Color::Magenta),
        ),
    }
}

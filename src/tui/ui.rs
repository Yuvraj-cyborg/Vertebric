use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Layout: Header (3), Main (flex), Footer (3), Input (3) if active
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Chat Area
            Constraint::Length(3), // Footer (Tokens/Cost)
            Constraint::Length(3), // Input area
        ])
        .split(size);

    draw_header(f, app, chunks[0]);
    draw_chat(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
    draw_input(f, app, chunks[3]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " Vertebric v{} | Provider: {} | Model: {}",
        env!("CARGO_PKG_VERSION"),
        app.config.provider.provider_display(),
        app.config.model
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(block, area);
}

fn draw_chat(f: &mut Frame, app: &mut App, area: Rect) {
    let mut text_lines = Vec::new();

    // In a real TUI we'd do smart text wrapping, Markdown formatting, and scrolling.
    // For now we just push the raw conversation history.
    for msg in &app.messages {
        let role_str = match msg.role {
            crate::types::Role::User => "USER",
            crate::types::Role::Assistant => "ASSISTANT",
            crate::types::Role::System => "SYSTEM",
            crate::types::Role::Tool => "TOOL",
        };
        let role_color = if matches!(msg.role, crate::types::Role::User) { Color::Green } else { Color::Blue };
        
        text_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", role_str), Style::default().fg(role_color).add_modifier(Modifier::BOLD)),
        ]));
        
        // Parse simple text blocks
        match &msg.content {
            crate::types::MessageContent::Text(text) => {
                for line in text.lines() {
                    text_lines.push(Line::from(line.to_string()));
                }
            }
            crate::types::MessageContent::Blocks(blocks) => {
                for b in blocks {
                    if let crate::types::ContentBlock::Text { text } = b {
                        for line in text.lines() {
                            text_lines.push(Line::from(line.to_string()));
                        }
                    }
                }
            }
        }
        text_lines.push(Line::from(""));
    }

    // Currently streaming text
    if !app.streaming_text.is_empty() {
        text_lines.push(Line::from(vec![
            Span::styled("ASSISTANT: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        ]));
        for line in app.streaming_text.lines() {
            text_lines.push(Line::from(line.to_string()));
        }
    }

    // Active tools
    for tool in app.active_tools.values() {
        text_lines.push(Line::from(vec![
            Span::styled(format!("🚀 RUNNING TOOL: {}", tool.name), Style::default().fg(Color::Yellow)),
        ]));
    }

    // Scroll to bottom (very rough)
    let paragraph = Paragraph::new(text_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    // We'd add Scroll state here properly, but letting it render for now
    f.render_widget(paragraph, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_str = if app.is_running { "Running..." } else { "Idle." };
    let summary = format!(
        "Turn {} | Status: {} | Tokens: {} in ({}%) / {} out | Cost: ${:.4}",
        app.current_turn, status_str, app.tokens_in, app.context_pct, app.tokens_out, app.cost_usd
    );

    let mut style = Style::default().fg(Color::DarkGray);
    if app.error.is_some() {
        style = Style::default().fg(Color::Red);
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .style(style);
    
    let text = if let Some(e) = &app.error {
        format!("ERROR: {}", e)
    } else {
        summary
    };

    f.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let width = area.width.max(3) - 3;
    let scroll = app.user_input.visual_scroll(width as usize);
    let input_str = app.user_input.value();
    
    let p = Paragraph::new(input_str)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Input (Press Enter to send)"));
    f.render_widget(p, area);
    
    // Position cursor
    if !app.is_running {
        f.set_cursor_position((
            area.x + 1 + (app.user_input.visual_cursor().max(scroll) - scroll) as u16,
            area.y + 1,
        ));
    }
}

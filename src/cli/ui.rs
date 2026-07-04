use std::iter::repeat_n;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Clear, Paragraph, RenderDirection, Sparkline},
};

use crate::cli::app::{App, Screen, StreamState};

pub fn render(app: &mut App, frame: &mut Frame) {
    let constraints = [Constraint::Length(1), Constraint::Fill(1)];
    let layout = Layout::vertical(constraints).spacing(1);
    let [top, first] = frame.area().layout(&layout);

    let stream_status = match app.stream_state() {
        StreamState::Playing { name } => Span::from(name).bold(),
        StreamState::Error { message } => Span::styled(
            format!("Stream error: {message}"),
            Style::default().fg(Color::Red),
        )
        .bold(),
        StreamState::Paused => Span::from("No Station Playing").bold(),
    };
    let title = Line::from_iter([
        stream_status,
        Span::from("  Press 'h' for help"),
        Span::from(format!("  Volume: {}%", app.volume())),
    ]);
    frame.render_widget(title.centered(), top);

    render_sparkline(app, frame, first);
    render_search_screen(app, frame);
    render_help_screen(app, frame);
    render_exit_screen(app, frame);
}

fn render_exit_screen(app: &mut App, frame: &mut Frame) {
    if let Screen::Quit = app.screen {
        let title = Line::from(" Exit ".bold());
        let instructions = Line::from(vec![" Y".yellow(), "es ".into()]);

        let no_instruction = Line::from(vec![" N".red().bold(), "o ".into()]);
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(instructions.left_aligned())
            .title_bottom(no_instruction.right_aligned())
            .border_set(border::THICK);

        let counter_text = Text::from(vec![Line::from(vec![
            "Are you sure you would like to quit? ".into(),
        ])]);

        let exit_paragraph = Paragraph::new(counter_text).centered().block(block);

        let clear_area = centered_rect(42, 22, frame.area());
        frame.render_widget(Clear, clear_area); //this clears the entire screen and anything already drawn

        let area = centered_rect(40, 20, frame.area());
        frame.render_widget(exit_paragraph, area);
    }
}

fn render_help_screen(app: &mut App, frame: &mut Frame) {
    if let Screen::Help = app.screen {
        let title = Line::from(" Help ".bold());
        let title_bottom = Line::from(" Esc to close ".bold());
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(title_bottom.right_aligned())
            .border_set(border::THICK);

        let content = Text::from(vec![
            Line::from("h        open/close help"),
            Line::from("f        search stations"),
            Line::from("Up/Down  move selection"),
            Line::from("Enter    play selected station"),
            Line::from("+ / -    volume"),
            Line::from("m        mute"),
            Line::from("q        quit"),
            Line::from("Esc      close help"),
        ]);

        let help_paragraph = Paragraph::new(content).block(block);

        let clear_area = centered_rect(50, 42, frame.area());
        frame.render_widget(Clear, clear_area);

        let area = centered_rect(48, 40, frame.area());
        frame.render_widget(help_paragraph, area);
    }
}

fn render_search_screen(app: &mut App, frame: &mut Frame) {
    if let Screen::Search = app.screen {
        let title = Line::from(" Search ".bold());
        let search_query = app.search_query().to_string();
        let instructions = if search_query.is_empty() {
            Line::from("Type to search".gray())
        } else {
            Line::from(search_query.white())
        };

        let content = match app.radio_state() {
            crate::radio::RadioState::Pending => {
                vec![instructions, Line::from("Loading stations...".blue())]
            }
            crate::radio::RadioState::Error(_) => vec![
                instructions,
                Line::from("An error occurred loading stations".red()),
            ],
            crate::radio::RadioState::Complete(api_stations) => {
                let mut v = vec![instructions];
                let mut iter = api_stations.into_iter().skip(app.song_selected());

                v.extend(
                    (&mut iter)
                        .take(1)
                        .map(|s| Line::from(s.name.white()).on_green()),
                );
                v.extend(iter.map(|s| Line::from(s.name.green())));
                v
            }
        };

        let title_bottom = Line::from(" Esc to close search ".bold());
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(title_bottom.right_aligned())
            .border_set(border::THICK);
        let search_paragraph = Paragraph::new(content).centered().block(block);
        let clear_area = centered_rect(42, 22, frame.area());
        frame.render_widget(Clear, clear_area); //this clears the entire screen and anything already drawn

        let area = centered_rect(40, 20, frame.area());
        frame.render_widget(search_paragraph, area);
    }
}

/// Render a sparkline with some sample data.
pub fn render_sparkline(app: &mut App, frame: &mut Frame, area: Rect) {
    let bar_count = area.width as usize / 2;

    app.compute_bars(bar_count);

    let data = app
        .bars()
        .iter()
        .flat_map(|item| repeat_n(item, 2))
        .map(|&b| (b * 100.0).round() as u64)
        .collect::<Vec<_>>();

    let sparkline = Sparkline::default()
        .data(&data)
        .max(100)
        .direction(RenderDirection::LeftToRight)
        .style(Color::Cyan);

    frame.render_widget(sparkline, area);
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}

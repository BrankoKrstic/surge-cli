use std::iter::repeat_n;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, RenderDirection, Sparkline},
};

use crate::cli::app::App;

pub fn render(app: &mut App, frame: &mut Frame) {
    let constraints = [Constraint::Length(1), Constraint::Fill(1)];
    let layout = Layout::vertical(constraints).spacing(1);
    let [top, first] = frame.area().layout(&layout);

    let title = Line::from_iter([
        Span::from("Sparkline Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_sparkline(app, frame, first);
}

/// Render a sparkline with some sample data.
pub fn render_sparkline(app: &mut App, frame: &mut Frame, area: Rect) {
    let bar_count = area.width as usize / 2;

    app.compute_bars(bar_count);

    let data = app
        .bars()
        .iter()
        .map(|item| repeat_n(item, 2))
        .flatten()
        .map(|&b| (b * 100.0).round() as u64)
        .collect::<Vec<_>>();

    let sparkline = Sparkline::default()
        .data(&data)
        .max(100)
        .direction(RenderDirection::LeftToRight)
        .style(Color::Cyan);

    frame.render_widget(sparkline, area);
}

/// Render a sin wave based on the current frame count.
pub fn render_sin_wave(frame: &mut Frame, area: Rect) {
    let phase_shift = frame.count() as f64 * 0.2;
    let data: Vec<u64> = (0..area.width)
        .map(|v| {
            let angle = f64::from(v) * 0.5 + phase_shift;
            ((angle.sin() * 3.0 + 3.0) * 10.0).round() as u64
        })
        .collect();

    let sparkline = Sparkline::default()
        .data(&data)
        .max(100)
        .direction(RenderDirection::RightToLeft)
        .style(Style::default().magenta().on_black())
        .absent_value_style(Color::Red);

    frame.render_widget(sparkline, area);
}

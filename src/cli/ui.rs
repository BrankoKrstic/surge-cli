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

    render_sparkline(frame, first, app.freqs());
}

/// Render a sparkline with some sample data.
pub fn render_sparkline(frame: &mut Frame, area: Rect, freqs: &[f64]) {
    let chunk_size = freqs.len() / area.width as usize;
    let norm_factor = 1.0 / freqs.len() as f64;

    let mut data = freqs.iter().map(|f| f.log10());

    let data = (0..area.width)
        .map(|_| data.by_ref().take(chunk_size).map(|f| f).sum())
        .collect::<Vec<f64>>();

    let max = data.iter().copied().reduce(|a, b| a.max(b)).unwrap();

    let data = data
        .into_iter()
        .map(|num| (num * 150.0).round() as u64)
        .collect::<Vec<_>>();

    let sparkline = Sparkline::default()
        .data(&data)
        .max(200)
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

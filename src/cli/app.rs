use std::{io, iter::repeat_n};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::processor::ProcessorReader;

pub static FREQUENCY_COUNT: usize = 4096;

pub struct App {
    processor_reader: ProcessorReader,
    pub freq: FrequencyState,
    exit: bool,
}

pub struct FrequencyState {
    pub frequencies: [f64; FREQUENCY_COUNT],
    pub prev_bars: Vec<f32>,
}

impl App {
    pub fn new(reader: ProcessorReader) -> Self {
        Self {
            processor_reader: reader,
            freq: FrequencyState {
                frequencies: [0.0; FREQUENCY_COUNT],
                prev_bars: vec![],
            },
            exit: false,
        }
    }
    pub fn tick(&mut self) {
        self.freq.frequencies = self.processor_reader.query_frequencies();
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event);
            }
            _ => {}
        }
        Ok(())
    }
    pub fn bars(&self) -> &[f32] {
        &self.freq.prev_bars[..]
    }
    pub fn compute_bars(&mut self, bar_count: usize) {
        let bars = normalize_freqs(self, bar_count);
        smooth_bars(&mut self.freq.prev_bars, &bars[..], 1.0 / 60.0);
    }
    pub fn should_quit(&self) -> bool {
        self.exit
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.quit(),
            _ => {}
        }
    }
    pub fn quit(&mut self) {
        self.exit = true;
    }
}

fn smooth_bars(previous: &mut Vec<f32>, target: &[f32], dt: f32) {
    if previous.len() != target.len() {
        *previous = vec![0.0; target.len()];
    }

    let attack_tau = 0.035; // seconds, smaller = faster rise
    let release_tau = 0.180; // seconds, larger = slower fall

    for (prev, &next) in previous.iter_mut().zip(target) {
        let tau = if next > *prev {
            attack_tau
        } else {
            release_tau
        };

        let alpha = 1.0 - (-dt / tau).exp();
        *prev += (next - *prev) * alpha;
    }
}

fn normalize_freqs(app: &App, bar_count: usize) -> Vec<f32> {
    let freqs = app.freq.frequencies;
    let norm_factor = 2.0 / freqs.len() as f64;

    let floor_db = -80.0;
    let ceil_db = 00.0;
    let magnitudes = &freqs[..FREQUENCY_COUNT / 2];
    let magnitudes = magnitudes
        .iter()
        .map(|f| (f * norm_factor) as f32)
        .collect::<Vec<_>>();

    let mut hz_per_bin = 44100.0 / FREQUENCY_COUNT as f32;
    let min_hz = 100.0f32;
    let max_hz = 15000.0f32;

    let log_min = min_hz.ln();
    let log_max = max_hz.ln();

    let mut bars = vec![0.0; bar_count];

    for bar_index in 0..bar_count {
        let t0 = bar_index as f32 / bar_count as f32;
        let t1 = (bar_index + 1) as f32 / bar_count as f32;

        let low_hz = (log_min + (log_max - log_min) * t0).exp();
        let high_hz = (log_min + (log_max - log_min) * t1).exp();

        let start_bin = (low_hz / hz_per_bin).floor() as usize;
        let end_bin = (high_hz / hz_per_bin).ceil() as usize;

        let start_bin = start_bin.min(magnitudes.len() - 1);
        let end_bin = end_bin.min(magnitudes.len());

        if start_bin >= end_bin {
            bars[bar_index] = magnitudes[start_bin];
            continue;
        }

        // Average power, then convert back to amplitude.
        let mut power_sum = 0.0;
        let mut count = 0;

        for &mag in &magnitudes[start_bin..end_bin] {
            power_sum += mag * mag;
            count += 1;
        }

        bars[bar_index] = (power_sum / count as f32).sqrt();
    }

    bars.into_iter()
        .map(|a| 20.0 * a.max(1e-8).log10())
        .map(|db| ((db - floor_db) / (ceil_db - floor_db)).clamp(0.0, 1.0))
        .collect()
}

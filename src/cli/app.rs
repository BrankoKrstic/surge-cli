use crate::{
    controller::{AudioController, Md},
    processor::ProcessorReader,
    radio::{RadioApiFetcher, RadioState},
};

pub static FREQUENCY_COUNT: usize = 4096;

pub struct App {
    processor_reader: ProcessorReader,
    pub freq: FrequencyState,
    pub screen: Screen,
    volume: u32,
    exit: bool,
    search_query: String,
    radio: RadioApiFetcher,
    selected_idx: usize,
    audio_controller: AudioController,
    muted: bool,
    sample_rate: u32,
}

pub struct FrequencyState {
    pub frequencies: [f64; FREQUENCY_COUNT],
    pub prev_bars: Vec<f32>,
}
pub enum StreamState {
    Playing { name: String },
    Error { message: String },
    Paused,
}
pub enum Direction {
    Up,
    Down,
}
impl App {
    pub fn new(
        reader: ProcessorReader,
        audio_controller: AudioController,
        sample_rate: u32,
    ) -> Self {
        Self {
            processor_reader: reader,
            freq: FrequencyState {
                frequencies: [0.0; FREQUENCY_COUNT],
                prev_bars: vec![],
            },
            exit: false,
            volume: 100,
            screen: Screen::Main,
            search_query: String::new(),
            radio: RadioApiFetcher::new(),
            selected_idx: 0,
            audio_controller,
            muted: false,
            sample_rate,
        }
    }
    pub fn volume(&self) -> u32 {
        self.volume
    }
    pub fn stream_state(&self) -> StreamState {
        match self.audio_controller.stream_metadata() {
            Some(Ok(md)) => StreamState::Playing {
                name: md.station_name,
            },
            Some(Err(err)) => StreamState::Error { message: err },
            None => StreamState::Paused,
        }
    }
    pub fn search_query(&self) -> &str {
        &self.search_query[..]
    }
    pub fn push_char(&mut self, c: char) {
        self.selected_idx = 0;
        self.search_query.push(c);
        self.radio.query(&self.search_query[..]);
    }
    pub fn song_selected(&self) -> usize {
        self.selected_idx
    }
    pub fn change_station(&mut self) {
        let RadioState::Complete(api_stations) = self.radio_state() else {
            return;
        };
        let Some(station) = api_stations.get(self.selected_idx) else {
            return;
        };

        self.audio_controller.load_stream(
            station.url_resolved.to_string(),
            Md::new(station.name.clone()),
        );
    }
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.audio_controller
            .set_volume(if self.muted { 0 } else { self.volume });
    }
    pub fn pop_char(&mut self) {
        self.selected_idx = 0;
        if self.search_query.pop().is_some() {
            self.radio.query(&self.search_query[..]);
        }
    }
    pub fn volume_up(&mut self) {
        if self.muted {
            self.muted = false;
        }
        self.volume = (self.volume + 5).min(150);
        self.audio_controller.set_volume(self.volume);
    }
    pub fn volume_down(&mut self) {
        if self.muted {
            self.muted = false
        }
        self.volume = self.volume.saturating_sub(5);
        self.audio_controller.set_volume(self.volume);
    }
    pub fn search_radio_station(&mut self, name: &str) {
        self.radio.query(name);
    }
    pub fn radio_state(&mut self) -> RadioState {
        self.radio.poll_state()
    }
    pub fn tick(&mut self) {
        self.audio_controller.poll_events();
        self.freq.frequencies = self.processor_reader.query_frequencies(1.0 / 60.0);
    }
    pub fn move_cursor(&mut self, direction: Direction) {
        if let RadioState::Complete(x) = self.radio_state()
            && !x.is_empty()
        {
            match direction {
                Direction::Up => {
                    self.selected_idx = self.selected_idx.wrapping_sub(1).min(x.len() - 1)
                }
                Direction::Down => self.selected_idx = (self.selected_idx + 1) % x.len(),
            }
        }
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
    pub fn quit(&mut self) {
        self.exit = true;
    }
}

#[derive(Copy, Clone)]
pub enum Screen {
    Main,
    Search,
    Quit,
    Help,
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

    let hz_per_bin = app.sample_rate as f32 / FREQUENCY_COUNT as f32;
    let min_hz = 100.0f32;
    let max_hz = 15000.0f32;

    let log_min = min_hz.ln();
    let log_max = max_hz.ln();

    let mut bars = vec![0.0; bar_count];

    for (bar_index, bar) in bars.iter_mut().enumerate() {
        let t0 = bar_index as f32 / bar_count as f32;
        let t1 = (bar_index + 1) as f32 / bar_count as f32;

        let low_hz = (log_min + (log_max - log_min) * t0).exp();
        let high_hz = (log_min + (log_max - log_min) * t1).exp();

        let start_bin = (low_hz / hz_per_bin).floor() as usize;
        let end_bin = (high_hz / hz_per_bin).ceil() as usize;

        let start_bin = start_bin.min(magnitudes.len() - 1);
        let end_bin = end_bin.min(magnitudes.len());

        if start_bin >= end_bin {
            *bar = magnitudes[start_bin];
            continue;
        }

        // Average power, then convert back to amplitude.
        let mut power_sum = 0.0;
        let mut count = 0;

        for &mag in &magnitudes[start_bin..end_bin] {
            power_sum += mag * mag;
            count += 1;
        }

        *bar = (power_sum / count as f32).sqrt();
    }

    bars.into_iter()
        .map(|a| 20.0 * a.max(1e-8).log10())
        .map(|db| ((db - floor_db) / (ceil_db - floor_db)).clamp(0.0, 1.0))
        .collect()
}

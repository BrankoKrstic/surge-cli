use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use ringbuf::{
    HeapRb,
    traits::{Consumer, RingBuffer},
};

use crate::fft;

#[derive(Debug)]
pub struct AudioChunk {
    start_sample: u64,
    len: usize,
    data: [f32; 1024],
}

pub struct Processor {
    playback_counter: Arc<AtomicU64>,
}

pub struct ProcessorReader {
    inner: Arc<Mutex<HeapRb<AudioChunk>>>,
    playback_counter: Arc<AtomicU64>,
    last_written: u64,
    visualisation_buffer: VecDeque<f32>,
    frequency: u32,
}

impl ProcessorReader {
    pub fn query_frequencies(&mut self, dt: f32) -> [f64; 4096] {
        let playback = self.playback_counter.load(Ordering::Relaxed);
        let mut lock = self.inner.lock().unwrap();

        if playback == self.last_written {
            let frames_dropped = (dt * self.frequency as f32) as usize;

            for _ in 0..frames_dropped {
                self.visualisation_buffer.push_back(0.0);
            }
        }
        while playback > self.last_written {
            let next_chunk = lock.try_pop();

            if let Some(next_chunk) = next_chunk {
                for i in 0..next_chunk.len {
                    self.visualisation_buffer.push_back(next_chunk.data[i]);
                }
                self.last_written = next_chunk.start_sample + next_chunk.len as u64;
            } else {
                break;
            }
        }
        drop(lock);

        let buf_offset = self.last_written.saturating_sub(playback);

        while self.visualisation_buffer.len() as u64 > 8192 + buf_offset {
            self.visualisation_buffer.pop_front();
        }

        let mut c1_buf = [0.0f64; 4096];
        let mut c2_buf = [0.0f64; 4096];
        let mut out = [0.0f64; 4096];

        for (i, (c1, c2)) in self
            .visualisation_buffer
            .iter()
            .step_by(2)
            .zip(self.visualisation_buffer.iter().skip(1).step_by(2))
            .take(4096)
            .enumerate()
        {
            c1_buf[i] = *c1 as f64;
            c2_buf[i] = *c2 as f64;
        }

        let c1_freq = fft::fft(&c1_buf);
        let c2_freq = fft::fft(&c2_buf);

        for i in 0..out.len() {
            let c1 = c1_freq[i].norm();
            let c2 = c2_freq[i].norm();
            out[i] = ((c1 * c1 + c2 * c2) * 0.5).sqrt();
        }

        out
    }
    pub fn set_frequency(&mut self, freq: u32) {
        self.frequency = freq;
    }
}

pub struct ProcessorWriter {
    bytes_written: u64,
    inner: Arc<Mutex<HeapRb<AudioChunk>>>,
}

impl ProcessorWriter {
    pub fn write_bytes(&mut self, frames: &[f32]) {
        let (chunks, rest) = frames.as_chunks::<1024>();

        let mut lock = self.inner.lock().unwrap();

        for chunk in chunks {
            let audio_chunk = AudioChunk {
                start_sample: self.bytes_written,
                len: 1024,
                data: *chunk,
            };
            self.bytes_written += 1024;
            lock.push_overwrite(audio_chunk);
        }
        if !rest.is_empty() {
            let mut audio_chunk = AudioChunk {
                start_sample: self.bytes_written,
                len: rest.len(),
                data: [0.0; 1024],
            };
            audio_chunk.data[0..rest.len()].copy_from_slice(rest);

            self.bytes_written += rest.len() as u64;
            lock.push_overwrite(audio_chunk);
        }
    }
}

impl Processor {
    pub fn new(playback_counter: Arc<AtomicU64>) -> Self {
        Self { playback_counter }
    }

    pub fn split(self) -> (ProcessorReader, ProcessorWriter) {
        let inner = Arc::new(Mutex::new(HeapRb::new(1000)));
        let reader = ProcessorReader {
            frequency: 44100,
            inner: inner.clone(),
            playback_counter: self.playback_counter,
            last_written: 0,
            visualisation_buffer: VecDeque::from([0.0; 9192]),
        };
        let writer = ProcessorWriter {
            inner,
            bytes_written: 0,
        };
        (reader, writer)
    }
}

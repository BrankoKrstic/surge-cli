use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use cpal::{
    Device, Sample, SampleFormat, StreamConfig, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::signal::Signal;

pub struct Playback {
    device: Device,
    config: SupportedStreamConfig,
    read_buf: rtrb::Consumer<f32>,
    playback_done: Signal,
    playback_counter: Arc<AtomicU64>,
}

impl Playback {
    pub fn new(
        read_buf: rtrb::Consumer<f32>,
        playback_done: Signal,
        playback_counter: Arc<AtomicU64>,
    ) -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");

        let supported_config = device
            .default_output_config()
            .expect("no output config available");

        Self {
            device,
            config: supported_config,
            read_buf,
            playback_done,
            playback_counter,
        }
    }
    pub fn run(mut self) {
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        // Choose format and build stream
        let stream = match self.config.sample_format() {
            SampleFormat::F32 => self.device.build_output_stream(
                &self.config.config(),
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let (copied, remaining) = self.read_buf.pop_partial_slice(data);

                    self.playback_counter
                        .fetch_add(copied.len() as u64, Ordering::Relaxed);

                    for sample in remaining.iter_mut() {
                        *sample = f32::EQUILIBRIUM;
                    }
                },
                err_fn,
                None,
            ),
            // Handle other formats (I16, U16) as needed
            x => panic!("Unsupported sample format {:?}", x),
        }
        .unwrap();

        stream.play().unwrap();

        self.playback_done.wait();
    }
}

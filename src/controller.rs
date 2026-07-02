use std::sync::{
    Arc,
    atomic::AtomicU64,
    mpsc::{Receiver, Sender, channel},
};

use rubato::{Fft, FixedSync, Indexing, Resampler};

use audioadapter_buffers::direct::InterleavedSlice;

use anyhow::{self, Context, Error};
use rtrb::RingBuffer;
use symphonia::{
    core::{
        codecs::audio::{self, AudioDecoderOptions},
        formats::{FormatOptions, FormatReader, TrackType},
        io::{MediaSourceStream, ReadOnlySource},
        meta::MetadataOptions,
    },
    default::get_probe,
};

use crate::{
    play::{Playback, PlaybackConfig},
    processor::ProcessorWriter,
    signal::Signal,
};

enum AudioPlaybackMessage {
    Quit,
    SetVolume(u32),
    LoadStream(String),
    StopStream,
}

pub struct AudioController {
    playback_tx: Sender<AudioPlaybackMessage>,
}

impl AudioController {
    pub fn new(writer: ProcessorWriter, playback_counter: Arc<AtomicU64>) -> Self {
        let playback_done_signal = Signal::new();
        let (producer, consumer) = RingBuffer::new(10000);
        let playback = Playback::new(consumer, playback_done_signal.clone(), playback_counter);
        let playback_config = playback.get_config();
        std::thread::spawn(move || playback.run());

        let (tx, rx) = channel();
        let decoder =
            AudioDecoder::new(producer, playback_done_signal, writer, rx, playback_config);

        std::thread::spawn(|| decoder.run());
        Self { playback_tx: tx }
    }
    pub fn set_volume(&mut self, volume: u32) {
        let _ = self
            .playback_tx
            .send(AudioPlaybackMessage::SetVolume(volume));
    }
    pub fn load_stream(&mut self, stream_url: String) {
        let _ = self
            .playback_tx
            .send(AudioPlaybackMessage::LoadStream(stream_url));
    }
    pub fn stop_stream(&mut self) {
        let _ = self.playback_tx.send(AudioPlaybackMessage::StopStream);
    }
}

impl Drop for AudioController {
    fn drop(&mut self) {
        let _ = self.playback_tx.send(AudioPlaybackMessage::Quit);
    }
}

struct AudioDecoder {
    audio_buffer: rtrb::Producer<f32>,
    playback_done_signal: Signal,
    processor_writer: ProcessorWriter,
    volume_percent: u32,
    controller_rx: Receiver<AudioPlaybackMessage>,
    stream: Option<AudioStream>,
    playback_config: PlaybackConfig,
    input_buf: [f32; 15_000],
    input_buf_size: usize,
    output_buf: [f32; 20_000],
    output_buf_size: usize,
}

impl AudioDecoder {
    fn new(
        audio_buffer: rtrb::Producer<f32>,
        playback_done_signal: Signal,
        processor_writer: ProcessorWriter,
        controller_rx: Receiver<AudioPlaybackMessage>,
        playback_config: PlaybackConfig,
    ) -> Self {
        Self {
            audio_buffer,
            playback_done_signal,
            processor_writer,
            volume_percent: 100,
            controller_rx,
            stream: None,
            playback_config,
            input_buf: [0.0; 15_000],
            input_buf_size: 0,
            output_buf: [0.0; 20_000],
            output_buf_size: 0,
        }
    }
    fn run(mut self) {
        loop {
            if let Ok(msg) = self.controller_rx.try_recv() {
                match msg {
                    AudioPlaybackMessage::Quit => return,
                    AudioPlaybackMessage::SetVolume(volume_percent) => {
                        self.volume_percent = volume_percent
                    }
                    AudioPlaybackMessage::LoadStream(stream) => {
                        if let Ok(stream) = AudioStream::connect(stream, self.playback_config) {
                            self.stream = Some(stream);
                        }
                    }
                    AudioPlaybackMessage::StopStream => {
                        self.stream = None;
                    }
                }
                continue;
            }
            self.try_decode_samples()
        }
    }
    fn resample(&mut self) {
        let Some(stream) = self.stream.as_mut() else {
            return;
        };
        let mut frames_needed = stream.resampler.input_frames_next();
        let out_cap = self.output_buf.len() / self.playback_config.channels as usize;
        // wrap it with an InterleavedSlice Adapter
        let nbr_input_frames = self.input_buf_size / stream.channels;
        let input_adapter =
            InterleavedSlice::new(&self.input_buf, stream.channels, nbr_input_frames).unwrap();

        let outdata_capacity = self.output_buf.len() / self.playback_config.channels as usize;
        let mut output_adapter = InterleavedSlice::new_mut(
            &mut self.output_buf,
            self.playback_config.channels as usize,
            outdata_capacity,
        )
        .unwrap();

        let mut indexing = Indexing {
            input_offset: 0,
            output_offset: self.output_buf_size / self.playback_config.channels as usize,
            active_channels_mask: None,
            partial_len: None,
        };

        let mut input_frames_left = self.input_buf_size / stream.channels;
        let mut output_frames_needed = stream.resampler.output_frames_next();

        while input_frames_left >= frames_needed
            && (indexing.output_offset + output_frames_needed) < out_cap
        {
            let (frames_read, frames_written) = stream
                .resampler
                .process_into_buffer(&input_adapter, &mut output_adapter, Some(&indexing))
                .unwrap();

            indexing.input_offset += frames_read;
            indexing.output_offset += frames_written;
            input_frames_left -= frames_read;
            frames_needed = stream.resampler.input_frames_next();
            output_frames_needed = stream.resampler.output_frames_next();
        }
        self.output_buf_size = indexing.output_offset * self.playback_config.channels as usize;

        self.input_buf.copy_within(
            (indexing.input_offset * stream.channels)
                ..(indexing.input_offset + input_frames_left) * stream.channels,
            0,
        );

        self.input_buf_size = input_frames_left * stream.channels;
    }
    fn try_decode_samples(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            if self.input_buf_size < 10_000
                && let Ok(samples_written) =
                    stream.decode_samples(&mut self.input_buf[self.input_buf_size..])
            {
                for i in self.input_buf_size..self.input_buf_size + samples_written {
                    self.input_buf[i] *= self.volume_percent as f32 / 100.0;
                }
                self.input_buf_size += samples_written;
            }

            self.resample();

            let mut written = 0;
            while self.output_buf_size - written > 1024 {
                if self
                    .audio_buffer
                    .push_entire_slice(&self.output_buf[written..written + 1024])
                    .is_err()
                {
                    break;
                }

                written += 1024;
            }
            self.processor_writer
                .write_bytes(&self.output_buf[..written]);

            for i in 0..(self.output_buf_size - written) {
                self.output_buf[i] = self.output_buf[i + written];
            }
            self.output_buf_size -= written;
        }
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        self.playback_done_signal.clone().finish();
    }
}

struct AudioStream {
    decoder: Box<dyn audio::AudioDecoder>,
    format: Box<dyn FormatReader>,
    track_id: u32,
    channels: usize,
    resampler: Fft<f32>,
}

impl AudioStream {
    fn connect(url: String, playback_config: PlaybackConfig) -> Result<Self, Error> {
        let response = reqwest::blocking::get(url).expect("Failed to connect to stream");
        let reader = ReadOnlySource::new(response);

        let mss = MediaSourceStream::new(Box::new(reader), Default::default());

        let probe = get_probe();

        let fmt_opts: FormatOptions = Default::default();
        let meta_opts: MetadataOptions = Default::default();

        let format = probe
            .probe(&Default::default(), mss, fmt_opts, meta_opts)
            .expect("Failed to probe media format");

        let track = format
            .default_track(TrackType::Audio)
            .expect("no audio track");

        let dec_opts: AudioDecoderOptions = Default::default();

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(
                track
                    .codec_params
                    .as_ref()
                    .expect("codec parameters missing")
                    .audio()
                    .unwrap(),
                &dec_opts,
            )
            .expect("unsupported codec");

        let track_id = track.id;

        let audio_params = track.codec_params.as_ref().unwrap().audio().unwrap();

        let sample_rate = audio_params.sample_rate.unwrap_or(44100);

        let channels = audio_params
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap_or(2);

        let resampler = Fft::<f32>::new(
            sample_rate as usize,
            playback_config.sample_rate as usize,
            1024,
            2,
            channels,
            FixedSync::Both,
        )
        .unwrap();

        Ok(Self {
            format,
            decoder,
            track_id,
            channels,
            resampler,
        })
    }

    fn decode_samples(&mut self, sample_buf: &mut [f32]) -> anyhow::Result<usize> {
        let packet = match self.format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                // Reached the end of the stream.
                return Err(anyhow::anyhow!("End of stream"));
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !self.format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            self.format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id != self.track_id {
            return Err(anyhow::anyhow!("Packet doesn't belong to current track"));
        }

        // Decode the packet into audio samples.
        match self.decoder.decode(&packet) {
            Ok(audio_buf) => {
                let sample_count = audio_buf.samples_interleaved();

                // Copy the audio samples from the generic audio buffer to the vector in interleaved
                // order. The sample format to convert to is inferred from the type of the Vec.
                audio_buf.copy_to_slice_interleaved(&mut sample_buf[..sample_count]);
                Ok(sample_count)
            }
            Err(symphonia::core::errors::Error::IoError(e)) => {
                Err(e).with_context(|| "failed to decode packet due to IO error")
                // The packet failed to decode due to an IO error, skip the packet.
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                Err(anyhow::anyhow!(
                    "failed to decode packet due to invalid data"
                ))
                // The packet failed to decode due to invalid data, skip the packet.
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        }
    }
}

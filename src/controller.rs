use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64},
    mpsc::{Receiver, Sender, channel},
};

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
    sample_buf: Vec<f32>,
    playback_config: PlaybackConfig,
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
            sample_buf: vec![],
            playback_config,
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
                        if let Ok(stream) = AudioStream::connect(stream) {
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
    fn try_decode_samples(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            if stream.decode_samples(&mut self.sample_buf).is_ok() {
                for frame in &mut self.sample_buf {
                    *frame *= (self.volume_percent as f32 / 100.0);
                }
                while !self
                    .audio_buffer
                    .push_entire_slice(&self.sample_buf)
                    .is_ok()
                {}
                self.processor_writer.write_bytes(&self.sample_buf);
            }
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
}

impl AudioStream {
    fn connect(url: String) -> Result<Self, Error> {
        let response = reqwest::blocking::get(url).expect("Failed to connect to stream");
        let reader = ReadOnlySource::new(response);

        let mss = MediaSourceStream::new(Box::new(reader), Default::default());

        let probe = get_probe();

        let fmt_opts: FormatOptions = Default::default();
        let meta_opts: MetadataOptions = Default::default();

        let mut format = probe
            .probe(&Default::default(), mss, fmt_opts, meta_opts)
            .expect("Failed to probe media format");

        let track = format
            .default_track(TrackType::Audio)
            .expect("no audio track");

        let dec_opts: AudioDecoderOptions = Default::default();

        let mut decoder = symphonia::default::get_codecs()
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
        Ok(Self {
            format,
            decoder,
            track_id,
        })
    }

    fn decode_samples(&mut self, sample_buf: &mut Vec<f32>) -> anyhow::Result<()> {
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
                sample_buf.resize(audio_buf.samples_interleaved(), 0.0);

                // Copy the audio samples from the generic audio buffer to the vector in interleaved
                // order. The sample format to convert to is inferred from the type of the Vec.
                audio_buf.copy_to_slice_interleaved(sample_buf);
                Ok(())
            }
            Err(symphonia::core::errors::Error::IoError(e)) => {
                return Err(e).with_context(|| "failed to decode packet due to IO error");
                // The packet failed to decode due to an IO error, skip the packet.
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                return Err(anyhow::anyhow!(
                    "failed to decode packet due to invalid data"
                ));
                // The packet failed to decode due to invalid data, skip the packet.
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        }
    }
}

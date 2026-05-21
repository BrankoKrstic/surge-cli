use symphonia::{
    core::{
        codecs::audio::AudioDecoderOptions,
        formats::{FormatOptions, TrackType},
        io::{MediaSourceStream, ReadOnlySource},
        meta::MetadataOptions,
    },
    default::get_probe,
};

use crate::signal::Signal;

pub struct AudioController {
    audio_buffer: rtrb::Producer<f32>,
    playback_done_signal: Signal,
}

impl AudioController {
    pub fn new(audio_buffer: rtrb::Producer<f32>, playback_done_signal: Signal) -> Self {
        Self {
            audio_buffer,
            playback_done_signal,
        }
    }
    pub fn start_stream(&mut self, url: &str) {
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
        let mut samples: Vec<f32> = Default::default();
        let mut total_sample_count = 0;

        loop {
            // Get the next packet from the media format.
            let packet = match format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    // Reached the end of the stream.
                    break;
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
            while !format.metadata().is_latest() {
                // Pop the old head of the metadata queue.
                format.metadata().pop();

                // Consume the new metadata at the head of the metadata queue.
            }

            // If the packet does not belong to the selected track, skip over it.
            if packet.track_id != track_id {
                continue;
            }

            // Decode the packet into audio samples.
            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    samples.resize(audio_buf.samples_interleaved(), 0.0);

                    // Copy the audio samples from the generic audio buffer to the vector in interleaved
                    // order. The sample format to convert to is inferred from the type of the Vec.
                    audio_buf.copy_to_slice_interleaved(&mut samples);
                    while !self.audio_buffer.push_entire_slice(&samples).is_ok() {}
                    // Sum up the total number of samples.
                    total_sample_count += samples.len();

                    // Consume the decoded audio samples (see below).
                }
                Err(symphonia::core::errors::Error::IoError(_)) => {
                    // The packet failed to decode due to an IO error, skip the packet.
                    continue;
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    // The packet failed to decode due to invalid data, skip the packet.
                    continue;
                }
                Err(err) => {
                    // An unrecoverable error occurred, halt decoding.
                    panic!("{}", err);
                }
            }
        }
    }
}

impl Drop for AudioController {
    fn drop(&mut self) {
        self.playback_done_signal.clone().finish();
    }
}

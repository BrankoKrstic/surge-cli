use std::{error::Error, f64::consts::PI};

use cpal::{
    Sample, SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use num::{Complex, pow::Pow};
use radiobrowser::{RadioBrowserAPI, StationOrder};
use reqwest::Client;
use symphonia::{
    core::{
        codecs::audio::AudioDecoderOptions,
        formats::{FormatOptions, TrackType},
        io::{MediaSourceStream, ReadOnlySource},
        meta::MetadataOptions,
    },
    default::get_probe,
};
use tokio::join;

fn fft(input: &[f64]) -> Vec<Complex<f64>> {
    let n = input.len();

    if n <= 1 {
        return input.iter().map(Complex::from).collect();
    }

    let evens: Vec<f64> = input.iter().step_by(2).map(|x| *x).collect();
    let odds: Vec<f64> = input.iter().skip(1).step_by(2).map(|x| *x).collect();

    let even = fft(&evens[..]);
    let odd = fft(&odds[..]);

    let w = (-2.0 * PI / n as f64 * Complex::i()).exp();
    let mut out = vec![Complex::new(0.0, 0.0); input.len()];
    for k in 0..n / 2 {
        let t = w.pow(k as f64) * odd[k];
        out[k] = even[k] + t;
        out[k + n / 2] = even[k] - t;
    }

    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut api = RadioBrowserAPI::new().await?;
    let countries = api.get_countries().send();
    let languages = api.get_languages().send();
    let stations = api
        .get_stations()
        .country("Serbia")
        .reverse(true)
        .order(StationOrder::Clickcount)
        .send();
    let config = api.get_server_config();
    let (stations, config, countries, languages) = join!(stations, config, countries, languages);

    println!("Config: {:#?}", config?);
    println!("Countries found: {}", countries?.len());
    println!("Languages found: {}", languages?.len());

    println!("Stations found: {:?}", stations?);

    // 1. Create a blocking request to the web stream
    //
    //
    tokio::task::spawn_blocking(|| {
        let response = reqwest::blocking::get("https://media.radioexs.com/stream/jackradio")
            .expect("Failed to connect to stream");
        // 2. Wrap the response body in a ReadOnlySource
        let reader = ReadOnlySource::new(response);

        // 3. Create a MediaSourceStream (disabling seekability)
        let mss = MediaSourceStream::new(Box::new(reader), Default::default());

        // 4. Probe and decode the stream
        let mut probe = get_probe();

        let fmt_opts: FormatOptions = Default::default();
        let meta_opts: MetadataOptions = Default::default();

        let mut format = probe
            .probe(&Default::default(), mss, fmt_opts, meta_opts)
            .expect("Failed to probe media format");

        // Find the first audio track with a known (decodeable) codec.
        let track = format
            .default_track(TrackType::Audio)
            .expect("no audio track");

        // Use the default options for the decoder.
        let dec_opts: AudioDecoderOptions = Default::default();

        // Create a decoder for the track.
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

        // Store the track identifier, it will be used to filter packets.
        let track_id = track.id;
        let mut samples: Vec<f32> = Default::default();
        let mut total_sample_count = 0;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");
        let mut supported_configs_range = device
            .supported_output_configs()
            .expect("error while querying configs");

        let supported_config = supported_configs_range
            .next()
            .expect("no supported config?!")
            .with_max_sample_rate();

        let mut sample_clock = 0f32;
        let sample_rate = supported_config.sample_rate() as f32;
        let mut next_value = move || {
            sample_clock = (sample_clock + 1.0) % sample_rate;
            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
        };

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        // Choose format and build stream
        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                &supported_config.config(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for sample in data.iter_mut() {
                        *sample = next_value();
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
        // The decode loop.
        //
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

                    // Sum up the total number of samples.
                    total_sample_count += samples.len();
                    print!("\rDecoded {total_sample_count} samples {}", samples.len());

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
    })
    .await;

    let mut input = [0f64; 8];
    let mut out = [Complex::new(0.0f64, 0.0); 8];
    for i in 0..input.len() {
        let t = i as f64 / 8.0;
        input[i] = (2.0 * PI * t * 1.0).cos() + (2.0 * PI * t * 2.0).sin();
    }

    for f in 0..input.len() {
        print!("{}: ", f);
        for j in 0..input.len() {
            let t = j as f64 / 8.0;
            print!("{:.2} ", (2.0 * PI * f as f64 * t).cos());
            out[f] += input[j] * (Complex::i() * 2.0 * PI * f as f64 * t).exp();
        }
        println!("");
    }
    println!("{:?}", input);

    let f = fft(&input[..]);
    println!("{:?}", f);

    Ok(())
}

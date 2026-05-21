use std::error::Error;

use radiobrowser::{RadioBrowserAPI, StationOrder};
use rtrb::RingBuffer;
use surge::{controller::AudioController, play::Playback, signal::Signal};
use tokio::join;

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
        // The decode loop.
        //
        //
        let playback_done_signal = Signal::new();
        let (producer, consumer) = RingBuffer::new(10000);
        let playback = Playback::new(consumer, playback_done_signal.clone());

        std::thread::spawn(move || playback.run());

        let mut audio = AudioController::new(producer, playback_done_signal);

        audio.start_stream("https://media.radioexs.com/stream/jackradio");
    })
    .await;

    Ok(())
}

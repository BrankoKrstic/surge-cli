use std::error::Error;

use radiobrowser::{RadioBrowserAPI, StationOrder};
use ratatui::{DefaultTerminal, Frame, Terminal, prelude::CrosstermBackend};
use rtrb::RingBuffer;
use surge::{
    cli::{
        app::App,
        event::{Event, EventHandler},
        tui::Tui,
        update::update,
    },
    controller::AudioController,
    play::Playback,
    signal::Signal,
};
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

    tokio::task::spawn_blocking(|| {
        let playback_done_signal = Signal::new();
        let (producer, consumer) = RingBuffer::new(10000);
        let playback = Playback::new(consumer, playback_done_signal.clone());

        std::thread::spawn(move || playback.run());

        let mut audio = AudioController::new(producer, playback_done_signal);

        audio.start_stream("https://media.radioexs.com/stream/jackradio");
    });

    let mut app = App::new();

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    // Start the main loop.
    while !app.should_quit() {
        // Render the user interface.
        tui.draw(&mut app)?;
        // Handle events.
        match tui.events.next()? {
            Event::Tick => {}
            Event::Key(key_event) => update(&mut app, key_event),
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        };
    }

    // Exit the user interface.
    tui.exit()?;

    Ok(())
}

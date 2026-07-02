use std::{
    error::Error,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use crossterm::event;
use ratatui::{Terminal, prelude::CrosstermBackend};
use surge::{
    cli::{
        app::App,
        event::{Event, EventHandler},
        tui::Tui,
        update::update,
    },
    controller::AudioController,
    processor::Processor,
};

const INITIAL_STREAM_NAME: &str = "Jack Radio";
const INITIAL_STREAM_URL: &str = "https://media.radioexs.com/stream/jackradio";

fn main() -> Result<(), Box<dyn Error>> {
    let playback_counter = Arc::new(AtomicU64::new(0));
    let (reader, writer) = Processor::new(playback_counter.clone()).split();
    let mut audio = AudioController::new(writer, playback_counter);
    let sample_rate = audio.playback_config().sample_rate;
    audio.load_stream(INITIAL_STREAM_URL.to_string());

    let mut app = App::new(reader, audio, INITIAL_STREAM_NAME.to_string(), sample_rate);

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new();
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    let frame_timeout = Duration::from_secs_f64(1.0 / 60.0);
    // Start the main loop.
    while !app.should_quit() {
        // Render the user interface.
        app.tick();
        tui.draw(&mut app)?;

        event::poll(frame_timeout).expect("Unable to poll event");
        // Handle events.
        //
        while let Some(event) = tui.events.try_next()? {
            match event {
                Event::Key(key_event) => update(&mut app, key_event),
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
            }
        }
    }

    // Exit the user interface.
    tui.exit()?;
    Ok(())
}

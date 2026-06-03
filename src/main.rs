use std::{
    error::Error,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use crossterm::event;
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
    processor::Processor,
    radio::RadioApiFetcher,
    signal::Signal,
};

fn main() -> Result<(), Box<dyn Error>> {
    let playback_counter = Arc::new(AtomicU64::new(0));
    let (reader, writer) = Processor::new(playback_counter.clone()).split();

    std::thread::spawn(|| {
        let playback_done_signal = Signal::new();
        let (producer, consumer) = RingBuffer::new(10000);
        let playback = Playback::new(consumer, playback_done_signal.clone(), playback_counter);

        std::thread::spawn(move || playback.run());

        let mut audio = AudioController::new(producer, playback_done_signal, writer);

        audio.start_stream("https://media.radioexs.com/stream/jackradio");
    });

    let mut app = App::new(reader);

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
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

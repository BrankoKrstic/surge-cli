use crossterm::event;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use color_eyre::Result;

/// Terminal events.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// Key press.
    Key(KeyEvent),
    /// Mouse click/scroll.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event receiver channel.
    receiver: mpsc::Receiver<Event>,
    /// Event handler thread.
    handler: thread::JoinHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        let handler = {
            let sender = sender.clone();
            thread::spawn(move || {
                loop {
                    let send_result = match event::read().expect("unable to read event") {
                        crossterm::event::Event::Key(e) if e.kind == event::KeyEventKind::Press => {
                            sender.send(Event::Key(e))
                        }
                        crossterm::event::Event::Key(e) => Ok(()),
                        crossterm::event::Event::Mouse(e) => sender.send(Event::Mouse(e)),
                        crossterm::event::Event::Resize(w, h) => sender.send(Event::Resize(w, h)),
                        _ => unimplemented!(),
                    };
                    if send_result.is_err() {
                        break;
                    }
                }
            })
        };
        Self { receiver, handler }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub fn next(&self) -> Result<Event> {
        Ok(self.receiver.recv()?)
    }
    pub fn try_next(&self) -> Result<Option<Event>> {
        match self.receiver.try_recv() {
            Ok(e) => Ok(Some(e)),
            Err(err) => match err {
                mpsc::TryRecvError::Empty => Ok(None),
                mpsc::TryRecvError::Disconnected => Err(err)?,
            },
        }
    }
}

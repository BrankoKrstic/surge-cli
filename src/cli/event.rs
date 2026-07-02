use crossterm::event;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use std::{sync::mpsc, thread};

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
    receiver: mpsc::Receiver<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let _ = {
            let sender = sender.clone();
            thread::spawn(move || {
                loop {
                    let send_result = match event::read().expect("unable to read event") {
                        crossterm::event::Event::Key(e) if e.kind == event::KeyEventKind::Press => {
                            sender.send(Event::Key(e))
                        }
                        crossterm::event::Event::Key(_) => Ok(()),
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
        Self { receiver }
    }

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

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

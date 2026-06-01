use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::processor::ProcessorReader;

pub struct App {
    processor_reader: ProcessorReader,
    frequencies: [f64; 4096],
    exit: bool,
}

impl App {
    pub fn new(reader: ProcessorReader) -> Self {
        Self {
            processor_reader: reader,
            frequencies: [0.0; 4096],
            exit: false,
        }
    }
    pub fn freqs(&self) -> &[f64] {
        &self.frequencies[..]
    }
    pub fn tick(&mut self) {
        self.frequencies = self.processor_reader.query_frequencies();
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event);
            }
            _ => {}
        }
        Ok(())
    }
    pub fn should_quit(&self) -> bool {
        self.exit
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.quit(),
            _ => {}
        }
    }
    pub fn quit(&mut self) {
        self.exit = true;
    }
}

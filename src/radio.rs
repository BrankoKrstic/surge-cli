use std::error::Error;

use oneshot::{Receiver, channel};
use radiobrowser::{ApiStation, StationOrder, blocking::RadioBrowserAPI};

pub struct RadioApiFetcher {
    api: RadioBrowserAPI,
    state: RadioState,
    pending_channel: Option<Receiver<Result<Vec<ApiStation>, String>>>,
}

#[derive(Clone)]
pub enum RadioState {
    Pending,
    Error(String),
    Complete(Vec<ApiStation>),
}

impl RadioApiFetcher {
    pub fn new() -> Self {
        let api = RadioBrowserAPI::new().unwrap();
        let mut out = Self {
            api,
            state: RadioState::Complete(vec![]),
            pending_channel: None,
        };
        out.query("");
        out
    }

    pub fn query(&mut self, search: &str) {
        let stations = self
            .api
            .get_stations()
            .name(search)
            .limit("20")
            .reverse(true)
            .order(StationOrder::Clickcount);
        let (tx, rx) = oneshot::channel();
        self.state = RadioState::Pending;
        self.pending_channel = Some(rx);
        std::thread::spawn(|| {
            let response = stations.send();
            let _ = tx.send(response.map_err(|e| e.to_string()));
        });
    }

    fn poll_channel(&mut self) {
        if let Some(channel) = self.pending_channel.take() {
            if let Ok(response) = channel.try_recv() {
                match response {
                    Err(err) => self.state = RadioState::Error(err),
                    Ok(stations) => self.state = RadioState::Complete(stations),
                }
            } else {
                self.pending_channel = Some(channel);
            }
        }
    }
    pub fn poll_state(&mut self) -> RadioState {
        self.poll_channel();
        return self.state.clone();
    }
}

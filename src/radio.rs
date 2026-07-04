use oneshot::Receiver;
use radiobrowser::{ApiStation, StationOrder, blocking::RadioBrowserAPI};

use crate::loader::{Loader, LoaderState};

pub struct RadioApiFetcher {
    api: RadioBrowserAPI,
    state: Loader<Result<Vec<ApiStation>, String>>,
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
            state: Loader::new(|| Ok(vec![])),
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
        self.state = Loader::new(move || stations.clone().send().map_err(|e| e.to_string()));
    }
    pub fn poll_state(&mut self) -> RadioState {
        match self.state.get_state() {
            LoaderState::Pending => RadioState::Pending,
            LoaderState::Done(res) => match res {
                Ok(res) => RadioState::Complete(res.clone()),
                Err(err) => RadioState::Error(err.clone()),
            },
        }
    }
}

impl Default for RadioApiFetcher {
    fn default() -> Self {
        Self::new()
    }
}

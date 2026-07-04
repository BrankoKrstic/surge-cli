use std::thread;

pub enum LoaderState<T> {
    Pending,
    Done(T),
}

pub struct Loader<T> {
    state: Option<T>,
    rx: oneshot::Receiver<T>,
}

impl<T: Send + 'static> Loader<T> {
    pub fn new(load: impl Fn() -> T + Send + 'static) -> Self {
        let (tx, rx) = oneshot::channel();
        thread::spawn(move || {
            let result = load();

            let _ = tx.send(result);
        });

        Self { rx, state: None }
    }
    pub fn get_state(&mut self) -> LoaderState<&T> {
        self.poll();
        match self.state.as_ref() {
            Some(res) => LoaderState::Done(res),
            None => LoaderState::Pending,
        }
    }
    pub fn take_state(mut self) -> Result<T, Self> {
        self.poll();
        if let Some(state) = self.state.take() {
            Ok(state)
        } else {
            Err(self)
        }
    }
    fn poll(&mut self) {
        if let Ok(res) = self.rx.try_recv() {
            self.state = Some(res);
        }
    }
}

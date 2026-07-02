use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone)]
pub struct Signal {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn wait(&self) {
        let mut done = self.inner.0.lock().unwrap();

        while !*done {
            done = self.inner.1.wait(done).unwrap();
        }
    }

    pub fn finish(self) {
        let mut done = self.inner.0.lock().unwrap();
        *done = true;
        self.inner.1.notify_all();
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

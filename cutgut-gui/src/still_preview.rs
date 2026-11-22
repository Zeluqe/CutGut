use std::sync::{Arc, Condvar, Mutex};

use eframe::egui::{ColorImage, Context};

use crate::player::{self, PreviewSize};
use crate::process::Process;

struct Request {
    path: String,
    time: f64,
    size: PreviewSize,
}

#[derive(Default)]
struct State {
    next: Option<(u64, Request)>,
    active: Option<Process>,
    completed: Option<(u64, Option<ColorImage>)>,
    generation: u64,
    first_valid: u64,
    closed: bool,
}

/// One decoder, one replaceable request and one replaceable result.
pub struct StillPreview {
    queue: Arc<(Mutex<State>, Condvar)>,
    pub pending: bool,
}

impl StillPreview {
    pub fn new(ctx: Context) -> Self {
        let queue: Arc<(Mutex<State>, Condvar)> = Arc::default();
        let worker = queue.clone();
        std::thread::spawn(move || {
            let (lock, wake) = &*worker;
            loop {
                let mut state = lock.lock().unwrap();
                state = wake
                    .wait_while(state, |state| state.next.is_none() && !state.closed)
                    .unwrap();
                if state.closed {
                    break;
                }
                let (generation, request) = state.next.take().unwrap();
                let process = Process::default();
                state.active = Some(process.clone());
                drop(state);

                let image =
                    player::decode_still(&request.path, request.time, request.size, &process);
                let mut state = lock.lock().unwrap();
                state.active = None;
                if generation >= state.first_valid {
                    state.completed = Some((generation, image));
                    ctx.request_repaint();
                }
            }
        });
        Self {
            queue,
            pending: false,
        }
    }

    pub fn request(&mut self, path: &str, time: f64, size: PreviewSize) {
        let (lock, wake) = &*self.queue;
        let mut state = lock.lock().unwrap();
        state.generation += 1;
        state.next = Some((
            state.generation,
            Request {
                path: path.to_owned(),
                time,
                size,
            },
        ));
        self.pending = true;
        wake.notify_one();
    }

    pub fn clear(&mut self) {
        let mut state = self.queue.0.lock().unwrap();
        state.first_valid = state.generation + 1;
        state.next = None;
        state.completed = None;
        if let Some(process) = state.active.take() {
            process.cancel();
        }
        self.pending = false;
    }

    pub fn poll(&mut self) -> Option<ColorImage> {
        let mut state = self.queue.0.lock().unwrap();
        let (generation, image) = state.completed.take()?;
        self.pending = generation != state.generation;
        image
    }
}

impl Drop for StillPreview {
    fn drop(&mut self) {
        self.clear();
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap().closed = true;
        wake.notify_one();
    }
}

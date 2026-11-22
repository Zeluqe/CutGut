use std::io;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

/// Gives the UI a way to interrupt a decoder blocked on pipe I/O.
#[derive(Clone, Default)]
pub struct Process(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    cancelled: bool,
    child: Option<Child>,
}

impl Process {
    pub fn spawn(&self, command: &mut Command) -> io::Result<()> {
        let child = command.spawn()?;
        let mut state = self.0.lock().unwrap();
        if state.cancelled {
            drop(state);
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::ErrorKind::Interrupted.into());
        }
        state.child = Some(child);
        Ok(())
    }

    pub fn stdout(&self) -> Option<std::process::ChildStdout> {
        self.0.lock().unwrap().child.as_mut()?.stdout.take()
    }

    pub fn cancel(&self) {
        let mut state = self.0.lock().unwrap();
        state.cancelled = true;
        if let Some(child) = state.child.as_mut() {
            let _ = child.kill();
        }
    }

    pub fn finish(&self) {
        // Both terminating and reaping belong to the worker on normal EOF.
        let child = self.0.lock().unwrap().child.take();
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    #[cfg(all(test, windows))]
    pub fn id(&self) -> Option<u32> {
        self.0.lock().unwrap().child.as_ref().map(Child::id)
    }
}

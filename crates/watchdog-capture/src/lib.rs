use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use watchdog_core::{CapturedFrame, WatchGate};
use xcap::Monitor;

pub type SharedFrame = Arc<Mutex<Option<CapturedFrame>>>;
pub type SharedCaptureError = Arc<Mutex<Option<String>>>;

/// Polls the watch gate. Monitor capture is never called while the gate is OFF.
pub struct CaptureWorker {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CaptureWorker {
    pub fn start(gate: WatchGate, latest: SharedFrame, last_error: SharedCaptureError) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let handle = thread::Builder::new()
            .name("watchdog-capture".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    if !gate.is_enabled() {
                        thread::sleep(Duration::from_millis(180));
                        continue;
                    }

                    let result = capture_primary_monitor();
                    match result {
                        Ok(frame) => {
                            if let Ok(mut slot) = latest.lock() {
                                // Re-check the gate while holding the frame lock so STOP
                                // cannot leave a newly captured frame behind.
                                if gate.is_enabled() {
                                    *slot = Some(frame);
                                }
                            }
                            if let Ok(mut error) = last_error.lock() {
                                *error = None;
                            }
                        }
                        Err(message) => {
                            if let Ok(mut error) = last_error.lock() {
                                *error = Some(message);
                            }
                        }
                    }

                    thread::sleep(Duration::from_millis(850));
                }
            })
            .expect("failed to create WATCHDOG capture worker");

        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn capture_primary_monitor() -> Result<CapturedFrame, String> {
    let mut monitors = Monitor::all().map_err(|error| error.to_string())?;
    let monitor = monitors
        .drain(..)
        .next()
        .ok_or_else(|| "No display was detected.".to_owned())?;
    let image = monitor.capture_image().map_err(|error| error.to_string())?;
    let (width, height) = image.dimensions();
    let frame = CapturedFrame {
        width,
        height,
        rgba: image.into_raw(),
    };

    if frame.is_valid() {
        Ok(frame)
    } else {
        Err("The capture API returned an invalid frame.".into())
    }
}

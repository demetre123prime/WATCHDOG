use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// A clonable, OFF-by-default capture gate shared by the UI and capture worker.
#[derive(Clone, Debug)]
pub struct WatchGate {
    enabled: Arc<AtomicBool>,
}

impl Default for WatchGate {
    fn default() -> Self {
        Self::new_off()
    }
}

impl WatchGate {
    pub fn new_off() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
}

/// A temporary frame stored in process memory, never persisted by this crate.
#[derive(Clone, Debug)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl CapturedFrame {
    pub fn is_valid(&self) -> bool {
        let expected = self.width as usize * self.height as usize * 4;
        self.width > 0 && self.height > 0 && self.rgba.len() == expected
    }
}

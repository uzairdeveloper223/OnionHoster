use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct ServiceStats {
    pub requests: AtomicU64,
    pub bytes_in: AtomicU64,
    pub bytes_out: AtomicU64,
    pub active: AtomicU64,
    pub last_request_ms: AtomicU64,
}

impl ServiceStats {
    pub fn request(&self, bytes_in: u64) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.bytes_in.fetch_add(bytes_in, Ordering::Relaxed);
        self.last_request_ms
            .store(crate::services::now_ms(), Ordering::Relaxed);
    }

    pub fn add_bytes_out(&self, bytes: u64) {
        self.bytes_out.fetch_add(bytes, Ordering::Relaxed);
    }
}

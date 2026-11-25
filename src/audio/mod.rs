//! Audio capture: WAV writing, sample clock, session worker and the WASAPI backend.
//! See docs/superpowers/specs/2026-11-24-audio-capture-design.md.

pub mod backend;
pub mod clock;
pub mod naming;
pub mod session;
#[cfg(windows)]
pub mod wasapi_backend;
pub mod wav;
pub mod worker;

/// Unique, freshly created directory under the system temp dir, for tests.
#[cfg(test)]
pub(crate) fn temp_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "meeting-recorder-test-{}-{}-{}",
        std::process::id(),
        tag,
        N.fetch_add(1, Ordering::Relaxed)
    ));
    // A process id can come back: start from nothing, not from what an old run left.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

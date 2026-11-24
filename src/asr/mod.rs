//! Thai transcription after recording: text clean-up, transcript files, WAV reading,
//! features, segmentation, the model and the worker.
//! See docs/superpowers/specs/2026-11-24-asr-transcription-design.md.

#[cfg(test)]
mod accuracy;
pub mod download;
pub mod engine;
pub mod features;
#[cfg(test)]
mod features_reference;
pub mod model;
pub mod resample;
pub mod segment;
pub mod thai;
pub mod transcript;
pub mod wavread;
pub mod worker;

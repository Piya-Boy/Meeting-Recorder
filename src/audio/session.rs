//! Session types shared by the UI and the capture worker: states, commands, events.

use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Starting,
    Recording,
    Paused,
    Stopping,
    Finalizing,
    Saved,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Begin,
    Started,
    Pause,
    Resume,
    Stop,
    Finalize,
    Finalized,
    Fail,
}

impl SessionState {
    /// The state after `t`, or `None` when `t` is not legal here.
    pub fn after(self, t: Transition) -> Option<SessionState> {
        use SessionState::*;
        use Transition::*;
        match (self, t) {
            (Idle | Saved | Error, Begin) => Some(Starting),
            (Starting, Started) => Some(Recording),
            (Recording, Pause) => Some(Paused),
            (Paused, Resume) => Some(Recording),
            (Recording | Paused, Stop) => Some(Stopping),
            (Stopping, Finalize) => Some(Finalizing),
            (Finalizing, Finalized) => Some(Saved),
            (Starting | Recording | Paused | Stopping | Finalizing, Fail) => Some(Error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    System,
    Mic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceStatus {
    Active,
    AllSystemFallback,
    WaitingForApp,
    DeviceLost,
    Disabled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// A device of `None` means the Windows default.
#[derive(Clone, Debug)]
pub struct StartRequest {
    pub system: bool,
    pub mic: bool,
    pub sample_rate: u32,
    pub base_dir: PathBuf,
    pub name: String,
    pub system_device: Option<String>,
    pub mic_device: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Command {
    Start(StartRequest),
    Pause,
    Resume,
    Stop,
    ListDevices,
}

#[derive(Clone, Debug)]
pub enum Event {
    State(SessionState),
    Levels {
        system_db: f32,
        mic_db: f32,
    },
    SourceStatus {
        source: Source,
        status: SourceStatus,
    },
    Elapsed(Duration),
    /// A session began: its folder exists and both track files (the sources that are on) are
    /// created, so something can follow them while they are written.
    SessionStarted {
        dir: PathBuf,
    },
    Saved {
        dir: PathBuf,
        duration: Duration,
    },
    Error(String),
    /// System render devices (for loopback) and microphones.
    Devices {
        system: Vec<DeviceInfo>,
        mic: Vec<DeviceInfo>,
    },
}

/// Level of `samples` in dB relative to full scale (RMS); -90 for silence or no samples.
pub fn dbfs(samples: &[i16]) -> f32 {
    const FLOOR: f32 = -90.0;
    if samples.is_empty() {
        return FLOOR;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    let rms = (sum / samples.len() as f64).sqrt() / 32_768.0;
    if rms <= 0.0 {
        return FLOOR;
    }
    ((20.0 * rms.log10()) as f32).max(FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use SessionState::*;
    use Transition::*;

    fn walk(start: SessionState, ts: &[Transition]) -> Option<SessionState> {
        ts.iter().try_fold(start, |s, t| s.after(*t))
    }

    #[test]
    fn legal_path_to_saved() {
        let path = [Begin, Started, Pause, Resume, Stop, Finalize, Finalized];
        assert_eq!(walk(Idle, &path), Some(Saved));
    }

    #[test]
    fn illegal_moves_are_none() {
        assert_eq!(Idle.after(Pause), None);
        assert_eq!(Starting.after(Pause), None);
        assert_eq!(Paused.after(Pause), None);
        assert_eq!(Recording.after(Resume), None);
        assert_eq!(Saved.after(Stop), None);
        assert_eq!(Idle.after(Stop), None);
    }

    #[test]
    fn stop_allowed_from_recording_and_paused() {
        assert_eq!(Recording.after(Stop), Some(Stopping));
        assert_eq!(Paused.after(Stop), Some(Stopping));
    }

    #[test]
    fn fail_reaches_error_from_every_active_state() {
        for s in [Starting, Recording, Paused, Stopping, Finalizing] {
            assert_eq!(s.after(Fail), Some(Error), "from {s:?}");
        }
        assert_eq!(Idle.after(Fail), None);
        assert_eq!(Saved.after(Fail), None);
    }

    #[test]
    fn restart_allowed_after_saved_and_error() {
        assert_eq!(Saved.after(Begin), Some(Starting));
        assert_eq!(Error.after(Begin), Some(Starting));
        assert_eq!(Recording.after(Begin), None);
    }

    #[test]
    fn dbfs_silence_is_floor() {
        assert_eq!(dbfs(&[0i16; 480]), -90.0);
        assert_eq!(dbfs(&[]), -90.0);
    }

    #[test]
    fn dbfs_full_scale_square_is_about_zero() {
        let s: Vec<i16> = (0..480)
            .map(|i| if i % 2 == 0 { i16::MAX } else { -i16::MAX })
            .collect();
        assert!(dbfs(&s).abs() < 0.1, "{}", dbfs(&s));
    }

    #[test]
    fn dbfs_half_amplitude_is_about_minus_six() {
        let d = dbfs(&[16_384i16; 480]);
        assert!((d + 6.0).abs() < 0.2, "{d}");
    }
}

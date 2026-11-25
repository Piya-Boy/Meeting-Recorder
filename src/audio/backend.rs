//! What the session worker needs from the audio hardware, behind a trait so the worker can be
//! tested with a fake and the real WASAPI backend (Windows) plugs in last.

use std::sync::mpsc::Sender;
use std::time::Instant;

use super::session::{DeviceInfo, Source};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Mic { device: Option<String> },
    WholeSystem { device: Option<String> },
    Process { pid: u32 },
}

impl SourceKind {
    pub fn source(&self) -> Source {
        match self {
            SourceKind::Mic { .. } => Source::Mic,
            SourceKind::WholeSystem { .. } | SourceKind::Process { .. } => Source::System,
        }
    }
}

/// Mono i16 samples at the session rate. `start_frame` counts from the shared session origin.
pub struct Chunk {
    pub source: Source,
    pub start_frame: u64,
    pub samples: Vec<i16>,
}

pub enum Msg {
    Chunk(Chunk),
    /// The stream or device died; the backend sends this once per stream.
    Lost(Source),
}

pub trait Stream: Send {
    fn stop(self: Box<Self>);
}

pub trait Backend: Send + Sync + 'static {
    /// Starts capturing `kind` on its own thread, sending `Msg`s to `tx`.
    fn open(
        &self,
        kind: SourceKind,
        origin: Instant,
        sample_rate: u32,
        tx: Sender<Msg>,
    ) -> Result<Box<dyn Stream>, String>;
    /// The meeting app process to capture, if one is playing sound.
    fn meeting_target(&self) -> Option<u32>;
    /// System render devices (for loopback), then microphones.
    fn list_devices(&self) -> (Vec<DeviceInfo>, Vec<DeviceInfo>);
}

#[cfg(test)]
pub(crate) use fake::{FailKind, FakeBackend};

#[cfg(test)]
mod fake {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum FailKind {
        Mic,
        WholeSystem,
        Process,
    }

    #[derive(Default)]
    struct State {
        opened: Vec<SourceKind>,
        fail: Vec<FailKind>,
        target: Option<u32>,
        txs: HashMap<usize, (u64, Sender<Msg>)>,
        next_gen: u64,
        devices: (Vec<DeviceInfo>, Vec<DeviceInfo>),
    }

    fn slot(s: Source) -> usize {
        match s {
            Source::System => 0,
            Source::Mic => 1,
        }
    }

    /// Lets a test push chunks per source, set the meeting target, make opens fail and see
    /// what was opened.
    #[derive(Clone, Default)]
    pub(crate) struct FakeBackend {
        state: Arc<Mutex<State>>,
    }

    impl FakeBackend {
        pub(crate) fn new() -> Self {
            Self::default()
        }
        pub(crate) fn fail_open(&self, kind: FailKind) {
            self.state.lock().unwrap().fail.push(kind);
        }
        pub(crate) fn set_target(&self, pid: Option<u32>) {
            self.state.lock().unwrap().target = pid;
        }
        pub(crate) fn set_devices(&self, system: Vec<DeviceInfo>, mic: Vec<DeviceInfo>) {
            self.state.lock().unwrap().devices = (system, mic);
        }
        pub(crate) fn opened(&self) -> Vec<SourceKind> {
            self.state.lock().unwrap().opened.clone()
        }
        /// Sends a chunk as the open stream of `source`; false if none is open.
        pub(crate) fn push(&self, source: Source, start_frame: u64, samples: Vec<i16>) -> bool {
            let st = self.state.lock().unwrap();
            match st.txs.get(&slot(source)) {
                Some((_, tx)) => tx
                    .send(Msg::Chunk(Chunk {
                        source,
                        start_frame,
                        samples,
                    }))
                    .is_ok(),
                None => false,
            }
        }
        /// Reports the stream of `source` as lost and closes it.
        pub(crate) fn lose(&self, source: Source) -> bool {
            let mut st = self.state.lock().unwrap();
            match st.txs.remove(&slot(source)) {
                Some((_, tx)) => tx.send(Msg::Lost(source)).is_ok(),
                None => false,
            }
        }
    }

    struct FakeStream {
        state: Arc<Mutex<State>>,
        slot: usize,
        generation: u64,
    }

    impl Stream for FakeStream {
        fn stop(self: Box<Self>) {
            let mut st = self.state.lock().unwrap();
            if st.txs.get(&self.slot).map(|(g, _)| *g) == Some(self.generation) {
                st.txs.remove(&self.slot);
            }
        }
    }

    impl Backend for FakeBackend {
        fn open(
            &self,
            kind: SourceKind,
            _origin: Instant,
            _sample_rate: u32,
            tx: Sender<Msg>,
        ) -> Result<Box<dyn Stream>, String> {
            let mut st = self.state.lock().unwrap();
            let fails = match kind {
                SourceKind::Mic { .. } => FailKind::Mic,
                SourceKind::WholeSystem { .. } => FailKind::WholeSystem,
                SourceKind::Process { .. } => FailKind::Process,
            };
            if st.fail.contains(&fails) {
                return Err(format!("fake open failure: {fails:?}"));
            }
            let slot = slot(kind.source());
            st.next_gen += 1;
            let generation = st.next_gen;
            st.txs.insert(slot, (generation, tx));
            st.opened.push(kind);
            Ok(Box::new(FakeStream {
                state: self.state.clone(),
                slot,
                generation,
            }))
        }

        fn meeting_target(&self) -> Option<u32> {
            self.state.lock().unwrap().target
        }

        fn list_devices(&self) -> (Vec<DeviceInfo>, Vec<DeviceInfo>) {
            self.state.lock().unwrap().devices.clone()
        }
    }
}

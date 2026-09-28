//! Application state and commands shared by standalone and plugin frontends.
//!
//! This crate deliberately has no UI, plugin-framework, filesystem, or audio
//! dependencies. Backends publish snapshots and consume commands; frontends
//! only render those snapshots.

mod backend;
mod mock_backend;
mod state;

pub use backend::{AudioBackend, BackendEvent};
pub use ir_eq::{EqBand, EqBandId, EqShape, EqualizerState};
pub use mock_backend::MockAudioBackend;
pub use state::*;

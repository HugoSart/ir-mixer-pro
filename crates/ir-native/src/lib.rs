//! Native standalone services and the production [`ir_app::AudioBackend`].

mod backend;
mod dialog;
mod runtime;
mod worker;

pub use backend::NativeAudioBackend;
pub use runtime::{AudioRuntime, DeviceCatalog, RuntimeConfig, RuntimeError};

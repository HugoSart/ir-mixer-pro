use crate::{AppCommand, AppSnapshot};

#[derive(Clone, Debug, PartialEq)]
pub enum BackendEvent {
    StatusChanged(String),
    PresetSaved,
    ExportCompleted(String),
    ExportFailed(String),
}

/// UI-facing boundary for mocked and native application services.
///
/// Native implementations may refresh `AppSnapshot` from lock-free queues, but
/// calls made through this trait always happen outside the audio callback.
pub trait AudioBackend: Send {
    fn snapshot(&self) -> &AppSnapshot;
    fn dispatch(&mut self, command: AppCommand);
    fn update(&mut self, now_seconds: f64);
    fn drain_events(&mut self) -> Vec<BackendEvent>;
}

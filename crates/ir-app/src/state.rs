use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PRESET_SCHEMA_VERSION: u32 = 3;
const BALANCE_SILENCE_DB: f32 = -144.0;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IrId(pub u64);

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OptionId(pub String);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrontendMode {
    #[default]
    Standalone,
    Plugin,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceMode {
    #[default]
    Preview,
    Live,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AnalysisTab {
    #[default]
    Frequency,
    Impulse,
    Phase,
    Spectrum,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ContentState {
    #[default]
    Ready,
    Loading {
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ExportState {
    #[default]
    Idle,
    Exporting {
        progress: f32,
    },
    Complete {
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub id: OptionId,
    pub label: String,
}

impl Choice {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: OptionId(id.into()),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IrSlotState {
    pub id: IrId,
    pub filename: String,
    pub file_path: Option<PathBuf>,
    #[serde(default)]
    pub file_reference: Option<IrFileReference>,
    pub metadata: String,
    pub color_index: u8,
    pub enabled: bool,
    pub gain_db: f32,
    #[serde(default)]
    pub balance_percent: f32,
    pub delay_samples: i32,
    pub sample_rate_hz: f32,
    pub pan: f32,
    pub polarity_inverted: bool,
    pub normalize: bool,
    pub soloed: bool,
    pub muted: bool,
    #[serde(skip)]
    pub load_state: ContentState,
    #[serde(skip)]
    pub waveform: Vec<f32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IrFileReference {
    pub original_path: PathBuf,
    #[serde(default)]
    pub relative_path: Option<PathBuf>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub content_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceState {
    pub mode: SourceMode,
    pub filename: Option<String>,
    pub file_path: Option<PathBuf>,
    pub metadata: String,
    pub transport: TransportState,
    pub elapsed_seconds: f64,
    pub duration_seconds: f64,
    pub looping: bool,
    pub gain_db: f32,
    pub normalize: bool,
    #[serde(skip)]
    pub device: OptionId,
    #[serde(skip)]
    pub channel: OptionId,
    #[serde(skip)]
    pub sample_rate: OptionId,
    #[serde(skip)]
    pub buffer_size: OptionId,
    #[serde(skip)]
    pub monitoring: bool,
    #[serde(skip)]
    pub content_state: ContentState,
    #[serde(skip)]
    pub waveform: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputState {
    #[serde(skip)]
    pub device: OptionId,
    #[serde(skip)]
    pub channel: OptionId,
    #[serde(skip)]
    pub buffer_size: OptionId,
    pub gain_db: f32,
    pub bypassed: bool,
    pub limit_output: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalysisState {
    pub tab: AnalysisTab,
    pub view_mode: OptionId,
    pub smoothing: OptionId,
    #[serde(skip)]
    pub content_state: ContentState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportSettings {
    pub filename: String,
    #[serde(skip)]
    pub destination: Option<PathBuf>,
    pub sample_rate: OptionId,
    pub bit_depth: OptionId,
    pub channel_mode: OptionId,
    pub length: OptionId,
    pub trim_to_length: bool,
    pub normalize: bool,
    #[serde(skip)]
    pub state: ExportState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectState {
    pub name: String,
    pub ir_slots: Vec<IrSlotState>,
    pub selected_ir: Option<IrId>,
    #[serde(default)]
    pub balance_mode: bool,
    pub source: SourceState,
    pub output: OutputState,
    pub analysis: AnalysisState,
    pub export: ExportSettings,
    #[serde(skip)]
    pub dirty: bool,
}

impl ProjectState {
    pub fn set_balance_mode(&mut self, enabled: bool) {
        self.balance_mode = enabled;
        if enabled {
            self.normalize_balance_from_gains();
        }
    }

    /// Gives each loaded IR an equal share of the 100% balance budget.
    pub fn equalize_balance(&mut self) {
        let count = self.ir_slots.len();
        if count == 0 {
            return;
        }

        let share = 100.0 / count as f32;
        let mut assigned = 0.0;
        for slot in self.ir_slots.iter_mut().take(count - 1) {
            slot.balance_percent = share;
            slot.gain_db = balance_percent_to_gain_db(share);
            assigned += share;
        }
        if let Some(slot) = self.ir_slots.last_mut() {
            slot.balance_percent = 100.0 - assigned;
            slot.gain_db = balance_percent_to_gain_db(slot.balance_percent);
        }
    }

    /// Changes one slot while preserving the relative balance of every other slot.
    /// The final slot receives the rounding remainder so the visible total is 100%.
    pub fn set_balance_percent(&mut self, id: IrId, percent: f32) -> bool {
        let Some(selected_index) = self.ir_slots.iter().position(|slot| slot.id == id) else {
            return false;
        };
        let count = self.ir_slots.len();
        if count == 1 {
            self.equalize_balance();
            return false;
        }

        let percent = percent.clamp(0.0, 100.0);
        let other_indices = (0..count)
            .filter(|index| *index != selected_index)
            .collect::<Vec<_>>();
        let current_other_total: f32 = other_indices
            .iter()
            .map(|index| self.ir_slots[*index].balance_percent.max(0.0))
            .sum();
        let remaining = 100.0 - percent;
        self.ir_slots[selected_index].balance_percent = percent;

        let mut assigned = percent;
        for index in other_indices.iter().take(other_indices.len() - 1) {
            let next = if current_other_total > f32::EPSILON {
                self.ir_slots[*index].balance_percent.max(0.0) / current_other_total * remaining
            } else {
                remaining / other_indices.len() as f32
            };
            self.ir_slots[*index].balance_percent = next;
            assigned += next;
        }
        if let Some(last_index) = other_indices.last() {
            self.ir_slots[*last_index].balance_percent = (100.0 - assigned).max(0.0);
        }
        self.sync_balance_gains();
        true
    }

    fn normalize_balance_from_gains(&mut self) {
        if self.ir_slots.len() <= 1 {
            self.equalize_balance();
            return;
        }
        let total: f32 = self
            .ir_slots
            .iter()
            .map(|slot| 10.0_f32.powf(slot.gain_db / 20.0).max(0.0))
            .sum();
        if total <= f32::EPSILON {
            self.equalize_balance();
            return;
        }
        let mut assigned = 0.0;
        let last = self.ir_slots.len() - 1;
        for slot in self.ir_slots.iter_mut().take(last) {
            slot.balance_percent = 10.0_f32.powf(slot.gain_db / 20.0).max(0.0) / total * 100.0;
            assigned += slot.balance_percent;
        }
        self.ir_slots[last].balance_percent = 100.0 - assigned;
        self.sync_balance_gains();
    }

    fn sync_balance_gains(&mut self) {
        for slot in &mut self.ir_slots {
            slot.gain_db = balance_percent_to_gain_db(slot.balance_percent);
        }
    }
}

pub fn balance_percent_to_gain_db(percent: f32) -> f32 {
    let linear_gain =
        (percent.clamp(0.0, 100.0) / 100.0).max(10.0_f32.powf(BALANCE_SILENCE_DB / 20.0));
    20.0 * linear_gain.log10()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetDocument {
    pub schema_version: u32,
    pub project: ProjectState,
}

impl PresetDocument {
    pub fn new(project: ProjectState) -> Self {
        Self {
            schema_version: PRESET_SCHEMA_VERSION,
            project,
        }
    }

    pub fn validate_version(&self) -> Result<(), PresetVersionError> {
        if self.schema_version == PRESET_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(PresetVersionError {
                found: self.schema_version,
                supported: PRESET_SCHEMA_VERSION,
            })
        }
    }

    pub fn migrate(mut self) -> Result<Self, PresetVersionError> {
        match self.schema_version {
            PRESET_SCHEMA_VERSION => Ok(self),
            2 => {
                self.project.balance_mode = false;
                for slot in &mut self.project.ir_slots {
                    slot.balance_percent = 0.0;
                }
                self.schema_version = PRESET_SCHEMA_VERSION;
                Ok(self)
            }
            1 => {
                for slot in &mut self.project.ir_slots {
                    if slot.file_reference.is_none() {
                        slot.file_reference =
                            slot.file_path.clone().map(|original_path| IrFileReference {
                                original_path,
                                relative_path: None,
                                size_bytes: None,
                                content_fingerprint: None,
                            });
                    }
                }
                self.schema_version = PRESET_SCHEMA_VERSION;
                Ok(self)
            }
            found => Err(PresetVersionError {
                found,
                supported: PRESET_SCHEMA_VERSION,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: u64, gain_db: f32) -> IrSlotState {
        IrSlotState {
            id: IrId(id),
            filename: String::new(),
            file_path: None,
            file_reference: None,
            metadata: String::new(),
            color_index: 0,
            enabled: true,
            gain_db,
            balance_percent: 0.0,
            delay_samples: 0,
            sample_rate_hz: 48_000.0,
            pan: 0.0,
            polarity_inverted: false,
            normalize: false,
            soloed: false,
            muted: false,
            load_state: ContentState::Ready,
            waveform: Vec::new(),
        }
    }

    fn project(gains: &[f32]) -> ProjectState {
        ProjectState {
            name: String::new(),
            ir_slots: gains
                .iter()
                .enumerate()
                .map(|(index, gain)| slot(index as u64 + 1, *gain))
                .collect(),
            selected_ir: None,
            balance_mode: false,
            source: SourceState {
                mode: SourceMode::Preview,
                filename: None,
                file_path: None,
                metadata: String::new(),
                transport: TransportState::Stopped,
                elapsed_seconds: 0.0,
                duration_seconds: 0.0,
                looping: false,
                gain_db: 0.0,
                normalize: false,
                device: OptionId::default(),
                channel: OptionId::default(),
                sample_rate: OptionId::default(),
                buffer_size: OptionId::default(),
                monitoring: false,
                content_state: ContentState::Ready,
                waveform: Vec::new(),
            },
            output: OutputState {
                device: OptionId::default(),
                channel: OptionId::default(),
                buffer_size: OptionId::default(),
                gain_db: 0.0,
                bypassed: false,
                limit_output: false,
            },
            analysis: AnalysisState {
                tab: AnalysisTab::Frequency,
                view_mode: OptionId::default(),
                smoothing: OptionId::default(),
                content_state: ContentState::Ready,
            },
            export: ExportSettings {
                filename: String::new(),
                destination: None,
                sample_rate: OptionId::default(),
                bit_depth: OptionId::default(),
                channel_mode: OptionId::default(),
                length: OptionId::default(),
                trim_to_length: false,
                normalize: false,
                state: ExportState::Idle,
            },
            dirty: false,
        }
    }

    #[test]
    fn equalized_balance_is_always_one_hundred_percent() {
        let mut project = project(&[0.0, 0.0, 0.0]);
        project.equalize_balance();
        let total: f32 = project
            .ir_slots
            .iter()
            .map(|slot| slot.balance_percent)
            .sum();
        assert!((total - 100.0).abs() < 0.000_01);
        assert!((project.ir_slots[0].balance_percent - 33.333_332).abs() < 0.000_01);
        assert!((project.ir_slots[2].balance_percent - 33.333_336).abs() < 0.000_01);
    }

    #[test]
    fn changing_one_balance_share_preserves_the_total() {
        let mut project = project(&[0.0, 0.0, 0.0]);
        project.equalize_balance();
        assert!(project.set_balance_percent(IrId(1), 50.0));
        let total: f32 = project
            .ir_slots
            .iter()
            .map(|slot| slot.balance_percent)
            .sum();
        assert!((total - 100.0).abs() < 0.000_01);
        assert!((project.ir_slots[0].balance_percent - 50.0).abs() < f32::EPSILON);
        assert!((project.ir_slots[1].balance_percent - 25.0).abs() < 0.000_01);
        assert!((project.ir_slots[2].balance_percent - 25.0).abs() < 0.000_01);
    }

    #[test]
    fn a_single_ir_is_locked_to_full_balance() {
        let mut project = project(&[-9.0]);
        project.set_balance_mode(true);
        assert_eq!(project.ir_slots[0].balance_percent, 100.0);
        assert!(!project.set_balance_percent(IrId(1), 25.0));
        assert_eq!(project.ir_slots[0].balance_percent, 100.0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresetVersionError {
    pub found: u32,
    pub supported: u32,
}

impl std::fmt::Display for PresetVersionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unsupported preset schema version {}; this build supports {}",
            self.found, self.supported
        )
    }
}

impl std::error::Error for PresetVersionError {}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisTrace {
    pub label: String,
    pub values: Vec<f32>,
    pub color_index: Option<u8>,
    pub emphasized: bool,
}

/// Transient file-picker and background-load activity exposed to the UI.
///
/// This deliberately lives on `AppSnapshot` rather than in `ProjectState` so
/// presets and plugin state never persist in-progress operations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileLoadActivity {
    pub dialog_open: bool,
    pub preview_loading: bool,
    pub adding_ir_count: usize,
    pub replacing_ir_ids: Vec<IrId>,
}

impl FileLoadActivity {
    pub fn adding_irs(&self) -> bool {
        self.adding_ir_count > 0
    }

    pub fn replacing_ir(&self, id: IrId) -> bool {
        self.replacing_ir_ids.contains(&id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppSnapshot {
    pub project: ProjectState,
    pub presets: Vec<Choice>,
    pub active_preset: OptionId,
    pub input_devices: Vec<Choice>,
    pub input_channels: Vec<Choice>,
    pub sample_rates: Vec<Choice>,
    pub buffer_sizes: Vec<Choice>,
    pub output_devices: Vec<Choice>,
    pub output_channels: Vec<Choice>,
    pub analysis_view_modes: Vec<Choice>,
    pub smoothing_options: Vec<Choice>,
    pub bit_depths: Vec<Choice>,
    pub channel_modes: Vec<Choice>,
    pub export_lengths: Vec<Choice>,
    pub file_load_activity: FileLoadActivity,
    pub frequency_traces: Vec<AnalysisTrace>,
    pub phase_traces: Vec<AnalysisTrace>,
    pub spectrum_traces: Vec<AnalysisTrace>,
    pub combined_waveform: Vec<f32>,
    pub output_levels_db: [f32; 2],
    pub output_peaks_db: [f32; 2],
    pub cpu_percent: f32,
    pub latency_ms: f32,
    pub status: String,
    pub status_is_error: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    SelectPreset(OptionId),
    SavePreset,
    SavePresetAs,
    DeletePreset,
    OpenSettings,
    SetSourceMode(SourceMode),
    BrowsePreview,
    RestartPreview,
    PlayPreview,
    PausePreview,
    StopPreview,
    SetLoop(bool),
    SetInputGainDb(f32),
    SetInputNormalize(bool),
    SetInputDevice(OptionId),
    SetInputChannel(OptionId),
    SetInputBufferSize(OptionId),
    SetMonitoring(bool),
    AddIr,
    ClearAllIrs,
    NormalizeAllIrs,
    SetBalanceMode(bool),
    SelectIr(IrId),
    ReplaceIr(IrId),
    RemoveIr(IrId),
    MoveIrUp(IrId),
    MoveIrDown(IrId),
    SetIrEnabled(IrId, bool),
    SetIrGainDb(IrId, f32),
    SetIrBalancePercent(IrId, f32),
    SetIrDelaySamples(IrId, i32),
    SetIrPan(IrId, f32),
    SetIrPolarity(IrId, bool),
    SetIrNormalize(IrId, bool),
    SetIrSolo(IrId, bool),
    SetIrMute(IrId, bool),
    SetAnalysisTab(AnalysisTab),
    SetAnalysisViewMode(OptionId),
    SetSmoothing(OptionId),
    SetOutputDevice(OptionId),
    SetOutputChannel(OptionId),
    SetOutputBufferSize(OptionId),
    SetOutputGainDb(f32),
    SetBypassed(bool),
    SetLimitOutput(bool),
    ChooseExportDestination,
    SetExportSampleRate(OptionId),
    SetExportBitDepth(OptionId),
    SetExportChannelMode(OptionId),
    SetExportLength(OptionId),
    SetTrimToLength(bool),
    SetExportNormalize(bool),
    Export,
}

pub fn selected_index(choices: &[Choice], selected: &OptionId) -> usize {
    choices
        .iter()
        .position(|choice| &choice.id == selected)
        .unwrap_or(0)
}

pub fn selected_id(choices: &[Choice], index: usize) -> Option<OptionId> {
    choices.get(index).map(|choice| choice.id.clone())
}

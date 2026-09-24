use crossbeam_channel::{Receiver, Sender};
use ir_app::{AnalysisTrace, IrFileReference, IrId, PresetDocument};
use ir_core::{
    AudioBuffer, MixSource, RenderSettings, SampleRate, WavEncoding, analyze_frequency_response,
    read_wav, render_mix, write_wav,
};
use ir_dsp::{EngineConfig, PreparedSlot, SlotParameters};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ExportSource {
    pub label: String,
    pub color_index: Option<u8>,
    pub audio: Arc<AudioBuffer>,
    pub params: SlotParameters,
}

pub(crate) enum WorkerRequest {
    LoadIr {
        id: IrId,
        path: PathBuf,
        config: EngineConfig,
        params: SlotParameters,
    },
    PrepareIr {
        id: IrId,
        source: Arc<AudioBuffer>,
        config: EngineConfig,
        params: SlotParameters,
    },
    LoadPreview {
        path: PathBuf,
        sample_rate: SampleRate,
    },
    Export {
        path: PathBuf,
        sources: Vec<ExportSource>,
        settings: RenderSettings,
        encoding: WavEncoding,
    },
    Analyze {
        generation: u64,
        sources: Vec<ExportSource>,
        settings: RenderSettings,
    },
    SavePreset {
        path: PathBuf,
        document: Box<PresetDocument>,
    },
    LoadPreset {
        path: PathBuf,
    },
    DeletePreset {
        path: PathBuf,
    },
}

pub(crate) enum WorkerResult {
    IrReady {
        id: IrId,
        path: PathBuf,
        source: Arc<AudioBuffer>,
        prepared_sample_rate: u32,
        prepared: Box<PreparedSlot>,
        file_reference: Option<IrFileReference>,
    },
    IrFailed {
        id: IrId,
        message: String,
    },
    PreviewReady {
        path: PathBuf,
        audio: Box<AudioBuffer>,
    },
    PreviewFailed(String),
    ExportComplete(PathBuf),
    ExportFailed(String),
    AnalysisReady {
        generation: u64,
        frequency: Vec<AnalysisTrace>,
        phase: Vec<AnalysisTrace>,
        waveform: Vec<f32>,
    },
    PresetSaved(PathBuf),
    PresetLoaded(PathBuf, Box<PresetDocument>),
    PresetDeleted,
    PresetFailed(String),
}

pub(crate) fn spawn() -> (Sender<WorkerRequest>, Receiver<WorkerResult>) {
    let (requests_tx, requests_rx) = crossbeam_channel::unbounded();
    let (results_tx, results_rx) = crossbeam_channel::unbounded();
    std::thread::Builder::new()
        .name("ir-mixer-worker".into())
        .spawn(move || {
            while let Ok(request) = requests_rx.recv() {
                handle(request, &results_tx);
            }
        })
        .expect("background audio preparation worker should start");
    (requests_tx, results_rx)
}

fn handle(request: WorkerRequest, results: &Sender<WorkerResult>) {
    match request {
        WorkerRequest::LoadIr {
            id,
            path,
            config,
            params,
        } => match read_wav(&path) {
            Ok(source) => prepare(id, path, Arc::new(source), config, params, results),
            Err(error) => {
                let _ = results.send(WorkerResult::IrFailed {
                    id,
                    message: error.to_string(),
                });
            }
        },
        WorkerRequest::PrepareIr {
            id,
            source,
            config,
            params,
        } => prepare(id, PathBuf::new(), source, config, params, results),
        WorkerRequest::LoadPreview { path, sample_rate } => match read_wav(&path)
            .and_then(|audio| audio.resample(sample_rate).map_err(Into::into))
        {
            Ok(audio) => {
                let _ = results.send(WorkerResult::PreviewReady {
                    path,
                    audio: Box::new(audio),
                });
            }
            Err(error) => {
                let _ = results.send(WorkerResult::PreviewFailed(error.to_string()));
            }
        },
        WorkerRequest::Export {
            path,
            sources,
            settings,
            encoding,
        } => {
            let borrowed: Vec<_> = sources
                .iter()
                .map(|source| MixSource {
                    audio: &source.audio,
                    enabled: source.params.enabled,
                    muted: source.params.muted,
                    soloed: source.params.soloed,
                    gain_db: source.params.gain_db,
                    delay_samples: source.params.delay_samples,
                    pan: source.params.pan,
                    polarity_inverted: source.params.polarity_inverted,
                    normalize: source.params.normalize,
                })
                .collect();
            match render_mix(&borrowed, settings) {
                Ok(audio) => match write_wav(&path, &audio, encoding) {
                    Ok(()) => {
                        let _ = results.send(WorkerResult::ExportComplete(path));
                    }
                    Err(error) => {
                        let _ = results.send(WorkerResult::ExportFailed(error.to_string()));
                    }
                },
                Err(error) => {
                    let _ = results.send(WorkerResult::ExportFailed(error.to_string()));
                }
            }
        }
        WorkerRequest::Analyze {
            generation,
            sources,
            settings,
        } => {
            let mut frequency = Vec::new();
            let mut phase = Vec::new();
            for source in &sources {
                if let Ok(audio) = render_sources(std::slice::from_ref(source), settings) {
                    let analysis = analyze_frequency_response(&audio, 180);
                    frequency.push(AnalysisTrace {
                        label: source.label.clone(),
                        values: analysis.magnitude_db,
                        color_index: source.color_index,
                        emphasized: false,
                    });
                    phase.push(AnalysisTrace {
                        label: source.label.clone(),
                        values: analysis.phase_degrees,
                        color_index: source.color_index,
                        emphasized: false,
                    });
                }
            }
            if let Ok(audio) = render_sources(&sources, settings) {
                let analysis = analyze_frequency_response(&audio, 180);
                frequency.push(AnalysisTrace {
                    label: "Sum (Mixed)".into(),
                    values: analysis.magnitude_db,
                    color_index: None,
                    emphasized: true,
                });
                phase.push(AnalysisTrace {
                    label: "Sum (Mixed)".into(),
                    values: analysis.phase_degrees,
                    color_index: None,
                    emphasized: true,
                });
                let _ = results.send(WorkerResult::AnalysisReady {
                    generation,
                    frequency,
                    phase,
                    waveform: audio.waveform_envelope(320),
                });
            } else {
                let _ = results.send(WorkerResult::AnalysisReady {
                    generation,
                    frequency,
                    phase,
                    waveform: Vec::new(),
                });
            }
        }
        WorkerRequest::SavePreset { path, document } => {
            let result = save_preset(&path, &document);
            let _ = results.send(match result {
                Ok(()) => WorkerResult::PresetSaved(path),
                Err(error) => WorkerResult::PresetFailed(error),
            });
        }
        WorkerRequest::LoadPreset { path } => {
            let result = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<PresetDocument>(&json).map_err(|error| error.to_string())
                })
                .and_then(|document| document.migrate().map_err(|error| error.to_string()))
                .map(|mut document| {
                    let preset_directory =
                        path.parent().unwrap_or_else(|| std::path::Path::new("."));
                    for slot in &mut document.project.ir_slots {
                        if slot
                            .file_path
                            .as_ref()
                            .is_some_and(|candidate| candidate.exists())
                        {
                            continue;
                        }
                        if let Some(reference) = &slot.file_reference {
                            let relative = reference
                                .relative_path
                                .as_ref()
                                .map(|relative| preset_directory.join(relative));
                            slot.file_path = relative
                                .filter(|candidate| candidate.exists())
                                .or_else(|| {
                                    reference
                                        .original_path
                                        .exists()
                                        .then(|| reference.original_path.clone())
                                })
                                .or_else(|| Some(reference.original_path.clone()));
                        }
                    }
                    document
                });
            let _ = results.send(match result {
                Ok(document) => WorkerResult::PresetLoaded(path, Box::new(document)),
                Err(error) => WorkerResult::PresetFailed(error),
            });
        }
        WorkerRequest::DeletePreset { path } => {
            let result = std::fs::remove_file(&path).map_err(|error| error.to_string());
            let _ = results.send(match result {
                Ok(()) => WorkerResult::PresetDeleted,
                Err(error) => WorkerResult::PresetFailed(error),
            });
        }
    }
}

fn render_sources(
    sources: &[ExportSource],
    settings: RenderSettings,
) -> Result<AudioBuffer, ir_core::RenderError> {
    let borrowed: Vec<_> = sources
        .iter()
        .map(|source| MixSource {
            audio: &source.audio,
            enabled: source.params.enabled,
            muted: source.params.muted,
            soloed: source.params.soloed,
            gain_db: source.params.gain_db,
            delay_samples: source.params.delay_samples,
            pan: source.params.pan,
            polarity_inverted: source.params.polarity_inverted,
            normalize: source.params.normalize,
        })
        .collect();
    render_mix(&borrowed, settings)
}

fn prepare(
    id: IrId,
    path: PathBuf,
    source: Arc<AudioBuffer>,
    config: EngineConfig,
    params: SlotParameters,
    results: &Sender<WorkerResult>,
) {
    let prepared_audio = match source.resample(SampleRate(config.sample_rate)) {
        Ok(audio) => audio,
        Err(error) => {
            let _ = results.send(WorkerResult::IrFailed {
                id,
                message: error.to_string(),
            });
            return;
        }
    };
    match PreparedSlot::new(id.0, &prepared_audio, config, params) {
        Ok(prepared) => {
            let file_reference = if path.as_os_str().is_empty() {
                None
            } else {
                Some(IrFileReference {
                    original_path: path.clone(),
                    relative_path: None,
                    size_bytes: std::fs::metadata(&path).ok().map(|metadata| metadata.len()),
                    content_fingerprint: Some(audio_fingerprint(&source)),
                })
            };
            let _ = results.send(WorkerResult::IrReady {
                id,
                path,
                source,
                prepared_sample_rate: config.sample_rate,
                prepared: Box::new(prepared),
                file_reference,
            });
        }
        Err(error) => {
            let _ = results.send(WorkerResult::IrFailed {
                id,
                message: error.to_string(),
            });
        }
    }
}

fn audio_fingerprint(audio: &AudioBuffer) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in audio
        .sample_rate()
        .0
        .to_le_bytes()
        .into_iter()
        .chain((audio.channel_count() as u32).to_le_bytes())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for sample in audio.channels().iter().flatten() {
        for byte in sample.to_bits().to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn save_preset(path: &PathBuf, document: &PresetDocument) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(document).map_err(|error| error.to_string())?;
    std::fs::write(&temp, json).map_err(|error| error.to_string())?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    std::fs::rename(temp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_source(
        source: Arc<AudioBuffer>,
        sample_rate: u32,
    ) -> (Arc<AudioBuffer>, Box<PreparedSlot>) {
        let (results_tx, results_rx) = crossbeam_channel::bounded(1);
        prepare(
            IrId(7),
            PathBuf::new(),
            source,
            EngineConfig {
                sample_rate,
                block_size: 128,
                ..EngineConfig::default()
            },
            SlotParameters::default(),
            &results_tx,
        );
        match results_rx.recv().expect("worker should return a result") {
            WorkerResult::IrReady {
                source, prepared, ..
            } => (source, prepared),
            WorkerResult::IrFailed { message, .. } => panic!("IR preparation failed: {message}"),
            _ => panic!("unexpected worker result"),
        }
    }

    #[test]
    fn engine_preparation_preserves_the_native_rate_source() {
        let original = Arc::new(AudioBuffer::mono(SampleRate(44_100), vec![0.0; 22_050]));

        let (after_48k, _) = prepare_source(Arc::clone(&original), 48_000);
        let (after_96k, _) = prepare_source(Arc::clone(&after_48k), 96_000);

        assert!(Arc::ptr_eq(&original, &after_48k));
        assert!(Arc::ptr_eq(&original, &after_96k));
        assert_eq!(after_96k.sample_rate(), SampleRate(44_100));
        assert_eq!(after_96k.frame_count(), 22_050);
    }
}

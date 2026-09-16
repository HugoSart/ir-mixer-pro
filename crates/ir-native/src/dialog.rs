use crossbeam_channel::{Receiver, Sender};
use ir_app::IrId;
use std::path::PathBuf;

pub enum DialogRequest {
    Preview,
    Irs {
        replacement: Option<IrId>,
    },
    Export {
        filename: String,
    },
    SavePreset {
        directory: PathBuf,
        filename: String,
    },
}

pub enum DialogResult {
    Preview(Option<PathBuf>),
    Irs {
        replacement: Option<IrId>,
        paths: Vec<PathBuf>,
    },
    Export(Option<PathBuf>),
    SavePreset(Option<PathBuf>),
}

pub fn spawn() -> (Sender<DialogRequest>, Receiver<DialogResult>) {
    let (requests_tx, requests_rx) = crossbeam_channel::bounded(8);
    let (results_tx, results_rx) = crossbeam_channel::bounded(8);
    std::thread::Builder::new()
        .name("ir-file-dialog".into())
        .spawn(move || run(requests_rx, results_tx))
        .expect("file dialog worker should start");
    (requests_tx, results_rx)
}

fn run(requests: Receiver<DialogRequest>, results: Sender<DialogResult>) {
    while let Ok(request) = requests.recv() {
        let result = match request {
            DialogRequest::Preview => DialogResult::Preview(
                rfd::FileDialog::new()
                    .add_filter("WAV audio", &["wav"])
                    .pick_file(),
            ),
            DialogRequest::Irs { replacement } => {
                let paths = if replacement.is_some() {
                    rfd::FileDialog::new()
                        .add_filter("Impulse response WAV", &["wav"])
                        .pick_file()
                        .into_iter()
                        .collect()
                } else {
                    rfd::FileDialog::new()
                        .add_filter("Impulse response WAV", &["wav"])
                        .pick_files()
                        .unwrap_or_default()
                };
                DialogResult::Irs { replacement, paths }
            }
            DialogRequest::Export { filename } => DialogResult::Export(
                rfd::FileDialog::new()
                    .add_filter("WAV audio", &["wav"])
                    .set_file_name(filename)
                    .save_file(),
            ),
            DialogRequest::SavePreset {
                directory,
                filename,
            } => DialogResult::SavePreset(
                rfd::FileDialog::new()
                    .add_filter("IR Mixer preset", &["json"])
                    .set_directory(directory)
                    .set_file_name(filename)
                    .save_file(),
            ),
        };
        if results.send(result).is_err() {
            break;
        }
    }
}

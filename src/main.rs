#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ir_app::{AudioBackend, FrontendMode, MockAudioBackend};
use ir_native::NativeAudioBackend;
use ir_ui::app::AppPage;
use nice_plug_egui::{App, EguiWindow, EguiWindowSettings, Frame, baseview::dpi::LogicalSize};

const PRODUCT_NAME: &str = "IR Mixer Pro";

fn main() {
    EguiWindow::create(
        EguiWindowSettings::new()
            .with_title(PRODUCT_NAME)
            .with_size(LogicalSize {
                width: 1536.0,
                height: 1024.0,
            })
            .with_min_size(Some(LogicalSize {
                width: 880.0,
                height: 680.0,
            })),
        IrMixerApplication::default(),
    )
    .expect("IR Mixer application window should open")
    .run_until_closed()
    .expect("IR Mixer application should close cleanly");
}

struct IrMixerApplication {
    backend: Box<dyn AudioBackend>,
    frontend_mode: FrontendMode,
}

impl Default for IrMixerApplication {
    fn default() -> Self {
        let backend: Box<dyn AudioBackend> = if std::env::var_os("IR_MIXER_MOCK").is_some() {
            Box::new(MockAudioBackend::default())
        } else {
            Box::new(NativeAudioBackend::default())
        };
        Self {
            backend,
            frontend_mode: FrontendMode::Standalone,
        }
    }
}

impl App for IrMixerApplication {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        frame: &mut Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        egui_extras::install_image_loaders(&egui_ctx);
        ir_ui::install(&egui_ctx);
        ir_ui::native_window::configure_borderless(
            &egui_ctx,
            frame.baseview_window(),
            PRODUCT_NAME,
        );
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let now = ui.input(|input| input.time);
        self.backend.update(now);
        let commands = AppPage::new(self.backend.snapshot())
            .frontend_mode(self.frontend_mode)
            .show(ui);
        for command in commands {
            self.backend.dispatch(command);
        }
        self.backend.drain_events();
        ui.ctx().request_repaint();
    }
}

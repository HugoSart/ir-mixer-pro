use ir_app::{AudioBackend, FrontendMode, MockAudioBackend};
use ir_ui::app::AppPage;
use nice_plug_egui::{App, EguiWindow, EguiWindowSettings, Frame, baseview::dpi::LogicalSize};

fn main() {
    EguiWindow::create(
        EguiWindowSettings::new()
            .with_title("IR Mixer Pro")
            .with_size(LogicalSize {
                width: 1536.0,
                height: 1024.0,
            })
            .with_min_size(Some(LogicalSize {
                width: 880.0,
                height: 680.0,
            })),
        MockApplication::default(),
    )
    .expect("IR Mixer mock application window should open")
    .run_until_closed()
    .expect("IR Mixer mock application should close cleanly");
}

struct MockApplication {
    backend: MockAudioBackend,
    frontend_mode: FrontendMode,
}

impl Default for MockApplication {
    fn default() -> Self {
        Self {
            backend: MockAudioBackend::default(),
            frontend_mode: FrontendMode::Standalone,
        }
    }
}

impl App for MockApplication {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        _frame: &mut Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        egui_extras::install_image_loaders(&egui_ctx);
        ir_ui::install(&egui_ctx);
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

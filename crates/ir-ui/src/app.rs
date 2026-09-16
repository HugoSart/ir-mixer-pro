//! Complete application-page composition over the shared application snapshot.

use crate::{
    DesignSystem, IR_COLORS,
    components::{
        AnalysisPreviewAction, AnalysisPreviewCard, AnalysisPreviewCardView, AnalysisTraceView,
        ContentStatusView, ExportMixedIrAction, ExportMixedIrCard, ExportMixedIrCardView,
        ExportStatusView, InputSourceAction, InputSourceCard, InputSourceCardView, IrRackAction,
        IrRackCard, IrRackCardView, IrRackSlotView, OutputAction, OutputCard, OutputCardView,
        SourceMode as UiSourceMode, StatusBar, StatusBarView, TopBar, TopBarAction, TopBarView,
        TransportState as UiTransportState,
    },
};
use egui::{ScrollArea, Ui};
use ir_app::{
    AnalysisTab, AppCommand, AppSnapshot, Choice, ContentState, ExportState, FrontendMode, IrId,
    SourceMode, TransportState, selected_id, selected_index,
};

#[derive(Clone, Copy, Debug)]
pub struct AppLayoutConfig {
    pub wide_breakpoint: f32,
    pub medium_breakpoint: f32,
    pub source_width: f32,
    pub right_rail_width: f32,
    pub gap: f32,
    pub page_margin: f32,
    pub rack_height: f32,
    pub graph_height: f32,
    pub lower_card_height: f32,
}

impl Default for AppLayoutConfig {
    fn default() -> Self {
        Self {
            wide_breakpoint: 1320.0,
            medium_breakpoint: 980.0,
            source_width: 280.0,
            right_rail_width: 300.0,
            gap: 8.0,
            page_margin: 8.0,
            rack_height: 460.0,
            graph_height: 260.0,
            lower_card_height: 560.0,
        }
    }
}

pub struct AppPage<'a> {
    snapshot: &'a AppSnapshot,
    frontend_mode: FrontendMode,
    layout: AppLayoutConfig,
}

impl<'a> AppPage<'a> {
    pub fn new(snapshot: &'a AppSnapshot) -> Self {
        Self {
            snapshot,
            frontend_mode: FrontendMode::Standalone,
            layout: AppLayoutConfig::default(),
        }
    }

    pub fn frontend_mode(mut self, mode: FrontendMode) -> Self {
        self.frontend_mode = mode;
        self
    }

    pub fn layout(mut self, layout: AppLayoutConfig) -> Self {
        self.layout = layout;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Vec<AppCommand> {
        let ds = DesignSystem::from_context(ui.ctx());
        let mut commands = Vec::new();
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new()
            .fill(ds.colors.surface_toolbar)
            .inner_margin(egui::Margin::symmetric(self.layout.page_margin as i8, 8))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 52.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        commands.extend(self.show_top_bar(ui));
                    },
                );
            });

        let status_height = 34.0;
        let body_height = (ui.available_height() - status_height).max(300.0);
        egui::Frame::new()
            .fill(ds.colors.surface_canvas)
            .inner_margin(egui::Margin::same(self.layout.page_margin as i8))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("application_page")
                    .max_height(body_height - self.layout.page_margin * 2.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        commands.extend(self.show_body(ui));
                    });
            });

        egui::Frame::new()
            .fill(ds.colors.surface_toolbar)
            .inner_margin(egui::Margin::symmetric(self.layout.page_margin as i8, 8))
            .show(ui, |ui| self.show_status_bar(ui));
        if self.frontend_mode == FrontendMode::Standalone {
            crate::native_window::show_resize_handles(ui);
        }
        commands
    }

    fn show_top_bar(&self, ui: &mut Ui) -> Vec<AppCommand> {
        let names = labels(&self.snapshot.presets);
        let view = TopBarView {
            product_name: "IR Mixer Pro",
            version: "v1.0.0",
            tagline: "Mix. Preview. Play Live. Export.",
            preset_names: &names,
            preset: selected_index(&self.snapshot.presets, &self.snapshot.active_preset),
            dirty: self.snapshot.project.dirty,
            cpu_percent: self.snapshot.cpu_percent,
            window_controls: self.frontend_mode == FrontendMode::Standalone,
        };
        TopBar::new("application_top_bar", &view)
            .show(ui)
            .into_iter()
            .filter_map(|action| match action {
                TopBarAction::SelectPreset(index) => {
                    selected_id(&self.snapshot.presets, index).map(AppCommand::SelectPreset)
                }
                TopBarAction::Save => Some(AppCommand::SavePreset),
                TopBarAction::SaveAs => Some(AppCommand::SavePresetAs),
                TopBarAction::Delete => Some(AppCommand::DeletePreset),
                TopBarAction::OpenSettings => Some(AppCommand::OpenSettings),
                TopBarAction::BeginWindowDrag => {
                    crate::native_window::request(
                        ui.ctx(),
                        crate::native_window::NativeWindowAction::BeginDrag,
                    );
                    None
                }
                TopBarAction::MinimizeWindow => {
                    crate::native_window::request(
                        ui.ctx(),
                        crate::native_window::NativeWindowAction::Minimize,
                    );
                    None
                }
                TopBarAction::ToggleMaximizeWindow => {
                    crate::native_window::request(
                        ui.ctx(),
                        crate::native_window::NativeWindowAction::ToggleMaximize,
                    );
                    None
                }
                TopBarAction::CloseWindow => {
                    crate::native_window::request(
                        ui.ctx(),
                        crate::native_window::NativeWindowAction::Close,
                    );
                    None
                }
            })
            .collect()
    }

    fn show_body(&self, ui: &mut Ui) -> Vec<AppCommand> {
        // egui's floating vertical scrollbar occupies the trailing edge of the
        // scroll area. Reserve that strip so the right rail and its border stay
        // fully inside the viewport when scrolling is active.
        let scroll = &ui.spacing().scroll;
        let scrollbar_reserve = scroll.bar_width + scroll.bar_inner_margin;
        let width = (ui.available_width() - scrollbar_reserve).max(280.0);
        let mut commands = Vec::new();
        if width >= self.layout.wide_breakpoint {
            let right_width = self.layout.right_rail_width;
            let main_width = width - right_width - self.layout.gap + scrollbar_reserve;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.spacing_mut().item_spacing.x = self.layout.gap;
                ui.allocate_ui_with_layout(
                    egui::vec2(right_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = self.layout.gap;
                        ui.set_width(right_width);
                        commands.extend(self.show_output(
                            ui,
                            right_width,
                            self.layout.rack_height + 82.0,
                        ));
                        commands.extend(self.show_export(
                            ui,
                            right_width,
                            self.layout.lower_card_height,
                        ));
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(main_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = self.layout.gap;
                        ui.set_width(main_width);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            ui.spacing_mut().item_spacing.x = self.layout.gap;
                            let rack_width =
                                main_width - self.layout.source_width - self.layout.gap;
                            commands.extend(self.show_rack(ui, rack_width));
                            commands.extend(self.show_input(ui, self.layout.source_width));
                        });
                        commands.extend(self.show_analysis(
                            ui,
                            main_width - self.layout.gap,
                            self.layout.lower_card_height,
                        ));
                    },
                );
            });
        } else if width >= self.layout.medium_breakpoint {
            let side_width = self.layout.right_rail_width;
            let main_width = width - side_width - self.layout.gap;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = self.layout.gap;
                ui.allocate_ui_with_layout(
                    egui::vec2(side_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = self.layout.gap;
                        ui.set_width(side_width);
                        commands.extend(self.show_input(ui, side_width));
                        commands.extend(self.show_output(ui, side_width, 0.0));
                        commands.extend(self.show_export(ui, side_width, 0.0));
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(main_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = self.layout.gap;
                        ui.set_width(main_width);
                        commands.extend(self.show_rack(ui, main_width));
                        commands.extend(self.show_analysis(ui, main_width, 0.0));
                    },
                );
            });
        } else {
            ui.spacing_mut().item_spacing.y = self.layout.gap;
            let narrow_width = width.max(280.0);
            commands.extend(self.show_input(ui, narrow_width.min(440.0)));
            ScrollArea::horizontal()
                .id_salt("narrow_rack")
                .show(ui, |ui| {
                    commands.extend(self.show_rack(ui, narrow_width.max(640.0)));
                });
            commands.extend(self.show_output(ui, narrow_width.min(440.0), 0.0));
            ScrollArea::horizontal()
                .id_salt("narrow_analysis")
                .show(ui, |ui| {
                    commands.extend(self.show_analysis(ui, narrow_width.max(640.0), 0.0));
                });
            commands.extend(self.show_export(ui, narrow_width.min(440.0), 0.0));
        }
        commands
    }

    fn show_input(&self, ui: &mut Ui, width: f32) -> Vec<AppCommand> {
        let source = &self.snapshot.project.source;
        let devices = labels(&self.snapshot.input_devices);
        let channels = labels(&self.snapshot.input_channels);
        let sample_rates = labels(&self.snapshot.sample_rates);
        let buffer_sizes = labels(&self.snapshot.buffer_sizes);
        let view = InputSourceCardView {
            mode: match source.mode {
                SourceMode::Preview => UiSourceMode::Preview,
                SourceMode::Live => UiSourceMode::Live,
            },
            filename: source.filename.as_deref(),
            metadata: &source.metadata,
            waveform: &source.waveform,
            transport: match source.transport {
                TransportState::Stopped => UiTransportState::Stopped,
                TransportState::Playing => UiTransportState::Playing,
                TransportState::Paused => UiTransportState::Paused,
            },
            elapsed_seconds: source.elapsed_seconds,
            duration_seconds: source.duration_seconds,
            looping: source.looping,
            gain_db: source.gain_db,
            normalize: source.normalize,
            devices: &devices,
            device: selected_index(&self.snapshot.input_devices, &source.device),
            channels: &channels,
            channel: selected_index(&self.snapshot.input_channels, &source.channel),
            sample_rates: &sample_rates,
            sample_rate: selected_index(&self.snapshot.sample_rates, &source.sample_rate),
            buffer_sizes: &buffer_sizes,
            buffer_size: selected_index(&self.snapshot.buffer_sizes, &source.buffer_size),
            monitoring: source.monitoring,
            standalone_routing: self.frontend_mode == FrontendMode::Standalone,
            browse_enabled: !self.snapshot.file_load_activity.dialog_open
                && !self.snapshot.file_load_activity.preview_loading,
            browse_loading: self.snapshot.file_load_activity.preview_loading,
            content_status: content_status(&source.content_state),
        };
        InputSourceCard::new("application_input", &view)
            .width(width)
            .min_height(self.layout.rack_height + 80.0)
            .show(ui)
            .inner
            .into_iter()
            .filter_map(|action| self.map_input_action(action))
            .collect()
    }

    fn show_rack(&self, ui: &mut Ui, width: f32) -> Vec<AppCommand> {
        let slots = self
            .snapshot
            .project
            .ir_slots
            .iter()
            .enumerate()
            .map(|(index, slot)| IrRackSlotView {
                id: slot.id.0,
                number: index + 1,
                filename: &slot.filename,
                metadata: &slot.metadata,
                waveform: &slot.waveform,
                color: IR_COLORS[slot.color_index as usize % IR_COLORS.len()],
                enabled: slot.enabled,
                gain_db: slot.gain_db,
                delay_samples: slot.delay_samples,
                sample_rate: slot.sample_rate_hz,
                pan: slot.pan,
                polarity_inverted: slot.polarity_inverted,
                normalize: slot.normalize,
                soloed: slot.soloed,
                muted: slot.muted,
                replace_enabled: !self.snapshot.file_load_activity.dialog_open
                    && !self.snapshot.file_load_activity.replacing_ir(slot.id),
                replace_loading: self.snapshot.file_load_activity.replacing_ir(slot.id),
                load_status: content_status(&slot.load_state),
            })
            .collect::<Vec<_>>();
        let view = IrRackCardView {
            slots: &slots,
            selected: self.snapshot.project.selected_ir.map(|id| id.0),
            add_enabled: !self.snapshot.file_load_activity.dialog_open
                && !self.snapshot.file_load_activity.adding_irs(),
            add_loading: self.snapshot.file_load_activity.adding_irs(),
        };
        IrRackCard::new("application_ir_rack", &view)
            .width(width)
            .rack_height(self.layout.rack_height + 2.0)
            .show(ui)
            .inner
            .into_iter()
            .map(map_rack_action)
            .collect()
    }

    fn show_analysis(&self, ui: &mut Ui, width: f32, min_height: f32) -> Vec<AppCommand> {
        let analysis = &self.snapshot.project.analysis;
        let view_modes = labels(&self.snapshot.analysis_view_modes);
        let smoothing = labels(&self.snapshot.smoothing_options);
        let frequency = self.analysis_traces(ui, &self.snapshot.frequency_traces);
        let phase = self.analysis_traces(ui, &self.snapshot.phase_traces);
        let spectrum = self.analysis_traces(ui, &self.snapshot.spectrum_traces);
        let sample_rate = choice_label(
            &self.snapshot.sample_rates,
            &self.snapshot.project.source.sample_rate,
        );
        let ir_length = choice_label(
            &self.snapshot.export_lengths,
            &self.snapshot.project.export.length,
        );
        let latency = format!("{:.1} ms", self.snapshot.latency_ms);
        let cpu = format!("{:.1}%", self.snapshot.cpu_percent);
        let view = AnalysisPreviewCardView {
            selected_tab: analysis_tab_index(analysis.tab),
            view_modes: &view_modes,
            view_mode: selected_index(&self.snapshot.analysis_view_modes, &analysis.view_mode),
            smoothing_options: &smoothing,
            smoothing: selected_index(&self.snapshot.smoothing_options, &analysis.smoothing),
            frequency_traces: &frequency,
            impulse_waveform: &self.snapshot.combined_waveform,
            impulse_color: DesignSystem::from_context(ui.ctx()).colors.accent,
            phase_traces: &phase,
            spectrum_traces: &spectrum,
            sample_rate,
            ir_length,
            latency: &latency,
            cpu: &cpu,
            content_status: content_status(&analysis.content_state),
        };
        AnalysisPreviewCard::new("application_analysis", &view)
            .width(width)
            .graph_height(self.layout.graph_height)
            .min_height(min_height)
            .show(ui)
            .inner
            .into_iter()
            .filter_map(|action| match action {
                AnalysisPreviewAction::SetTab(index) => {
                    Some(AppCommand::SetAnalysisTab(index_to_analysis_tab(index)))
                }
                AnalysisPreviewAction::SetViewMode(index) => {
                    selected_id(&self.snapshot.analysis_view_modes, index)
                        .map(AppCommand::SetAnalysisViewMode)
                }
                AnalysisPreviewAction::SetSmoothing(index) => {
                    selected_id(&self.snapshot.smoothing_options, index)
                        .map(AppCommand::SetSmoothing)
                }
            })
            .collect()
    }

    fn show_output(&self, ui: &mut Ui, width: f32, min_height: f32) -> Vec<AppCommand> {
        let output = &self.snapshot.project.output;
        let devices = labels(&self.snapshot.output_devices);
        let channels = labels(&self.snapshot.output_channels);
        let buffers = labels(&self.snapshot.buffer_sizes);
        let view = OutputCardView {
            devices: &devices,
            device: selected_index(&self.snapshot.output_devices, &output.device),
            channels: &channels,
            channel: selected_index(&self.snapshot.output_channels, &output.channel),
            buffer_sizes: &buffers,
            buffer_size: selected_index(&self.snapshot.buffer_sizes, &output.buffer_size),
            gain_db: output.gain_db,
            bypassed: output.bypassed,
            limit_output: output.limit_output,
            levels_db: &self.snapshot.output_levels_db,
            peaks_db: &self.snapshot.output_peaks_db,
            standalone_routing: self.frontend_mode == FrontendMode::Standalone,
        };
        OutputCard::new("application_output", &view)
            .width(width)
            .min_height(min_height)
            .show(ui)
            .inner
            .into_iter()
            .filter_map(|action| self.map_output_action(action))
            .collect()
    }

    fn show_export(&self, ui: &mut Ui, width: f32, min_height: f32) -> Vec<AppCommand> {
        let export = &self.snapshot.project.export;
        let sample_rates = labels(&self.snapshot.sample_rates);
        let bit_depths = labels(&self.snapshot.bit_depths);
        let channel_modes = labels(&self.snapshot.channel_modes);
        let lengths = labels(&self.snapshot.export_lengths);
        let metadata = format!(
            "{} | {} | WAV",
            choice_label(&self.snapshot.sample_rates, &export.sample_rate),
            choice_label(&self.snapshot.bit_depths, &export.bit_depth)
        );
        let view = ExportMixedIrCardView {
            filename: &export.filename,
            metadata: &metadata,
            sample_rates: &sample_rates,
            sample_rate: selected_index(&self.snapshot.sample_rates, &export.sample_rate),
            bit_depths: &bit_depths,
            bit_depth: selected_index(&self.snapshot.bit_depths, &export.bit_depth),
            channel_modes: &channel_modes,
            channel_mode: selected_index(&self.snapshot.channel_modes, &export.channel_mode),
            lengths: &lengths,
            length: selected_index(&self.snapshot.export_lengths, &export.length),
            trim_to_length: export.trim_to_length,
            normalize: export.normalize,
            status: export_status(&export.state),
        };
        ExportMixedIrCard::new("application_export", &view)
            .width(width)
            .min_height(min_height)
            .show(ui)
            .inner
            .into_iter()
            .filter_map(|action| self.map_export_action(action))
            .collect()
    }

    fn show_status_bar(&self, ui: &mut Ui) {
        let source = &self.snapshot.project.source;
        let active_irs = self
            .snapshot
            .project
            .ir_slots
            .iter()
            .filter(|slot| slot.enabled && !slot.muted)
            .count();
        let view = StatusBarView {
            message: &self.snapshot.status,
            is_error: self.snapshot.status_is_error,
            sample_rate: choice_label(&self.snapshot.sample_rates, &source.sample_rate),
            buffer_size: choice_label(&self.snapshot.buffer_sizes, &source.buffer_size),
            active_irs,
            total_irs: self.snapshot.project.ir_slots.len(),
        };
        StatusBar::new(&view).show(ui);
    }

    fn analysis_traces<'b>(
        &self,
        ui: &Ui,
        traces: &'b [ir_app::AnalysisTrace],
    ) -> Vec<AnalysisTraceView<'b>> {
        let mixed = DesignSystem::from_context(ui.ctx()).colors.text_primary;
        traces
            .iter()
            .map(|trace| AnalysisTraceView {
                label: &trace.label,
                values: &trace.values,
                color: trace
                    .color_index
                    .map(|index| IR_COLORS[index as usize % IR_COLORS.len()])
                    .unwrap_or(mixed),
                emphasized: trace.emphasized,
            })
            .collect()
    }

    fn map_input_action(&self, action: InputSourceAction) -> Option<AppCommand> {
        Some(match action {
            InputSourceAction::SetMode(mode) => AppCommand::SetSourceMode(match mode {
                UiSourceMode::Preview => SourceMode::Preview,
                UiSourceMode::Live => SourceMode::Live,
            }),
            InputSourceAction::Browse => AppCommand::BrowsePreview,
            InputSourceAction::Restart => AppCommand::RestartPreview,
            InputSourceAction::Play => AppCommand::PlayPreview,
            InputSourceAction::Pause => AppCommand::PausePreview,
            InputSourceAction::Stop => AppCommand::StopPreview,
            InputSourceAction::SetLoop(value) => AppCommand::SetLoop(value),
            InputSourceAction::SetGainDb(value) => AppCommand::SetInputGainDb(value),
            InputSourceAction::SetNormalize(value) => AppCommand::SetInputNormalize(value),
            InputSourceAction::SetDevice(index) => {
                AppCommand::SetInputDevice(selected_id(&self.snapshot.input_devices, index)?)
            }
            InputSourceAction::SetChannel(index) => {
                AppCommand::SetInputChannel(selected_id(&self.snapshot.input_channels, index)?)
            }
            InputSourceAction::SetSampleRate(index) => {
                AppCommand::SetSampleRate(selected_id(&self.snapshot.sample_rates, index)?)
            }
            InputSourceAction::SetBufferSize(index) => {
                AppCommand::SetInputBufferSize(selected_id(&self.snapshot.buffer_sizes, index)?)
            }
            InputSourceAction::SetMonitoring(value) => AppCommand::SetMonitoring(value),
        })
    }

    fn map_output_action(&self, action: OutputAction) -> Option<AppCommand> {
        Some(match action {
            OutputAction::SetDevice(index) => {
                AppCommand::SetOutputDevice(selected_id(&self.snapshot.output_devices, index)?)
            }
            OutputAction::SetChannel(index) => {
                AppCommand::SetOutputChannel(selected_id(&self.snapshot.output_channels, index)?)
            }
            OutputAction::SetBufferSize(index) => {
                AppCommand::SetOutputBufferSize(selected_id(&self.snapshot.buffer_sizes, index)?)
            }
            OutputAction::SetGainDb(value) => AppCommand::SetOutputGainDb(value),
            OutputAction::SetBypass(value) => AppCommand::SetBypassed(value),
            OutputAction::SetLimitOutput(value) => AppCommand::SetLimitOutput(value),
        })
    }

    fn map_export_action(&self, action: ExportMixedIrAction) -> Option<AppCommand> {
        Some(match action {
            ExportMixedIrAction::ChooseDestination => AppCommand::ChooseExportDestination,
            ExportMixedIrAction::SetSampleRate(index) => {
                AppCommand::SetExportSampleRate(selected_id(&self.snapshot.sample_rates, index)?)
            }
            ExportMixedIrAction::SetBitDepth(index) => {
                AppCommand::SetExportBitDepth(selected_id(&self.snapshot.bit_depths, index)?)
            }
            ExportMixedIrAction::SetChannelMode(index) => {
                AppCommand::SetExportChannelMode(selected_id(&self.snapshot.channel_modes, index)?)
            }
            ExportMixedIrAction::SetLength(index) => {
                AppCommand::SetExportLength(selected_id(&self.snapshot.export_lengths, index)?)
            }
            ExportMixedIrAction::SetTrimToLength(value) => AppCommand::SetTrimToLength(value),
            ExportMixedIrAction::SetNormalize(value) => AppCommand::SetExportNormalize(value),
            ExportMixedIrAction::Export => AppCommand::Export,
        })
    }
}

fn map_rack_action(action: IrRackAction) -> AppCommand {
    match action {
        IrRackAction::AddIr => AppCommand::AddIr,
        IrRackAction::ClearAll => AppCommand::ClearAllIrs,
        IrRackAction::NormalizeAll => AppCommand::NormalizeAllIrs,
        IrRackAction::Select { id } => AppCommand::SelectIr(IrId(id)),
        IrRackAction::Browse { id } => AppCommand::ReplaceIr(IrId(id)),
        IrRackAction::Remove { id } => AppCommand::RemoveIr(IrId(id)),
        IrRackAction::MoveUp { id } => AppCommand::MoveIrUp(IrId(id)),
        IrRackAction::MoveDown { id } => AppCommand::MoveIrDown(IrId(id)),
        IrRackAction::SetEnabled { id, enabled } => AppCommand::SetIrEnabled(IrId(id), enabled),
        IrRackAction::SetGainDb { id, gain_db } => AppCommand::SetIrGainDb(IrId(id), gain_db),
        IrRackAction::SetDelaySamples { id, delay_samples } => {
            AppCommand::SetIrDelaySamples(IrId(id), delay_samples)
        }
        IrRackAction::SetPan { id, pan } => AppCommand::SetIrPan(IrId(id), pan),
        IrRackAction::SetPolarity { id, inverted } => AppCommand::SetIrPolarity(IrId(id), inverted),
        IrRackAction::SetNormalize { id, normalize } => {
            AppCommand::SetIrNormalize(IrId(id), normalize)
        }
        IrRackAction::SetSolo { id, soloed } => AppCommand::SetIrSolo(IrId(id), soloed),
        IrRackAction::SetMute { id, muted } => AppCommand::SetIrMute(IrId(id), muted),
    }
}

fn labels(choices: &[Choice]) -> Vec<&str> {
    choices.iter().map(|choice| choice.label.as_str()).collect()
}

fn choice_label<'a>(choices: &'a [Choice], selected: &ir_app::OptionId) -> &'a str {
    choices
        .iter()
        .find(|choice| &choice.id == selected)
        .or_else(|| choices.first())
        .map(|choice| choice.label.as_str())
        .unwrap_or("Unavailable")
}

fn content_status(state: &ContentState) -> ContentStatusView<'_> {
    match state {
        ContentState::Ready => ContentStatusView::Ready,
        ContentState::Loading { message } => ContentStatusView::Loading(message),
        ContentState::Error { message } => ContentStatusView::Error(message),
    }
}

fn export_status(state: &ExportState) -> ExportStatusView<'_> {
    match state {
        ExportState::Idle => ExportStatusView::Idle,
        ExportState::Exporting { progress } => ExportStatusView::Exporting {
            progress: *progress,
        },
        ExportState::Complete { message } => ExportStatusView::Complete(message),
        ExportState::Error { message } => ExportStatusView::Error(message),
    }
}

fn analysis_tab_index(tab: AnalysisTab) -> usize {
    match tab {
        AnalysisTab::Frequency => 0,
        AnalysisTab::Impulse => 1,
        AnalysisTab::Phase => 2,
        AnalysisTab::Spectrum => 3,
    }
}

fn index_to_analysis_tab(index: usize) -> AnalysisTab {
    match index {
        1 => AnalysisTab::Impulse,
        2 => AnalysisTab::Phase,
        3 => AnalysisTab::Spectrum,
        _ => AnalysisTab::Frequency,
    }
}

//! Complete application-page composition over the shared application snapshot.

use crate::{
    DesignSystem, IR_COLORS,
    components::{
        AnalysisPreviewAction, AnalysisPreviewCard, AnalysisPreviewCardView, AnalysisTraceView,
        ContentStatusView, ExportMixedIrAction, ExportMixedIrCard, ExportMixedIrCardView,
        ExportStatusView, InputSourceAction, InputSourceCard, InputSourceCardView, IrRackAction,
        IrRackCard, IrRackCardView, IrRackSlotView, OutputAction, OutputCard, OutputCardView,
        SlidingPane, SourceMode as UiSourceMode, StatusBar, StatusBarView, TopBar, TopBarAction,
        TopBarView, TransportState as UiTransportState,
    },
};
use egui::{Color32, Id, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};
use egui_lucide::Lucide;
use ir_app::{
    AnalysisTab, AppCommand, AppSnapshot, Choice, ContentState, EqBandId, EqShape, EqualizerState,
    EqualizerTarget, ExportState, FrontendMode, IrId, SourceMode, TransportState, selected_id,
    selected_index,
};

#[derive(Clone, Copy, Debug, Default)]
struct EqPaneUiState {
    open: bool,
    target: Option<EqualizerTarget>,
}

const EQ_PANE_STATE_ID: &str = "application_equalizer_pane_state";

/// Opens the per-IR equalizer pane on the next application-page frame.
pub fn open_equalizer_pane(ctx: &egui::Context, ir_id: IrId) {
    ctx.data_mut(|data| {
        data.insert_temp(
            Id::new(EQ_PANE_STATE_ID),
            EqPaneUiState {
                open: true,
                target: Some(EqualizerTarget::Ir(ir_id)),
            },
        );
    });
}

/// Opens the post-mix global equalizer pane on the next application-page frame.
pub fn open_global_equalizer_pane(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.insert_temp(
            Id::new(EQ_PANE_STATE_ID),
            EqPaneUiState {
                open: true,
                target: Some(EqualizerTarget::Global),
            },
        );
    });
}

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
        let body = egui::Frame::new()
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
        commands.extend(self.show_equalizer_pane(ui, body.response.rect));
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
                            main_width,
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
                balance_percent: slot.balance_percent,
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
            balance_mode: self.snapshot.project.balance_mode,
        };
        IrRackCard::new("application_ir_rack", &view)
            .width(width)
            .rack_height(self.layout.rack_height + 2.0)
            .show(ui)
            .inner
            .into_iter()
            .filter_map(|action| match action {
                IrRackAction::EditGlobalEqualizer => {
                    open_global_equalizer_pane(ui.ctx());
                    None
                }
                IrRackAction::EditEqualizer { id } => {
                    open_equalizer_pane(ui.ctx(), IrId(id));
                    Some(AppCommand::SelectIr(IrId(id)))
                }
                action => Some(map_rack_action(action)),
            })
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

    fn show_equalizer_pane(&self, ui: &mut Ui, body_rect: Rect) -> Vec<AppCommand> {
        let ctx = ui.ctx();
        let state_id = Id::new(EQ_PANE_STATE_ID);
        let mut pane =
            ctx.data_mut(|data| data.get_temp::<EqPaneUiState>(state_id).unwrap_or_default());
        let Some(target) = pane.target else {
            pane.open = false;
            ctx.data_mut(|data| data.insert_temp(state_id, pane));
            return Vec::new();
        };
        let ds = DesignSystem::from_context(ctx);
        let global_sample_rate = self
            .snapshot
            .project
            .source
            .sample_rate
            .0
            .parse::<u32>()
            .unwrap_or(48_000);
        let (equalizer, heading, metadata, curve_color, sample_rate, waveform, pane_title) =
            match target {
                EqualizerTarget::Global => (
                    &self.snapshot.project.global_equalizer,
                    "Global Mix",
                    "Post-mix EQ before master output controls",
                    ds.colors.accent,
                    global_sample_rate,
                    self.snapshot.combined_waveform.as_slice(),
                    "Global EQ",
                ),
                EqualizerTarget::Ir(ir_id) => {
                    let Some(slot) = self
                        .snapshot
                        .project
                        .ir_slots
                        .iter()
                        .find(|slot| slot.id == ir_id)
                    else {
                        pane.open = false;
                        pane.target = None;
                        ctx.data_mut(|data| data.insert_temp(state_id, pane));
                        return Vec::new();
                    };
                    (
                        &slot.equalizer,
                        slot.filename.as_str(),
                        slot.metadata.as_str(),
                        IR_COLORS[slot.color_index as usize % IR_COLORS.len()],
                        slot.sample_rate_hz.max(1.0) as u32,
                        slot.waveform.as_slice(),
                        "Parametric EQ",
                    )
                }
            };
        let selected = self
            .snapshot
            .selected_eq
            .filter(|(selected_target, _)| *selected_target == target)
            .map(|(_, band)| band);
        let estimated_peak = match target {
            EqualizerTarget::Global => waveform_peak_db(waveform),
            EqualizerTarget::Ir(_) => estimate_eq_peak_db(waveform, equalizer, sample_rate),
        };
        let mut commands = Vec::new();
        let response = SlidingPane::new(
            "application_equalizer_pane",
            pane_title,
            Lucide::SlidersHorizontal,
            body_rect,
        )
        .width(ds.metrics.equalizer_pane_width.min(body_rect.width()))
        .show(ctx, &mut pane.open, |ui| {
            ui.spacing_mut().item_spacing.y = ds.metrics.space_md;
            ui.horizontal(|ui| {
                ui.colored_label(
                    curve_color,
                    "●",
                );
                ui.vertical(|ui| {
                    ui.strong(heading);
                    ui.small(metadata);
                });
            });

            ui.horizontal(|ui| {
                let mut bypassed = equalizer.bypassed;
                if ui.checkbox(&mut bypassed, "Bypass EQ").changed() {
                    commands.push(AppCommand::SetEqBypassed(target, bypassed));
                }
                ui.separator();
                ui.label("Output");
                let mut output_gain = equalizer.output_gain_db;
                if ui
                    .add(
                        egui::DragValue::new(&mut output_gain)
                            .range(ir_eq::MIN_GAIN_DB..=ir_eq::MAX_GAIN_DB)
                            .speed(0.1)
                            .suffix(" dB"),
                    )
                    .changed()
                {
                    commands.push(AppCommand::SetEqOutputGainDb(target, output_gain));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Reset all").clicked() {
                        commands.push(AppCommand::ResetEq(target));
                    }
                    if ui
                        .add_enabled(self.snapshot.eq_clipboard_available, egui::Button::new("Paste"))
                        .clicked()
                    {
                        commands.push(AppCommand::PasteEq(target));
                    }
                    if ui.button("Copy").clicked() {
                        commands.push(AppCommand::CopyEq(target));
                    }
                });
            });

            show_eq_graph(
                ui,
                target,
                equalizer,
                sample_rate,
                curve_color,
                selected,
                &mut commands,
            );
            ui.small("Click a node to select · Drag for frequency/gain · Shift-drag locks an axis · Wheel changes Q · Shift-wheel changes gain · Right-click for band options");
            show_selected_band(ui, target, equalizer, selected, &mut commands);

            let headroom = -estimated_peak;
            let color = if headroom < 0.0 { ds.colors.status_danger } else { ds.colors.text_secondary };
            ui.horizontal(|ui| {
                ui.label("Estimated processed peak");
                ui.colored_label(color, format!("{estimated_peak:.1} dBFS"));
                ui.separator();
                ui.label("Headroom");
                ui.colored_label(color, format!("{headroom:.1} dB"));
                if headroom < 0.0 {
                    ui.colored_label(ds.colors.status_danger, "Clipping risk");
                }
            });
        });
        if response.fully_closed {
            pane.target = None;
        }
        ctx.data_mut(|data| data.insert_temp(state_id, pane));
        commands
    }
}

fn show_eq_graph(
    ui: &mut Ui,
    target: EqualizerTarget,
    equalizer: &EqualizerState,
    sample_rate: u32,
    curve_color: Color32,
    selected: Option<EqBandId>,
    commands: &mut Vec<AppCommand>,
) {
    let ds = DesignSystem::from_context(ui.ctx());
    let desired = Vec2::new(ui.available_width(), 360.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Equalizer response graph")
    });
    let painter = ui.painter_at(rect);
    let plot_rect = Rect::from_min_max(
        rect.min + Vec2::new(54.0, 34.0),
        rect.max - Vec2::new(14.0, 28.0),
    );
    painter.rect_filled(rect, ds.metrics.radius_control, ds.colors.surface_inset);
    painter.rect_stroke(
        rect,
        ds.metrics.radius_control,
        Stroke::new(1.0, ds.colors.border_control),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.left_top() + Vec2::new(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        "EQ filter preview",
        crate::TextRole::GraphLabel.font_id(),
        curve_color,
    );

    let frequency_ticks = [
        (20.0, "20 Hz"),
        (50.0, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1_000.0, "1k"),
        (2_000.0, "2k"),
        (5_000.0, "5k"),
        (10_000.0, "10k"),
        (20_000.0, "20 kHz"),
    ];
    for (index, (frequency, label)) in frequency_ticks.iter().enumerate() {
        let x = eq_frequency_to_x(plot_rect, *frequency);
        painter.vline(
            x,
            plot_rect.y_range(),
            Stroke::new(1.0, ds.colors.border_subtle),
        );
        let alignment = if index == 0 {
            egui::Align2::LEFT_TOP
        } else if index == frequency_ticks.len() - 1 {
            egui::Align2::RIGHT_TOP
        } else {
            egui::Align2::CENTER_TOP
        };
        painter.text(
            Pos2::new(x, plot_rect.bottom() + 6.0),
            alignment,
            *label,
            crate::TextRole::GraphLabel.font_id(),
            ds.colors.text_secondary,
        );
    }
    for gain in [-12.0, -6.0, 0.0, 6.0, 12.0] {
        let y = eq_gain_to_y(plot_rect, gain);
        painter.hline(
            plot_rect.x_range(),
            y,
            Stroke::new(
                if gain == 0.0 { 1.5 } else { 1.0 },
                if gain == 0.0 {
                    ds.colors.border_strong
                } else {
                    ds.colors.border_subtle
                },
            ),
        );
        painter.text(
            Pos2::new(plot_rect.left() - 8.0, y),
            egui::Align2::RIGHT_CENTER,
            if gain > 0.0 {
                format!("+{gain:.0} dB")
            } else {
                format!("{gain:.0} dB")
            },
            crate::TextRole::GraphLabel.font_id(),
            ds.colors.text_secondary,
        );
    }

    for band in &equalizer.bands {
        if let Some(x) = eq_filter_guide_x(plot_rect, band) {
            let base = eq_band_color(band.shape);
            let guide = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 77);
            let mut y = plot_rect.top();
            while y < plot_rect.bottom() {
                painter.line_segment(
                    [
                        Pos2::new(x, y),
                        Pos2::new(x, (y + 3.0).min(plot_rect.bottom())),
                    ],
                    Stroke::new(4.0, guide),
                );
                y += 7.0;
            }
        }
    }

    let points = 256;
    let curve = (0..points)
        .map(|index| {
            let t = index as f32 / (points - 1) as f32;
            let frequency = ir_eq::MIN_FREQUENCY_HZ
                * (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).powf(t);
            Pos2::new(
                egui::lerp(plot_rect.x_range(), t),
                eq_gain_to_y(
                    plot_rect,
                    ir_eq::filter_response_db(equalizer, sample_rate, frequency),
                ),
            )
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(curve, Stroke::new(2.0, curve_color)));

    let interaction_pointer = response.interact_pointer_pos();
    let hovered_band = response
        .hover_pos()
        .and_then(|position| nearest_eq_band(plot_rect, &equalizer.bands, position));
    for (index, band) in equalizer.bands.iter().enumerate() {
        let position = eq_band_position(plot_rect, band);
        let is_selected = selected == Some(band.id);
        let type_color = eq_band_color(band.shape);
        let fill = if band.enabled {
            type_color
        } else {
            type_color.gamma_multiply(0.45)
        };
        painter.circle_filled(position, 13.0, fill);
        painter.circle_stroke(
            position,
            if is_selected { 16.0 } else { 14.0 },
            Stroke::new(
                if is_selected { 2.0 } else { 1.0 },
                if is_selected {
                    ds.colors.accent_focus
                } else {
                    ds.colors.border_strong
                },
            ),
        );
        let label_color = match band.shape {
            EqShape::LowShelf | EqShape::HighPass | EqShape::LowPass => ds.colors.surface_canvas,
            _ => ds.colors.text_on_accent,
        };
        painter.text(
            position,
            egui::Align2::CENTER_CENTER,
            eq_band_label(band.shape, index),
            crate::TextRole::GraphLabel.font_id(),
            if band.enabled {
                label_color
            } else {
                label_color.gamma_multiply(0.65)
            },
        );
    }

    if response.clicked_by(egui::PointerButton::Primary) {
        commands.push(AppCommand::SelectEqBand(target, hovered_band));
    }
    if response.dragged_by(egui::PointerButton::Primary)
        && let Some(band_id) = selected
        && let Some(band) = equalizer.bands.iter().find(|band| band.id == band_id)
    {
        let delta = ui.input(|input| input.pointer.delta());
        let shift = ui.input(|input| input.modifiers.shift);
        let horizontal = !shift || delta.x.abs() >= delta.y.abs();
        let vertical = !shift || delta.y.abs() > delta.x.abs();
        if horizontal && delta.x != 0.0 {
            let octaves = delta.x / plot_rect.width()
                * (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).log2();
            commands.push(AppCommand::SetEqBandFrequency(
                target,
                band_id,
                band.frequency_hz * 2.0_f32.powf(octaves),
            ));
        }
        if vertical && band.shape.has_gain() && delta.y != 0.0 {
            commands.push(AppCommand::SetEqBandGain(
                target,
                band_id,
                band.gain_db - delta.y / plot_rect.height() * 24.0,
            ));
        }
    }
    if let Some(band_id) = hovered_band {
        let scroll = raw_vertical_scroll_delta(ui);
        if scroll != 0.0
            && let Some(band) = equalizer.bands.iter().find(|band| band.id == band_id)
        {
            let shift = ui.input(|input| input.modifiers.shift);
            if !shift || band.shape.has_gain() {
                ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
                if shift {
                    commands.push(AppCommand::SetEqBandGain(
                        target,
                        band_id,
                        band.gain_db + scroll.signum() * 0.1,
                    ));
                } else {
                    commands.push(AppCommand::SetEqBandQ(
                        target,
                        band_id,
                        band.q * 2.0_f32.powf(scroll.signum() * 0.08),
                    ));
                }
            }
        }
    }

    let menu_position = interaction_pointer.unwrap_or(plot_rect.center());
    response.context_menu(|ui| {
        if let Some(band_id) = hovered_band {
            for shape in EqShape::ALL {
                if ui.button(shape.label()).clicked() {
                    commands.push(AppCommand::SetEqBandShape(target, band_id, shape));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Delete band").clicked() {
                commands.push(AppCommand::RemoveEqBand(target, band_id));
                ui.close();
            }
        } else if plot_rect.contains(menu_position)
            && ui
                .add_enabled(
                    equalizer.bands.len() < ir_eq::MAX_BANDS,
                    egui::Button::new("Create Bell band here"),
                )
                .clicked()
        {
            commands.push(AppCommand::AddEqBand(
                target,
                eq_x_to_frequency(plot_rect, menu_position.x),
                eq_y_to_gain(plot_rect, menu_position.y),
            ));
            ui.close();
        }
    });
}

fn show_selected_band(
    ui: &mut Ui,
    target: EqualizerTarget,
    equalizer: &EqualizerState,
    selected: Option<EqBandId>,
    commands: &mut Vec<AppCommand>,
) {
    let selected_band = selected.and_then(|id| equalizer.bands.iter().find(|band| band.id == id));
    ui.separator();
    ui.add_enabled_ui(selected_band.is_some(), |ui| {
        ui.horizontal(|ui| {
            let Some(band) = selected_band else {
                ui.label("Enabled: None");
                ui.label("Type: None");
                ui.label("Frequency: None");
                ui.label("Gain: None");
                ui.label("Q: None");
                return;
            };
            let mut enabled = band.enabled;
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                commands.push(AppCommand::SetEqBandEnabled(target, band.id, enabled));
            }
            egui::ComboBox::from_id_salt(("eq_shape", target, band.id.0))
                .selected_text(band.shape.label())
                .show_ui(ui, |ui| {
                    for shape in EqShape::ALL {
                        if ui
                            .selectable_label(shape == band.shape, shape.label())
                            .clicked()
                        {
                            commands.push(AppCommand::SetEqBandShape(target, band.id, shape));
                        }
                    }
                });
            let mut frequency = band.frequency_hz;
            if ui
                .add(
                    egui::DragValue::new(&mut frequency)
                        .range(ir_eq::MIN_FREQUENCY_HZ..=ir_eq::MAX_FREQUENCY_HZ)
                        .speed(1.0)
                        .suffix(" Hz"),
                )
                .changed()
            {
                commands.push(AppCommand::SetEqBandFrequency(target, band.id, frequency));
            }
            if band.shape.has_gain() {
                let mut gain = band.gain_db;
                if ui
                    .add(
                        egui::DragValue::new(&mut gain)
                            .range(ir_eq::MIN_GAIN_DB..=ir_eq::MAX_GAIN_DB)
                            .speed(0.1)
                            .suffix(" dB"),
                    )
                    .changed()
                {
                    commands.push(AppCommand::SetEqBandGain(target, band.id, gain));
                }
            } else {
                ui.add_enabled(false, egui::Label::new("Gain: None"));
            }
            let mut q = band.q;
            if ui
                .add(
                    egui::DragValue::new(&mut q)
                        .range(ir_eq::MIN_Q..=ir_eq::MAX_Q)
                        .speed(0.05)
                        .prefix("Q "),
                )
                .changed()
            {
                commands.push(AppCommand::SetEqBandQ(target, band.id, q));
            }
        });
    });
}

fn eq_band_color(shape: EqShape) -> Color32 {
    match shape {
        EqShape::Bell => IR_COLORS[0],
        EqShape::LowShelf => IR_COLORS[6],
        EqShape::HighShelf => IR_COLORS[1],
        EqShape::Notch => IR_COLORS[5],
        EqShape::HighPass => IR_COLORS[2],
        EqShape::LowPass => IR_COLORS[3],
    }
}

fn eq_band_label(shape: EqShape, band_index: usize) -> String {
    match shape {
        EqShape::Bell => format!("{:02}", band_index.min(99)),
        EqShape::LowShelf => "LS".to_owned(),
        EqShape::HighShelf => "HS".to_owned(),
        EqShape::Notch => "NT".to_owned(),
        EqShape::HighPass => "HP".to_owned(),
        EqShape::LowPass => "LP".to_owned(),
    }
}

fn eq_filter_guide_x(rect: Rect, band: &ir_app::EqBand) -> Option<f32> {
    matches!(band.shape, EqShape::HighPass | EqShape::LowPass)
        .then(|| eq_frequency_to_x(rect, band.frequency_hz))
}

fn raw_vertical_scroll_delta(ui: &Ui) -> f32 {
    ui.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::MouseWheel {
                    delta, modifiers, ..
                } if !modifiers.ctrl && !modifiers.command => Some(delta.y),
                _ => None,
            })
            .sum()
    })
}

fn eq_frequency_to_x(rect: Rect, frequency: f32) -> f32 {
    let t = (frequency.clamp(ir_eq::MIN_FREQUENCY_HZ, ir_eq::MAX_FREQUENCY_HZ)
        / ir_eq::MIN_FREQUENCY_HZ)
        .ln()
        / (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).ln();
    egui::lerp(rect.x_range(), t)
}

fn eq_x_to_frequency(rect: Rect, x: f32) -> f32 {
    let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    ir_eq::MIN_FREQUENCY_HZ * (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).powf(t)
}

fn eq_gain_to_y(rect: Rect, gain: f32) -> f32 {
    egui::lerp(
        rect.y_range(),
        ((12.0 - gain.clamp(-12.0, 12.0)) / 24.0).clamp(0.0, 1.0),
    )
}

fn eq_y_to_gain(rect: Rect, y: f32) -> f32 {
    12.0 - ((y - rect.top()) / rect.height()).clamp(0.0, 1.0) * 24.0
}

fn eq_band_position(rect: Rect, band: &ir_app::EqBand) -> Pos2 {
    Pos2::new(
        eq_frequency_to_x(rect, band.frequency_hz),
        eq_gain_to_y(
            rect,
            if band.shape.has_gain() {
                band.gain_db
            } else {
                0.0
            },
        ),
    )
}

fn nearest_eq_band(rect: Rect, bands: &[ir_app::EqBand], position: Pos2) -> Option<EqBandId> {
    bands
        .iter()
        .filter_map(|band| {
            let distance = eq_band_position(rect, band).distance(position);
            (distance <= 14.0).then_some((band.id, distance))
        })
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(id, _)| id)
}

fn waveform_peak_db(waveform: &[f32]) -> f32 {
    let source_peak = waveform
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if source_peak <= f32::MIN_POSITIVE {
        return -144.0;
    }

    20.0 * source_peak.log10()
}

fn estimate_eq_peak_db(waveform: &[f32], equalizer: &EqualizerState, sample_rate: u32) -> f32 {
    let source_peak_db = waveform_peak_db(waveform);
    if source_peak_db <= -144.0 {
        return source_peak_db;
    }
    let max_eq = (0..180)
        .map(|index| {
            let t = index as f32 / 179.0;
            let frequency = ir_eq::MIN_FREQUENCY_HZ
                * (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).powf(t);
            ir_eq::response_db(equalizer, sample_rate, frequency)
        })
        .fold(f32::NEG_INFINITY, f32::max);
    source_peak_db + max_eq
}

fn map_rack_action(action: IrRackAction) -> AppCommand {
    match action {
        IrRackAction::EditGlobalEqualizer => {
            AppCommand::SelectEqBand(EqualizerTarget::Global, None)
        }
        IrRackAction::AddIr => AppCommand::AddIr,
        IrRackAction::ClearAll => AppCommand::ClearAllIrs,
        IrRackAction::NormalizeAll => AppCommand::NormalizeAllIrs,
        IrRackAction::SetBalanceMode(enabled) => AppCommand::SetBalanceMode(enabled),
        IrRackAction::Select { id } => AppCommand::SelectIr(IrId(id)),
        IrRackAction::Browse { id } => AppCommand::ReplaceIr(IrId(id)),
        IrRackAction::Remove { id } => AppCommand::RemoveIr(IrId(id)),
        IrRackAction::MoveUp { id } => AppCommand::MoveIrUp(IrId(id)),
        IrRackAction::MoveDown { id } => AppCommand::MoveIrDown(IrId(id)),
        IrRackAction::SetEnabled { id, enabled } => AppCommand::SetIrEnabled(IrId(id), enabled),
        IrRackAction::SetGainDb { id, gain_db } => AppCommand::SetIrGainDb(IrId(id), gain_db),
        IrRackAction::SetBalancePercent { id, percent } => {
            AppCommand::SetIrBalancePercent(IrId(id), percent)
        }
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
        IrRackAction::EditEqualizer { id } => AppCommand::SelectIr(IrId(id)),
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

#[cfg(test)]
mod equalizer_tests {
    use super::*;

    #[test]
    fn logarithmic_frequency_mapping_round_trips() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 360.0));
        for frequency in [20.0, 80.0, 1_000.0, 8_000.0, 20_000.0] {
            let restored = eq_x_to_frequency(rect, eq_frequency_to_x(rect, frequency));
            assert!((restored / frequency - 1.0).abs() < 1.0e-5);
        }
    }

    #[test]
    fn no_gain_shapes_are_drawn_on_the_zero_db_axis() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 360.0));
        let band = ir_app::EqBand {
            id: EqBandId(1),
            enabled: true,
            shape: EqShape::Notch,
            frequency_hz: 1_000.0,
            gain_db: 12.0,
            q: 2.0,
        };
        assert_eq!(eq_band_position(rect, &band).y, eq_gain_to_y(rect, 0.0));
    }

    #[test]
    fn band_labels_use_type_abbreviations_and_full_list_position() {
        assert_eq!(eq_band_label(EqShape::Bell, 0), "00");
        assert_eq!(eq_band_label(EqShape::Bell, 7), "07");
        assert_eq!(eq_band_label(EqShape::LowShelf, 0), "LS");
        assert_eq!(eq_band_label(EqShape::HighShelf, 0), "HS");
        assert_eq!(eq_band_label(EqShape::Notch, 0), "NT");
        assert_eq!(eq_band_label(EqShape::HighPass, 0), "HP");
        assert_eq!(eq_band_label(EqShape::LowPass, 0), "LP");
    }

    #[test]
    fn every_shape_has_the_approved_type_color() {
        assert_eq!(eq_band_color(EqShape::Bell), IR_COLORS[0]);
        assert_eq!(eq_band_color(EqShape::LowShelf), IR_COLORS[6]);
        assert_eq!(eq_band_color(EqShape::HighShelf), IR_COLORS[1]);
        assert_eq!(eq_band_color(EqShape::Notch), IR_COLORS[5]);
        assert_eq!(eq_band_color(EqShape::HighPass), IR_COLORS[2]);
        assert_eq!(eq_band_color(EqShape::LowPass), IR_COLORS[3]);
    }

    #[test]
    fn only_pass_filters_receive_frequency_guides() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 360.0));
        let mut band = ir_app::EqBand {
            id: EqBandId(1),
            enabled: true,
            shape: EqShape::HighPass,
            frequency_hz: 1_000.0,
            gain_db: 0.0,
            q: 1.0,
        };
        assert_eq!(
            eq_filter_guide_x(rect, &band),
            Some(eq_frequency_to_x(rect, 1_000.0))
        );
        band.shape = EqShape::LowPass;
        assert!(eq_filter_guide_x(rect, &band).is_some());
        band.shape = EqShape::Bell;
        assert_eq!(eq_filter_guide_x(rect, &band), None);
    }
}

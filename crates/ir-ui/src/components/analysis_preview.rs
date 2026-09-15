use super::{CardFrame, ContentStatusView};
use crate::{DesignSystem, TextRole, widgets::*};
use egui::{Color32, InnerResponse, RichText, Stroke, Ui, vec2};
use egui_lucide::Lucide;
use std::cell::RefCell;

pub struct AnalysisTraceView<'a> {
    pub label: &'a str,
    pub values: &'a [f32],
    pub color: Color32,
    pub emphasized: bool,
}

pub struct AnalysisPreviewCardView<'a> {
    pub selected_tab: usize,
    pub view_modes: &'a [&'a str],
    pub view_mode: usize,
    pub smoothing_options: &'a [&'a str],
    pub smoothing: usize,
    pub frequency_traces: &'a [AnalysisTraceView<'a>],
    pub impulse_waveform: &'a [f32],
    pub impulse_color: Color32,
    pub phase_traces: &'a [AnalysisTraceView<'a>],
    pub spectrum_traces: &'a [AnalysisTraceView<'a>],
    pub sample_rate: &'a str,
    pub ir_length: &'a str,
    pub latency: &'a str,
    pub cpu: &'a str,
    pub content_status: ContentStatusView<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnalysisPreviewAction {
    SetTab(usize),
    SetViewMode(usize),
    SetSmoothing(usize),
}

pub struct AnalysisPreviewCard<'a> {
    view: &'a AnalysisPreviewCardView<'a>,
    id: egui::Id,
    width: f32,
    graph_height: f32,
}

impl<'a> AnalysisPreviewCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a AnalysisPreviewCardView<'a>) -> Self {
        Self {
            view,
            id: egui::Id::new(id),
            width: 900.0,
            graph_height: 240.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        // Graph content adapts to the card; retaining a large card-level minimum
        // would push the right rail outside medium-sized windows.
        self.width = width.max(280.0);
        self
    }

    pub fn graph_height(mut self, height: f32) -> Self {
        self.graph_height = height.max(160.0);
        self
    }

    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<AnalysisPreviewAction>> {
        ui.push_id(self.id, |ui| {
            let actions = RefCell::new(Vec::new());
            let v = self.view;
            CardFrame::new("Analysis & Preview")
                .icon(Lucide::AudioLines)
                .width(self.width)
                .show_with_header(
                    ui,
                    |ui| {
                        ui.allocate_ui_with_layout(
                            vec2(264.0, 56.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let mut view_mode = v.view_mode;
                                ui.add(
                                    DropdownSelector::new(
                                        "analysis_view_mode",
                                        v.view_modes,
                                        &mut view_mode,
                                    )
                                    .width(140.0)
                                    .label("View", SelectorLabelPosition::Top),
                                );
                                if view_mode != v.view_mode {
                                    actions
                                        .borrow_mut()
                                        .push(AnalysisPreviewAction::SetViewMode(view_mode));
                                }

                                let mut smoothing = v.smoothing;
                                ui.add(
                                    DropdownSelector::new(
                                        "analysis_smoothing",
                                        v.smoothing_options,
                                        &mut smoothing,
                                    )
                                    .width(116.0)
                                    .label("Smoothing", SelectorLabelPosition::Top),
                                );
                                if smoothing != v.smoothing {
                                    actions
                                        .borrow_mut()
                                        .push(AnalysisPreviewAction::SetSmoothing(smoothing));
                                }
                            },
                        );
                    },
                    |ui| {
                        if v.content_status != ContentStatusView::Ready {
                            super::status::status_banner(ui, v.content_status);
                            return;
                        }
                        let mut selected_tab = v.selected_tab;
                        let tabs = [
                            "Frequency Response",
                            "Impulse Response",
                            "Phase",
                            "Spectrogram",
                        ];
                        TabViewer::new(&tabs, &mut selected_tab)
                            .tab_min_width(148.0)
                            .min_content_height(self.graph_height + 16.0)
                            .show(ui, |ui, active| match active {
                                0 => graph_with_legend(ui, v.frequency_traces, self.graph_height),
                                1 => {
                                    ui.add(
                                        WaveformView::new(v.impulse_waveform, v.impulse_color)
                                            .size(vec2(ui.available_width(), self.graph_height))
                                            .label("Combined impulse response")
                                            .render_mode(WaveformRenderMode::Signed),
                                    );
                                }
                                2 => graph_with_legend(ui, v.phase_traces, self.graph_height),
                                _ => graph_with_legend(ui, v.spectrum_traces, self.graph_height),
                            });
                        if selected_tab != v.selected_tab {
                            actions
                                .borrow_mut()
                                .push(AnalysisPreviewAction::SetTab(selected_tab));
                        }

                        ui.add_space(4.0);
                        status_strip(ui, v);
                    },
                );
            actions.into_inner()
        })
    }
}

fn graph_with_legend(ui: &mut Ui, traces: &[AnalysisTraceView<'_>], height: f32) {
    let ds = DesignSystem::from_context(ui.ctx());
    let graph_curves = traces
        .iter()
        .map(|trace| (trace.values, trace.color))
        .collect::<Vec<_>>();
    ui.horizontal_top(|ui| {
        let legend_width = 116.0;
        let graph_width = (ui.available_width() - legend_width - ds.metrics.space_md).max(280.0);
        ui.add(GraphFrame::new(&graph_curves).size(vec2(graph_width, height)));
        ui.add_space(ds.metrics.space_md);
        ui.vertical(|ui| {
            ui.set_width(legend_width);
            ui.add_space(8.0);
            for trace in traces {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(34.0, 14.0), egui::Sense::hover());
                    if trace.emphasized {
                        for segment in 0..4 {
                            let left = rect.left() + segment as f32 * 9.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(left, rect.center().y),
                                    egui::pos2((left + 5.0).min(rect.right()), rect.center().y),
                                ],
                                Stroke::new(2.0, trace.color),
                            );
                        }
                    } else {
                        ui.painter().hline(
                            rect.x_range(),
                            rect.center().y,
                            Stroke::new(2.0, trace.color),
                        );
                    }
                    ui.label(
                        RichText::new(trace.label)
                            .font(TextRole::Metadata.font_id())
                            .color(ds.colors.text_secondary),
                    );
                });
            }
        });
    });
}

fn status_strip(ui: &mut Ui, view: &AnalysisPreviewCardView<'_>) {
    let ds = DesignSystem::from_context(ui.ctx());
    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Center),
        |ui| {
            for (index, (label, value)) in [
                ("Sample Rate", view.sample_rate),
                ("IR Length (Mixed)", view.ir_length),
                ("Latency (est)", view.latency),
                ("CPU", view.cpu),
            ]
            .into_iter()
            .enumerate()
            {
                if index > 0 {
                    ui.separator();
                }
                ui.label(
                    RichText::new(format!("{label}:  {value}"))
                        .font(TextRole::Metadata.font_id())
                        .color(ds.colors.text_secondary),
                );
            }
        },
    );
}

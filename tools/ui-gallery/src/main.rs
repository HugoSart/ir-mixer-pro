mod cards;
use egui::{Color32, RichText, ScrollArea, Stroke, Vec2, vec2};
use egui_lucide::Lucide;
use ir_ui::widgets::{
    ActionButton, AudioKnob, ButtonKind, ChannelToggle, Checkbox, DbValueEditor, DropdownSelector,
    GraphFrame, IconButton, LevelMeter, ListSelector, ListSelectorItem, MiniFader, PanKnob,
    SampleDelayEditor, SegmentedControl, SelectorLabelPosition, TabViewer, WaveformRenderMode,
    WaveformView, deterministic_curve, deterministic_waveform, section_header,
};
use ir_ui::{DesignSystem, IR_COLORS, TextRole};
use nice_plug_egui::{App, EguiWindow, EguiWindowSettings, Frame, baseview::dpi::LogicalSize};

fn main() {
    EguiWindow::create(
        EguiWindowSettings::new()
            .with_title("IR Mixer Pro — Component Gallery")
            .with_size(LogicalSize {
                width: 1180.0,
                height: 780.0,
            })
            .with_min_size(Some(LogicalSize {
                width: 880.0,
                height: 620.0,
            })),
        GalleryApp::default(),
    )
    .expect("component gallery window should open")
    .run_until_closed()
    .expect("component gallery should close cleanly");
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum GalleryPage {
    Cards,
    #[default]
    Foundations,
    Buttons,
    Selectors,
    Values,
    AudioVisuals,
}

struct GalleryApp {
    preview_card: cards::CardsDemo,
    live_card: cards::CardsDemo,
    ir_rack_card: cards::IrRackDemo,
    output_card: cards::OutputDemo,
    export_card: cards::ExportDemo,
    page: GalleryPage,
    source_mode: usize,
    muted: bool,
    soloed: bool,
    polarity: bool,
    normalize: bool,
    limit_output: bool,
    unlabeled_checkbox: bool,
    dropdown_plain: usize,
    dropdown_left: usize,
    dropdown_top: usize,
    list_selection: usize,
    analysis_tab: usize,
    selection_feedback: String,
    gain_db: f32,
    output_db: f32,
    delay_samples: i32,
    pan: f32,
    demo_gains: [f32; 4],
    waveforms: Vec<Vec<f32>>,
    curves: Vec<Vec<f32>>,
}

impl Default for GalleryApp {
    fn default() -> Self {
        Self {
            preview_card: cards::CardsDemo::default(),
            live_card: cards::CardsDemo::live(),
            ir_rack_card: cards::IrRackDemo::default(),
            output_card: cards::OutputDemo::default(),
            export_card: cards::ExportDemo::default(),
            page: GalleryPage::Foundations,
            source_mode: 0,
            muted: true,
            soloed: false,
            polarity: false,
            normalize: true,
            limit_output: false,
            unlabeled_checkbox: true,
            dropdown_plain: 0,
            dropdown_left: 1,
            dropdown_top: 2,
            list_selection: 0,
            analysis_tab: 0,
            selection_feedback: "Choose a preset. The third row is guarded.".into(),
            gain_db: -7.0,
            output_db: -2.0,
            delay_samples: 7,
            pan: 0.0,
            demo_gains: [-18.0, -12.0, -6.0, 0.0],
            waveforms: (0..IR_COLORS.len())
                .map(|index| deterministic_waveform(index as f32 * 0.71, 256))
                .collect(),
            curves: (0..4)
                .map(|index| deterministic_curve(index as f32 * 0.9, 180))
                .collect(),
        }
    }
}

impl App for GalleryApp {
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
        let ds = DesignSystem::default();
        egui::Panel::left("gallery_navigation")
            .exact_size(220.0)
            .frame(
                egui::Frame::new()
                    .fill(ds.colors.surface_toolbar)
                    .stroke(Stroke::new(1.0, ds.colors.border_subtle))
                    .inner_margin(16.0),
            )
            .show(ui, |ui| self.navigation(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ds.colors.surface_canvas)
                    .inner_margin(20.0),
            )
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        GalleryPage::Foundations => self.foundations(ui),
                        GalleryPage::Cards => {
                            Self::page_title(ui, "Cards", "Interactive dummy source, IR rack, output, and export controls; no real devices or files.");
                            ui.horizontal_wrapped(|ui| {
                                ui.vertical(|ui| { ui.set_width(320.0); self.preview_card.show(ui, "preview_card"); });
                                ui.add_space(16.0);
                                ui.vertical(|ui| { ui.set_width(320.0); self.live_card.show(ui, "live_card"); });
                            });
                            ui.add_space(16.0);
                            let rack_width = ui.available_width().clamp(640.0, 900.0);
                            self.ir_rack_card.show(ui, rack_width);
                            ui.add_space(16.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.vertical(|ui| { ui.set_width(320.0); self.output_card.show(ui); });
                                ui.add_space(16.0);
                                ui.vertical(|ui| { ui.set_width(320.0); self.export_card.show(ui); });
                            });
                        },
                        GalleryPage::Buttons => self.buttons(ui),
                        GalleryPage::Selectors => self.selectors(ui),
                        GalleryPage::Values => self.values(ui),
                        GalleryPage::AudioVisuals => self.audio_visuals(ui),
                    });
            });
    }
}

impl GalleryApp {
    fn navigation(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        ui.label(
            RichText::new("IR Mixer Pro")
                .font(TextRole::ProductTitle.font_id())
                .color(ds.colors.text_primary),
        );
        ui.label(
            RichText::new("Component Gallery")
                .font(TextRole::Metadata.font_id())
                .color(ds.colors.text_secondary),
        );
        ui.add_space(20.0);

        for (page, icon, label) in [
            (GalleryPage::Foundations, Lucide::Palette, "Foundations"),
            (GalleryPage::Cards, Lucide::PanelTop, "Cards"),
            (
                GalleryPage::Buttons,
                Lucide::MousePointerClick,
                "Buttons & toggles",
            ),
            (
                GalleryPage::Values,
                Lucide::SlidersHorizontal,
                "Value controls",
            ),
            (GalleryPage::Selectors, Lucide::ListFilter, "Selectors"),
            (
                GalleryPage::AudioVisuals,
                Lucide::AudioLines,
                "Audio visuals",
            ),
        ] {
            if ui
                .add(
                    ActionButton::new(label)
                        .icon(icon)
                        .selected(self.page == page)
                        .min_width(184.0),
                )
                .clicked()
            {
                self.page = page;
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.label(
                RichText::new("Design-system reference · v0.1")
                    .font(TextRole::Metadata.font_id())
                    .color(ds.colors.text_muted),
            );
        });
    }

    fn page_title(ui: &mut egui::Ui, title: &str, description: &str) {
        let ds = DesignSystem::default();
        ui.label(
            RichText::new(title)
                .font(TextRole::ProductTitle.font_id())
                .color(ds.colors.text_primary),
        );
        ui.label(
            RichText::new(description)
                .font(TextRole::Body.font_id())
                .color(ds.colors.text_secondary),
        );
        ui.add_space(16.0);
    }

    fn foundations(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        Self::page_title(
            ui,
            "Foundations",
            "Normative surfaces, typography, geometry, and channel identity colors.",
        );

        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 1, "Surfaces & application accent");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for (name, color) in [
                    ("Canvas", ds.colors.surface_canvas),
                    ("Toolbar", ds.colors.surface_toolbar),
                    ("Panel", ds.colors.surface_panel),
                    ("Inset", ds.colors.surface_inset),
                    ("Control", ds.colors.surface_control),
                    ("Hover", ds.colors.surface_hover),
                    ("Accent", ds.colors.accent),
                ] {
                    color_swatch(ui, name, color);
                }
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 2, "IR identity palette");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for (index, color) in IR_COLORS.iter().copied().enumerate() {
                    color_swatch(ui, &format!("IR {}", index + 1), color);
                }
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Categorical only: these hues do not imply warning, error, or success.",
                )
                .font(TextRole::Metadata.font_id())
                .color(ds.colors.text_secondary),
            );
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 3, "Typography");
            ui.add_space(12.0);
            for (role, sample) in [
                (TextRole::ProductTitle, "Product title — IR Mixer Pro"),
                (TextRole::SectionTitle, "Section title — Analysis & Preview"),
                (TextRole::ControlLabel, "Control label — Output Gain"),
                (TextRole::Body, "Body/value — -12.0 dB"),
                (
                    TextRole::Metadata,
                    "Metadata — 48.0 kHz · 24-bit · 2048 samples",
                ),
                (TextRole::GraphLabel, "Graph label — 20 Hz   1 kHz   20 kHz"),
            ] {
                ui.label(RichText::new(sample).font(role.font_id()));
            }
        });
    }

    fn buttons(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        Self::page_title(
            ui,
            "Buttons & toggles",
            "Interactive, selected, icon, disabled, and channel-control states.",
        );
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 1, "Actions");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    ActionButton::new("Add IR")
                        .kind(ButtonKind::Primary)
                        .icon(Lucide::Plus),
                );
                ui.add(ActionButton::new("Remove").icon(Lucide::Trash2));
                ui.add(ActionButton::new("Browse").icon(Lucide::FolderOpen));
                ui.add(
                    ActionButton::new("Settings")
                        .kind(ButtonKind::Ghost)
                        .icon(Lucide::Settings),
                );
                ui.add(
                    ActionButton::new("Disabled")
                        .icon(Lucide::Ban)
                        .enabled(false),
                );
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 2, "Selection");
            ui.add_space(12.0);
            ui.add(SegmentedControl::new(
                &["Dry", "Selected IR", "Full Mix"],
                &mut self.source_mode,
            ));
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 3, "Channel controls");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add(ChannelToggle::new(
                    "S",
                    &mut self.soloed,
                    ds.colors.accent_focus,
                    "Solo this IR",
                ));
                ui.add(ChannelToggle::new(
                    "M",
                    &mut self.muted,
                    ds.colors.accent,
                    "Mute this IR",
                ));
                ui.add(ChannelToggle::new(
                    "Ø",
                    &mut self.polarity,
                    ds.colors.text_primary,
                    "Invert polarity",
                ));
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 4, "Checkboxes");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(Checkbox::new(&mut self.normalize, "Normalize"));
                ui.add(Checkbox::new(&mut self.limit_output, "Limit output"));
                ui.add_enabled(false, Checkbox::new(&mut self.limit_output, "Disabled"));
                ui.add(
                    Checkbox::new(&mut self.unlabeled_checkbox, "Unlabeled checkbox")
                        .show_label(false),
                );
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 5, "Icon-only actions");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(IconButton::new(Lucide::FolderOpen, "Browse files"));
                ui.add(IconButton::new(Lucide::Settings, "Open settings").kind(ButtonKind::Ghost));
                ui.add(
                    IconButton::new(Lucide::Pin, "Pinned")
                        .kind(ButtonKind::Ghost)
                        .selected(true),
                );
                ui.add(
                    IconButton::new(Lucide::Trash2, "Remove")
                        .kind(ButtonKind::Ghost)
                        .enabled(false),
                );
            });
        });
    }

    fn selectors(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        Self::page_title(
            ui,
            "Selectors",
            "Dropdown label arrangements and always-visible single-selection lists.",
        );
        let devices = ["System Default", "Interface 1-2", "Loopback 1-2"];

        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 1, "Dropdown selectors");
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Without label")
                            .font(TextRole::Metadata.font_id())
                            .color(ds.colors.text_secondary),
                    );
                    ui.add(DropdownSelector::new(
                        "gallery_dropdown_plain",
                        &devices,
                        &mut self.dropdown_plain,
                    ));
                });
                ui.add_space(24.0);
                ui.add(
                    DropdownSelector::new(
                        "gallery_dropdown_left",
                        &devices,
                        &mut self.dropdown_left,
                    )
                    .label("Input", SelectorLabelPosition::Left),
                );
                ui.add_space(24.0);
                ui.add(
                    DropdownSelector::new("gallery_dropdown_top", &devices, &mut self.dropdown_top)
                        .label("Output device", SelectorLabelPosition::Top),
                );
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 2, "List selector with selection guard");
            ui.add_space(12.0);
            let items = [
                ListSelectorItem::new("01", "90-10 Close+Room"),
                ListSelectorItem::new("02", "Tight Modern"),
                ListSelectorItem::new("100", "Unsaved Experiment"),
                ListSelectorItem::new("04", "Empty Template"),
            ];
            let feedback = &mut self.selection_feedback;
            let mut guard = |_: usize, proposed: usize| {
                if proposed == 2 {
                    *feedback =
                        "Selection blocked: this is where an unsaved-changes dialog can open."
                            .into();
                    false
                } else {
                    *feedback = format!("Selection changed to {}.", items[proposed].value);
                    true
                }
            };
            ui.add(
                ListSelector::new(&items, &mut self.list_selection).on_before_change(&mut guard),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(&self.selection_feedback)
                    .font(TextRole::Metadata.font_id())
                    .color(ds.colors.text_secondary),
            );
        });
    }

    fn values(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        Self::page_title(
            ui,
            "Value controls",
            "Audio-specific adjustment, fine control, reset, and direct entry.",
        );
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 1, "Knobs");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add(
                    AudioKnob::new(&mut self.output_db, -60.0..=12.0, "Output")
                        .default_value(0.0)
                        .suffix(" dB"),
                );
                ui.add(PanKnob::new(&mut self.pan));
                for (value, color) in self.demo_gains.iter_mut().zip(IR_COLORS) {
                    ui.add(
                        AudioKnob::new(value, -60.0..=12.0, "IR Gain")
                            .default_value(0.0)
                            .suffix(" dB")
                            .accent(color),
                    );
                }
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 2, "Compact rack controls");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Level");
                    ui.add(MiniFader::new(
                        &mut self.gain_db,
                        -60.0..=12.0,
                        IR_COLORS[1],
                    ));
                });
                ui.vertical(|ui| {
                    ui.label("Gain");
                    ui.add(DbValueEditor::new(&mut self.gain_db, -60.0..=12.0));
                });
                ui.vertical(|ui| {
                    ui.label("Delay");
                    ui.add(SampleDelayEditor::new(&mut self.delay_samples, 48_000.0));
                });
            });
        });
    }

    fn audio_visuals(&mut self, ui: &mut egui::Ui) {
        let ds = DesignSystem::default();
        Self::page_title(
            ui,
            "Audio visuals",
            "Deterministic waveform, meter, graph, and non-happy-path states.",
        );
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 1, "Waveform render modes");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    WaveformView::new(&self.waveforms[0], IR_COLORS[0])
                        .size(vec2(320.0, 100.0))
                        .label("Signed")
                        .render_mode(WaveformRenderMode::Signed),
                );
                ui.add(
                    WaveformView::new(&self.waveforms[0], IR_COLORS[0])
                        .size(vec2(320.0, 100.0))
                        .label("Symmetric magnitude")
                        .render_mode(WaveformRenderMode::Symmetric),
                );
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 2, "Symmetric IR thumbnails");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for (index, (waveform, color)) in self.waveforms.iter().zip(IR_COLORS).enumerate() {
                    ui.add(
                        WaveformView::new(waveform, color)
                            .label(&format!("IR {}", index + 1))
                            .render_mode(WaveformRenderMode::Symmetric),
                    );
                }
            });
        });

        ui.add_space(12.0);
        ui.horizontal_top(|ui| {
            ds.panel_frame().show(ui, |ui| {
                section_header(ui, 3, "Meter");
                ui.add_space(12.0);
                let time = ui.input(|input| input.time) as f32;
                let levels = [-10.0 + time.sin() * 4.0, -12.0 + (time * 1.17).sin() * 5.0];
                let peaks = [-1.8, -2.5];
                ui.add(LevelMeter::new(&levels, &peaks).accent(IR_COLORS[2]));
                ui.ctx().request_repaint();
            });

            ds.panel_frame().show(ui, |ui| {
                section_header(ui, 4, "Frequency graph frame");
                ui.add_space(12.0);
                let curves = self
                    .curves
                    .iter()
                    .enumerate()
                    .map(|(index, curve)| (curve.as_slice(), IR_COLORS[index]))
                    .collect::<Vec<_>>();
                ui.add(GraphFrame::new(&curves).size(vec2(560.0, 230.0)));
            });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 5, "Analysis tab viewer");
            ui.add_space(12.0);
            let labels = [
                "Frequency Response",
                "Impulse Response",
                "Phase",
                "Spectrogram",
            ];
            let curve_refs = self
                .curves
                .iter()
                .enumerate()
                .map(|(index, curve)| (curve.as_slice(), IR_COLORS[index]))
                .collect::<Vec<_>>();
            TabViewer::new(&labels, &mut self.analysis_tab)
                .min_content_height(220.0)
                .show(ui, |ui, active| match active {
                    0 => {
                        ui.label("Frequency response (dummy)");
                        ui.add(
                            GraphFrame::new(&curve_refs).size(vec2(ui.available_width(), 180.0)),
                        );
                    }
                    1 => {
                        ui.label("Combined impulse response (dummy)");
                        ui.add(
                            WaveformView::new(&self.waveforms[0], IR_COLORS[0])
                                .size(vec2(ui.available_width(), 180.0))
                                .render_mode(WaveformRenderMode::Signed),
                        );
                    }
                    2 => {
                        ui.label("Phase comparison (dummy)");
                        ui.add(
                            GraphFrame::new(&curve_refs[1..])
                                .size(vec2(ui.available_width(), 180.0)),
                        );
                    }
                    _ => {
                        ui.label("Spectrogram preview (dummy)");
                        ui.add(
                            GraphFrame::new(&curve_refs[..1])
                                .size(vec2(ui.available_width(), 180.0)),
                        );
                    }
                });
        });

        ui.add_space(12.0);
        ds.panel_frame().show(ui, |ui| {
            section_header(ui, 6, "Empty, loading, and error");
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                state_card(
                    ui,
                    Lucide::FileMusic,
                    "Empty",
                    "Drop or browse for an IR",
                    ds.colors.text_muted,
                );
                state_card(
                    ui,
                    Lucide::LoaderCircle,
                    "Loading",
                    "Preparing waveform…",
                    ds.colors.accent_focus,
                );
                state_card(
                    ui,
                    Lucide::TriangleAlert,
                    "Could not load",
                    "Unsupported WAV encoding",
                    ds.colors.status_danger,
                );
            });
        });
    }
}

fn color_swatch(ui: &mut egui::Ui, label: &str, color: Color32) {
    let ds = DesignSystem::default();
    ui.vertical(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(96.0, 54.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, ds.metrics.radius_control, color);
        ui.painter().rect_stroke(
            rect,
            ds.metrics.radius_control,
            Stroke::new(1.0, ds.colors.border_control),
            egui::StrokeKind::Inside,
        );
        response.on_hover_text(format!(
            "#{:02X}{:02X}{:02X}",
            color.r(),
            color.g(),
            color.b()
        ));
        ui.label(
            RichText::new(label)
                .font(TextRole::Metadata.font_id())
                .color(ds.colors.text_secondary),
        );
    });
}

fn state_card(ui: &mut egui::Ui, icon: Lucide, title: &str, detail: &str, accent: Color32) {
    let ds = DesignSystem::default();
    ds.inset_frame().show(ui, |ui| {
        ui.set_min_width(220.0);
        ui.horizontal(|ui| {
            ui.add(icon.color(accent).size(20.0).stroke_width(1.8));
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(TextRole::ControlLabel.font_id())
                        .color(ds.colors.text_primary),
                );
                ui.label(
                    RichText::new(detail)
                        .font(TextRole::Metadata.font_id())
                        .color(ds.colors.text_secondary),
                );
            });
        });
    });
}

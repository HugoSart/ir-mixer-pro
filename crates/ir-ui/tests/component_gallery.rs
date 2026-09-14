use egui::{RichText, Vec2, vec2};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use egui_lucide::Lucide;
use ir_ui::widgets::{
    ActionButton, AudioKnob, ButtonKind, ChannelToggle, Checkbox, DbValueEditor, DropdownSelector,
    GraphFrame, IconButton, LevelMeter, ListSelector, ListSelectorItem, MiniFader,
    SampleDelayEditor, SegmentedControl, SelectorLabelPosition, WaveformRenderMode, WaveformView,
    deterministic_curve, deterministic_waveform, section_header,
};
use ir_ui::{DesignSystem, IR_COLORS, TextRole};

#[derive(Debug)]
struct ControlState {
    initialized: bool,
    segment: usize,
    muted: bool,
    soloed: bool,
    polarity: bool,
    gain: f32,
    knob: f32,
    delay: i32,
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            initialized: false,
            segment: 0,
            muted: true,
            soloed: false,
            polarity: false,
            gain: -7.0,
            knob: -2.0,
            delay: 7,
        }
    }
}

fn control_harness(scale: f32) -> Harness<'static, ControlState> {
    Harness::builder()
        .with_size(Vec2::new(720.0, 380.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    egui_extras::install_image_loaders(ui.ctx());
                    ir_ui::install(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }
                let ds = DesignSystem::default();
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(ds.colors.surface_canvas)
                            .inner_margin(16.0),
                    )
                    .show(ui, |ui| {
                        ds.panel_frame().show(ui, |ui| {
                            section_header(ui, 1, "Controls");
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.add(ActionButton::new("Add IR").kind(ButtonKind::Primary));
                                ui.add(ActionButton::new("Remove"));
                                ui.add(ActionButton::new("Disabled").enabled(false));
                            });
                            ui.add_space(12.0);
                            ui.add(SegmentedControl::new(
                                &["Dry", "Selected IR", "Full Mix"],
                                &mut state.segment,
                            ));
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.add(ChannelToggle::new(
                                    "S",
                                    &mut state.soloed,
                                    ds.colors.accent_focus,
                                    "Solo",
                                ));
                                ui.add(ChannelToggle::new(
                                    "M",
                                    &mut state.muted,
                                    ds.colors.accent,
                                    "Mute",
                                ));
                                ui.add(ChannelToggle::new(
                                    "Ø",
                                    &mut state.polarity,
                                    ds.colors.text_primary,
                                    "Polarity",
                                ));
                                ui.add(MiniFader::new(&mut state.gain, -60.0..=12.0, IR_COLORS[1]));
                                ui.add(DbValueEditor::new(&mut state.gain, -60.0..=12.0));
                                ui.add(SampleDelayEditor::new(&mut state.delay, 48_000.0));
                                ui.add(
                                    AudioKnob::new(&mut state.knob, -60.0..=12.0, "Output")
                                        .suffix(" dB"),
                                );
                            });
                        });
                    });
            },
            ControlState::default(),
        )
}

#[test]
fn controls_are_interactive() {
    let mut harness = control_harness(1.0);
    harness.get_by_label("M").click();
    harness.get_by_label("Selected IR").click();
    harness.run();

    assert!(!harness.state().muted);
    assert_eq!(harness.state().segment, 1);
}

#[test]
fn controls_snapshot_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for (scale, name) in [(1.0, "controls_1x"), (2.0, "controls_2x")] {
        let mut harness = control_harness(scale);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

fn audio_visual_harness(scale: f32) -> Harness<'static> {
    let waveforms = (0..IR_COLORS.len())
        .map(|index| deterministic_waveform(index as f32 * 0.71, 192))
        .collect::<Vec<_>>();
    let curves = (0..4)
        .map(|index| deterministic_curve(index as f32 * 0.9, 120))
        .collect::<Vec<_>>();

    let mut initialized = false;
    Harness::builder()
        .with_size(Vec2::new(760.0, 520.0))
        .with_pixels_per_point(scale)
        .build_ui(move |ui| {
            if !initialized {
                egui_extras::install_image_loaders(ui.ctx());
                ir_ui::install(ui.ctx());
                initialized = true;
                ui.ctx().request_repaint();
                return;
            }
            let ds = DesignSystem::default();
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(ds.colors.surface_canvas)
                        .inner_margin(16.0),
                )
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("IR identity & audio visuals")
                            .font(TextRole::SectionTitle.font_id()),
                    );
                    ui.add_space(12.0);
                    ui.horizontal_wrapped(|ui| {
                        for (label, mode) in [
                            ("Signed", WaveformRenderMode::Signed),
                            ("Symmetric", WaveformRenderMode::Symmetric),
                        ] {
                            ui.add(
                                WaveformView::new(&waveforms[0], IR_COLORS[0])
                                    .size(vec2(340.0, 82.0))
                                    .label(label)
                                    .render_mode(mode),
                            );
                        }
                    });
                    ui.add_space(12.0);
                    ui.horizontal_wrapped(|ui| {
                        for (waveform, color) in waveforms.iter().zip(IR_COLORS).take(4) {
                            ui.add(
                                WaveformView::new(waveform, color)
                                    .size(vec2(164.0, 58.0))
                                    .render_mode(WaveformRenderMode::Symmetric),
                            );
                        }
                    });
                    ui.add_space(12.0);
                    ui.horizontal_top(|ui| {
                        ui.add(
                            LevelMeter::new(&[-10.0, -12.0], &[-1.8, -2.5])
                                .size(vec2(54.0, 210.0))
                                .accent(IR_COLORS[2]),
                        );
                        let curve_refs = curves
                            .iter()
                            .enumerate()
                            .map(|(index, curve)| (curve.as_slice(), IR_COLORS[index]))
                            .collect::<Vec<_>>();
                        ui.add(GraphFrame::new(&curve_refs).size(vec2(620.0, 210.0)));
                    });
                });
        })
}

#[test]
fn palette_and_audio_visuals_snapshot() {
    let mut results = SnapshotResults::new();
    for (scale, name) in [(1.0, "audio_visuals_1x"), (2.0, "audio_visuals_2x")] {
        let mut harness = audio_visual_harness(scale);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[derive(Debug)]
struct SelectionState {
    initialized: bool,
    checked: bool,
    unlabeled_checked: bool,
    dropdown: usize,
    list: usize,
    guard_calls: usize,
    icon_clicks: usize,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            initialized: false,
            checked: true,
            unlabeled_checked: true,
            dropdown: 0,
            list: 0,
            guard_calls: 0,
            icon_clicks: 0,
        }
    }
}

fn selection_harness(scale: f32) -> Harness<'static, SelectionState> {
    Harness::builder()
        .with_size(Vec2::new(720.0, 500.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    egui_extras::install_image_loaders(ui.ctx());
                    ir_ui::install(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }
                let ds = DesignSystem::default();
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(ds.colors.surface_canvas)
                            .inner_margin(16.0),
                    )
                    .show(ui, |ui| {
                        ds.panel_frame().show(ui, |ui| {
                            section_header(ui, 1, "Selection controls");
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.add(Checkbox::new(&mut state.checked, "Normalize"));
                                ui.add(
                                    Checkbox::new(
                                        &mut state.unlabeled_checked,
                                        "Unlabeled checkbox",
                                    )
                                    .show_label(false),
                                );
                                if ui
                                    .add(IconButton::new(Lucide::Settings, "Open settings"))
                                    .clicked()
                                {
                                    state.icon_clicks += 1;
                                }
                                ui.add(
                                    IconButton::new(Lucide::Pin, "Pinned")
                                        .kind(ButtonKind::Ghost)
                                        .selected(true),
                                );
                            });
                            ui.add_space(12.0);
                            let devices = ["System Default", "Interface 1-2", "Loopback 1-2"];
                            ui.add(
                                DropdownSelector::new(
                                    "test_dropdown",
                                    &devices,
                                    &mut state.dropdown,
                                )
                                .label("Input", SelectorLabelPosition::Left),
                            );
                            ui.add_space(12.0);
                            let items = [
                                ListSelectorItem::new("1", "Studio Default"),
                                ListSelectorItem::new("02", "Wide Double"),
                                ListSelectorItem::new("100", "Unsaved Experiment"),
                            ];
                            let calls = &mut state.guard_calls;
                            let mut guard = |_: usize, proposed: usize| {
                                *calls += 1;
                                proposed != 2
                            };
                            ui.add(
                                ListSelector::new(&items, &mut state.list)
                                    .on_before_change(&mut guard),
                            );
                        });
                    });
            },
            SelectionState::default(),
        )
}

#[test]
fn selection_controls_are_interactive_and_guarded() {
    let mut harness = selection_harness(1.0);
    harness.get_by_label("Normalize").click();
    harness.get_by_label("Unlabeled checkbox").click();
    harness.get_by_label("Open settings").click();
    harness.get_by_label("02: Wide Double").click();
    harness.run();
    assert!(!harness.state().checked);
    assert!(!harness.state().unlabeled_checked);
    assert_eq!(harness.state().icon_clicks, 1);
    assert_eq!(harness.state().list, 1);
    assert_eq!(harness.state().guard_calls, 1);

    harness.get_by_label("100: Unsaved Experiment").click();
    harness.run();
    assert_eq!(harness.state().list, 1);
    assert_eq!(harness.state().guard_calls, 2);

    harness.get_by_value("System Default").click();
    harness.run();
    harness.get_by_label("Interface 1-2").click();
    harness.run();
    assert_eq!(harness.state().dropdown, 1);

    harness.get_by_label("02: Wide Double").focus();
    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().list, 0);
    assert_eq!(harness.state().guard_calls, 3);
}

#[test]
fn selection_controls_snapshot_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for (scale, name) in [
        (1.0, "selection_controls_1x"),
        (2.0, "selection_controls_2x"),
    ] {
        let mut harness = selection_harness(scale);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

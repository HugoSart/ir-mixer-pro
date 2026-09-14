use egui::{RichText, Vec2, vec2};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_ui::widgets::{
    ActionButton, AudioKnob, ButtonKind, ChannelToggle, DbValueEditor, GraphFrame, LevelMeter,
    MiniFader, SampleDelayEditor, SegmentedControl, WaveformView, deterministic_curve,
    deterministic_waveform, section_header,
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
                                &["Preview File", "Live Input"],
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
    harness.get_by_label("Live Input").click();
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

#[test]
fn palette_and_audio_visuals_snapshot() {
    let waveforms = (0..IR_COLORS.len())
        .map(|index| deterministic_waveform(index as f32 * 0.71, 192))
        .collect::<Vec<_>>();
    let curves = (0..4)
        .map(|index| deterministic_curve(index as f32 * 0.9, 120))
        .collect::<Vec<_>>();

    let mut initialized = false;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(760.0, 520.0))
        .build_ui(move |ui| {
            if !initialized {
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
                        for (waveform, color) in waveforms.iter().zip(IR_COLORS) {
                            ui.add(WaveformView::new(waveform, color).size(vec2(164.0, 58.0)));
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
        });

    harness.snapshot("audio_visuals_1x");
}

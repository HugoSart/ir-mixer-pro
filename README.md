# IR Mixer Pro

IR Mixer Pro is a Rust application for loading, blending, monitoring, analyzing,
and exporting guitar cabinet impulse responses. The product targets a shared
standalone, VST3, and CLAP workflow built on one application model, DSP engine,
and UI.

The Windows standalone host is the current executable implementation. It
supports preview-file playback, live audio input, partitioned convolution,
multi-IR mixing, analysis, presets, and mixed-IR WAV export. VST3 and CLAP host
adapters remain part of the first-release roadmap and are not shipped by the
current build.

![Current IR Mixer Pro interface](docs/screenshots/app.png)

## Quick start

Run the native standalone application:

```powershell
cargo run --release
```

Release mode is strongly recommended for audio testing. Debug builds may not
process small device buffers quickly enough and can sound glitchy even when the
DSP behavior is otherwise correct.

Run the deterministic mock backend without opening audio devices or files:

```powershell
$env:IR_MIXER_MOCK = "1"
cargo run
```

Run the component gallery:

```powershell
cargo run -p ir-ui-gallery
```

See the [development guide](docs/development.md) for toolchain setup, checks,
snapshots, and common diagnostics.

## Documentation

- [Product specification](docs/product-spec.md) — normative multi-format product requirements
- [Architecture](docs/architecture.md) — current workspace, data flow, threading, and extension points
- [Audio processing](docs/audio-processing.md) — sample rates, IR preparation, monitoring, and export fidelity
- [Development](docs/development.md) — running, testing, visual regression, and troubleshooting
- [Build and release](docs/build-and-release.md) — packaging, signing, and release gates
- [Roadmap](docs/roadmap.md) — remaining first-release and post-release work
- [UI design](design/ui-design.md) — implemented application layout and interaction contract
- [UI design system](design/ui-design-system.md) — visual tokens and reusable component rules

## Repository layout

```text
src/                 Native standalone launcher
crates/ir-app/       Serializable state, commands, backend contract, and mock backend
crates/ir-core/      WAV I/O, audio buffers, resampling, analysis, and offline export
crates/ir-dsp/       Partitioned convolution and real-time mix engine
crates/ir-native/    CPAL host, workers, dialogs, presets, and native backend
crates/ir-ui/        Theme, widgets, cards, application page, and visual tests
tools/ui-gallery/    Interactive component and card gallery
tools/               Release packaging scripts
docs/                Product, engineering, and release documentation
design/              Current UI contract, visual tokens, artwork, and historical mockups
samples/             Development preview and IR WAV files
```

## Product targets

- Windows standalone application
- VST3 plugin
- CLAP plugin

The standalone application owns device routing. Plugin builds will receive
audio configuration from the host and reuse the shared state, DSP, UI, preset,
analysis, and export layers.

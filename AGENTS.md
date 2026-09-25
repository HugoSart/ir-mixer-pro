# Project: IR Mixer

IR Mixer is a standalone desktop application and audio plugin for loading, blending, previewing, analyzing, and exporting multiple guitar cabinet impulse responses (IRs).

## Primary Stack

- Rust
- nice-plug
- egui via nice-plug-egui
- VST3
- CLAP
- Standalone desktop build
- Cargo workspace

Do not introduce React, WebView, JUCE, or C++ in v1 unless a concrete blocker makes them necessary.

## Product Goals

The product should let a guitarist or audio engineer:

- Load N cabinet IRs simultaneously.
- Blend them in real time.
- Adjust each IR independently.
- Preview the mix using a dry guitar DI WAV file.
- Monitor a live audio-interface input through the active IR mix in standalone mode.
- Route standalone output to a selected audio device.
- Analyze waveform, frequency response, phase, and level information.
- Save and recall mix presets.
- Export the final combined IR as a standard WAV file for hardware modelers such as the Hotone Ampero 2.

## Per-IR Controls

Each IR slot should support:

- Gain / level
- Mute
- Solo
- Delay in samples and milliseconds
- Polarity invert
- Pan when stereo output is enabled
- Enable / disable
- Remove
- Reorder
- File name and metadata display

The UI must support an arbitrary number of IR slots. The implementation may impose a practical active-IR limit for real-time performance, but this must not be hard-coded into the product model without a documented reason.

## DSP Rules

- Use partitioned FFT convolution for real-time processing.
- Each active IR should have its own convolver in the real-time path.
- Gain, mute, solo, pan, and similar controls must update without rebuilding the entire mixed IR.
- Delay and polarity changes must be handled safely and predictably.
- Do not allocate memory, block, lock a mutex, access the filesystem, or perform other unbounded work on the audio thread.
- Use lock-free, atomic, or message-passing techniques where real-time state transfer is required.
- Parameter smoothing should be used where abrupt changes could create clicks.
- Exported mixed IRs should mathematically reproduce the configured mix as closely as possible.
- Internal processing should use floating point with sufficient headroom.
- Avoid accidental normalization that changes the user's intended relative gain unless explicitly requested.

## Standalone Requirements

Standalone mode should support:

- Preview WAV / DI file playback
- Live audio-interface input
- Input device selection
- Input channel selection
- Output device selection
- Sample-rate selection where supported
- Buffer-size selection where supported
- Input monitoring
- Loop playback
- Play / pause / stop
- Dry / IR / mixed comparison modes

Audio routing controls belong to standalone mode. Plugin builds should use the host's audio I/O and buffer configuration.

## Plugin Requirements

Initial plugin formats:

- VST3
- CLAP

The plugin should:

- Receive audio from the DAW host.
- Process it through the same shared IR engine as the standalone application.
- Expose automatable parameters where practical.
- Persist plugin state reliably inside host sessions.
- Keep mixed-IR export available from the plugin UI.

Future formats such as AU or AAX are out of scope for v1.

## UI Direction

The approved visual direction is a polished dark professional audio-tool interface.

Use custom egui components rather than accepting stock egui appearance.

Visual language:

- Dark charcoal / near-black background
- Layered raised panels
- Teal / cyan primary accent
- Purple secondary accent
- Clear white / light-gray typography
- Compact, dense desktop layout
- Rounded corners, restrained glow, clean borders
- Professional DAW / plugin aesthetic

Key UI areas:

- Top bar / project / preset area
- Source and transport section
- Scrollable N-IR rack
- Per-IR controls
- Mix / output section
- Waveform view
- Frequency-response graph
- Phase graph
- Real-time spectrum / level meters
- Preset browser
- Export controls
- Status bar

The UI should visually resemble the approved mockups from the design phase rather than generic egui defaults.

## Development Strategy

Build the UI first using a mocked application / audio layer.

Keep the UI independent from the final DSP implementation through clear interfaces.

Suggested conceptual boundary:

```text
UI / egui
   ↓
Application state / commands
   ↓
AudioBackend trait
   ├── MockAudioBackend
   └── NativeAudioBackend
          ↓
      IR / DSP engine
```

The mock backend should provide realistic fake state for:

- Loaded IRs
- Meters
- Spectrum data
- Waveforms
- Transport
- Device lists
- Presets
- Export progress / status

Do not tightly couple UI widgets to nice-plug process callbacks.

## Architecture Principles

- One shared DSP engine for standalone and plugin builds.
- One shared UI component library where possible.
- Keep host-specific code thin.
- Keep filesystem and device I/O outside the real-time processing layer.
- Prefer small modules with explicit responsibilities.
- Make state serializable from the start.
- Presets and plugin state should use versioned schemas.
- Prefer deterministic DSP behavior.
- Write unit tests for pure DSP and state transformations.
- Add integration tests around preset serialization and mixed-IR export.

## First Implementation Milestone

Do not begin with convolution DSP.

First milestone:

1. Scaffold Cargo workspace.
2. Add nice-plug and egui dependencies.
3. Create the shared application-state model.
4. Create `AudioBackend` abstraction.
5. Implement `MockAudioBackend`.
6. Recreate the approved desktop UI in egui.
7. Make add/remove/reorder/mute/solo/gain/delay/polarity controls fully interactive against mock state.
8. Add mocked waveform, spectrum, frequency, phase, and meter views.
9. Add responsive resizing and scrolling.
10. Only after the UI architecture is stable, begin native DSP work.

## Coding Style

- Prefer idiomatic Rust.
- Keep real-time DSP code isolated and obvious.
- Avoid `unwrap()` in runtime paths unless an invariant is proven and documented.
- Prefer explicit error propagation.
- Use descriptive types for units where confusion is possible, especially dB, samples, milliseconds, sample rate, and channel indices.
- Document real-time safety assumptions next to code that depends on them.

## Codex Guidance

Before making architectural changes:

1. Read this file.
2. Read all Markdown files under `docs/` and `design/`.
3. Preserve the core product decisions unless the requested task explicitly changes them.
4. If implementation details differ from the documented architecture, update the docs in the same change.

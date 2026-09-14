# IR Mixer — Architecture

## 1. Architectural Goals

The implementation should support three frontends from one product model:

```text
                  Shared Rust Core
                        │
        ┌───────────────┼───────────────┐
        │               │               │
   Standalone          VST3            CLAP
```

The UI and DSP engine should be reusable across plugin and standalone builds.

Host-specific concerns should remain at the edges.

## 2. Proposed Workspace

```text
ir-mixer/
├── Cargo.toml
├── AGENTS.md
├── docs/
│   ├── PRODUCT_SPEC.md
│   ├── ARCHITECTURE.md
│   └── UI_DESIGN.md
│
├── crates/
│   ├── ir-core/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ir.rs
│   │   │   ├── mix.rs
│   │   │   ├── transform.rs
│   │   │   ├── export.rs
│   │   │   └── analysis.rs
│   │   └── Cargo.toml
│   │
│   ├── ir-dsp/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── engine.rs
│   │   │   ├── convolver.rs
│   │   │   ├── realtime.rs
│   │   │   └── meters.rs
│   │   └── Cargo.toml
│   │
│   ├── ir-app/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── state.rs
│   │   │   ├── commands.rs
│   │   │   ├── backend.rs
│   │   │   ├── mock_backend.rs
│   │   │   └── preset.rs
│   │   └── Cargo.toml
│   │
│   ├── ir-ui/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── theme.rs
│   │   │   ├── app.rs
│   │   │   ├── components/
│   │   │   ├── graphs/
│   │   │   └── widgets/
│   │   └── Cargo.toml
│   │
│   └── ir-plugin/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── plugin.rs
│       │   ├── params.rs
│       │   └── editor.rs
│       └── Cargo.toml
│
└── xtask/
    └── ...
```

This layout is a starting point, not a rigid requirement. Prefer fewer crates if splitting them produces unnecessary friction.

## 3. Core Data Model

A pure application model should exist independently of the plugin framework.

Example conceptual model:

```rust
pub struct ProjectState {
    pub ir_slots: Vec<IrSlotState>,
    pub selected_ir: Option<IrId>,
    pub output_gain_db: f32,
    pub source: SourceState,
    pub transport: TransportState,
}

pub struct IrSlotState {
    pub id: IrId,
    pub name: String,
    pub path: Option<PathBuf>,
    pub gain_db: f32,
    pub delay_samples: i32,
    pub polarity_inverted: bool,
    pub muted: bool,
    pub soloed: bool,
    pub enabled: bool,
    pub pan: f32,
}
```

The DSP layer should receive compact real-time-safe representations rather than reading UI state directly.

## 4. UI-to-Backend Boundary

The UI uses controlled display data and typed UI actions.
`ir_ui::components::{InputSourceCard, IrRackCard, AnalysisPreviewCard, OutputCard, ExportMixedIrCard}`
return typed action values to their caller. Gallery adapters apply those to local
dummy state. `ir_ui::app::AppPage` adapts `ir_app::AppSnapshot` into those
views and translates actions into stable-ID `AppCommand` values. Cards do not
own devices, open files, start exports, or call the backend. Shared `CardFrame`
chrome remains independent of semantic card content.

The first implementation should use a backend abstraction.

Conceptually:

```rust
pub trait AudioBackend {
    fn snapshot(&self) -> &AppSnapshot;
    fn dispatch(&mut self, command: AppCommand);
    fn update(&mut self, now_seconds: f64);
    fn drain_events(&mut self) -> Vec<BackendEvent>;
}
```

The current mock backend implements this API directly. A native backend may use
channels internally while preserving this UI-facing contract.

Backends:

```text
AudioBackend
├── MockAudioBackend
└── NativeAudioBackend
```

### Mock backend

Used during UI-first development.

Responsibilities:

- Simulate IR loading
- Simulate meters
- Simulate spectrum data
- Simulate waveform data
- Simulate transport
- Simulate device lists
- Simulate export status
- Produce deterministic demo data where useful

### Native backend

Responsible for communicating with:

- DSP engine
- Standalone audio devices
- File decoding
- Preset storage
- Export engine

Plugin builds may use a specialized adapter around the same application services.

### Current UI-first milestone

- `ir-app` owns serializable project state, versioned preset documents,
  application commands, transient snapshots, and `MockAudioBackend`.
- `ir-ui` owns reusable widgets/cards and the responsive, mode-aware
  `AppPage`.
- The root binary hosts `AppPage` with the mock backend.
- `FrontendMode::Standalone` exposes device routing.
  `FrontendMode::Plugin` replaces it with host-routing status.
- Native audio, filesystem dialogs, export encoding, and DSP are the next
  implementation phase.

## 5. DSP Architecture

### High-level path

```text
Input
 │
 ├──► Convolver 1 ─► gain/delay/pan ─┐
 ├──► Convolver 2 ─► gain/delay/pan ─┤
 ├──► Convolver 3 ─► gain/delay/pan ─┤
 │                ...                ├──► Sum ─► Output gain ─► Output
 └──► Convolver N ─► gain/delay/pan ─┘
```

### Convolution

Use partitioned FFT convolution rather than whole-buffer offline convolution in the live path.

The implementation should be benchmarked before choosing exact partition sizes.

Potential strategy:

- Small head partition for low latency.
- Larger tail partitions for efficiency.

### Parameter updates

Avoid reconstructing convolvers for ordinary mixer changes.

Fast controls:

- Gain
- Mute
- Solo
- Pan

These should operate on convolver output.

IR-changing controls:

- Loading a new file
- Resampling
- Major trim changes

These should be prepared off the audio thread and atomically swapped into the active engine.

### Delay

Two possible implementations:

1. Delay each convolved output before summing.
2. Shift the IR during preparation.

For live tweaking, output delay may provide easier parameter updates.

For export, delay is applied directly to the impulse data.

### Polarity

Can be represented as a gain multiplier of `+1` or `-1`.

## 6. Real-Time Safety

The audio callback must not:

- Allocate
- Deallocate
- Lock a blocking mutex
- Read files
- Write files
- Log synchronously
- Perform UI work
- Trigger expensive graph recomputation

Recommended communication patterns:

- Atomics for simple scalar parameters
- Lock-free SPSC queues for events
- Double buffering / atomic pointer swaps for prepared DSP state
- Preallocated buffers

All code touching the process callback should be reviewed with real-time constraints in mind.

## 7. File Loading Pipeline

```text
User selects IR
      ↓
Background file read
      ↓
Decode WAV
      ↓
Validate
      ↓
Resample if needed
      ↓
Convert to internal float format
      ↓
Prepare convolver partitions
      ↓
Atomic engine swap
```

The UI should show loading / error state per slot.

## 8. Preview-File Pipeline

Standalone preview playback:

```text
WAV decoder
    ↓
resampler if required
    ↓
transport / loop
    ↓
shared IR DSP engine
    ↓
selected audio output
```

The preview transport should not be implemented inside the core convolution engine.

## 9. Live Input Pipeline

Standalone:

```text
Selected device input
        ↓
channel mapping
        ↓
shared IR DSP engine
        ↓
selected output channels
```

Device ownership belongs to the standalone host layer.

## 10. Plugin Pipeline

```text
DAW audio buffer
      ↓
Plugin process callback
      ↓
Shared IR DSP engine
      ↓
DAW output buffer
```

Host sample rate and block size are supplied through nice-plug lifecycle callbacks.

Plugin state should serialize project parameters and IR references.

## 11. Analysis Architecture

Do not run FFT visualization work directly in the audio callback.

### Real-time spectrum

- Copy or downsample recent audio into a ring buffer.
- Analysis worker performs FFT.
- UI consumes the latest spectrum snapshot.

### IR frequency response

This can be computed when an IR or IR parameter affecting the response changes.

No need to recompute every UI frame.

### Waveform

Generate display-friendly downsampled waveform data after loading the IR.

### Phase

Compute cached phase data for selected / combined IRs.

## 12. Export Architecture

Export does not need to run under real-time constraints.

Conceptual flow:

```text
Take project snapshot
      ↓
Load / access source IR buffers
      ↓
Apply delay
      ↓
Apply polarity
      ↓
Apply gain
      ↓
Pad lengths
      ↓
Sum
      ↓
Optional normalization
      ↓
Resample to requested rate
      ↓
Encode WAV
```

Use a background task and surface progress / errors to the UI.

## 13. Preset / State Format

Use a versioned serializable schema.

Example:

```json
{
  "schema_version": 1,
  "name": "JCM800 Live",
  "irs": [
    {
      "path": "York/MRSH/Mix01.wav",
      "gain_db": -1.5,
      "delay_samples": 0,
      "polarity_inverted": false,
      "mute": false,
      "solo": false,
      "pan": 0.0
    },
    {
      "path": "York/MRSH/Room.wav",
      "gain_db": -18.0,
      "delay_samples": 7,
      "polarity_inverted": false,
      "mute": false,
      "solo": false,
      "pan": 0.0
    }
  ],
  "output_gain_db": -2.0
}
```

Do not rely only on absolute paths long term. Future versions may add search roots, hashes, or embedded references to relocate missing files.

## 14. Threading Model

Potential threads / execution contexts:

- Audio thread
- UI thread
- File / preparation worker
- Analysis worker
- Export worker

Do not create background threads casually. Prefer a controlled worker pool or clearly owned workers.

## 15. Testing

### Unit tests

Test:

- dB conversion
- gain mixing
- polarity
- delay application
- solo / mute logic
- export summation
- preset migrations

### Golden tests

For deterministic DSP transforms, compare generated output against known-good vectors.

### Integration tests

Test:

- Load multiple IRs
- Save / reload preset
- Missing file behavior
- Export mixed WAV
- Plugin state round trip

### Benchmarks

Benchmark:

- Number of active IRs
- IR lengths
- 44.1 / 48 / 96 kHz
- Buffer sizes 64 / 128 / 256
- CPU cost
- Convolver rebuild time

## 16. Build and Packaging

Use Cargo workspace tooling.

Consider an `xtask` crate for:

- Building plugin bundles
- Copying VST3 artifacts
- Packaging standalone binaries
- Running validation
- Creating release archives

Keep release build steps documented and reproducible.

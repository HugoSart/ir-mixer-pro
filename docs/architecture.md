# IR Mixer Pro — Architecture

This document describes the current implementation and the boundaries that the
planned VST3 and CLAP adapters must preserve. Product requirements belong in
[product-spec.md](product-spec.md); detailed sample-rate and export behavior
belongs in [audio-processing.md](audio-processing.md). The implemented per-IR
and global equalizers are specified in [equalizer.md](equalizer.md).

## System shape

```text
                         AppSnapshot / AppCommand
                                  │
                         Shared egui AppPage
                                  │
                    ┌─────────────┴─────────────┐
                    │                           │
          NativeAudioBackend            future host adapter
                    │                    (VST3 or CLAP)
          CPAL + workers + dialogs              │
                    └─────────────┬─────────────┘
                                  │
                           shared DSP/core
```

The UI renders immutable display state and returns typed commands. It does not
open files, enumerate devices, own audio streams, or perform DSP. Backends own
those side effects and publish the next snapshot.

## Workspace

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Windows standalone window and application loop |
| `crates/ir-app` | Serializable state, commands, backend trait, events, preset migrations, and mock backend |
| `crates/ir-core` | Planar floating-point buffers, WAV I/O, resampling, analysis, and offline mix rendering |
| `crates/ir-dsp` | Uniform partitioned FFT convolution and the allocation-free real-time mix engine |
| `crates/ir-eq` | Serializable EQ model, `biquad` coefficient design, response evaluation, and shared filtering |
| `crates/ir-native` | CPAL devices and streams, background work, dialogs, presets, and `NativeAudioBackend` |
| `crates/ir-ui` | Design system, reusable widgets/cards, responsive `AppPage`, native window integration, and visual tests |
| `tools/ui-gallery` | Interactive isolated widget and card reference |
| `tools/build-release.ps1` | Current Windows standalone validation and ZIP packaging |

There is no plugin crate or host adapter yet. Those are first-release roadmap
work, not hidden behind a Cargo feature in the current executable.

## Application boundary

`ir-app` defines the stable UI-facing service boundary:

```rust
pub trait AudioBackend: Send {
    fn snapshot(&self) -> &AppSnapshot;
    fn dispatch(&mut self, command: AppCommand);
    fn update(&mut self, now_seconds: f64);
    fn drain_events(&mut self) -> Vec<BackendEvent>;
}
```

`AppSnapshot` contains project state plus transient device choices, meters,
graphs, loading state, and status text. `AppCommand` uses stable IDs rather than
widget indices for project mutations.

Two implementations currently exist:

- `MockAudioBackend` supplies deterministic state for UI development.
- `NativeAudioBackend` connects the same model to files, CPAL, DSP, presets,
  analysis, and export.

The root executable selects the native backend by default and the mock backend
when `IR_MIXER_MOCK` is set.

## UI composition

`ir_ui::app::AppPage` adapts `AppSnapshot` into borrowed card views and maps
returned card actions to `AppCommand`. The reusable cards are:

- `InputSourceCard`
- `IrRackCard`
- `AnalysisPreviewCard`
- `OutputCard`
- `ExportMixedIrCard`

Cards remain controlled presentation components. Their caller owns selections,
transport, project state, and side effects. `FrontendMode::Standalone` exposes
device routing; `FrontendMode::Plugin` replaces those controls with host-routing
status, but no plugin host currently instantiates that mode.

## Native threading and ownership

The standalone implementation has four important execution contexts:

1. **UI thread** — renders snapshots, dispatches commands, and polls results.
2. **Audio-host thread** — owns CPAL streams and runs stream callbacks because
   CPAL stream handles are thread-affine.
3. **Background worker** — decodes and prepares audio, renders analysis and
   exports, and performs preset file I/O.
4. **Dialog thread** — runs native file dialogs outside the egui callback to
   avoid nested-window-loop re-entry.

Channels between these contexts are bounded. The UI/application object holds
Send-safe handles and owned snapshots rather than stream objects.

## Real-time engine

Each enabled IR has its own `PreparedSlot` and pair of partitioned convolvers.
The engine processes fixed blocks and then applies per-slot delay, smoothed
gain, polarity, pan, mute/solo audibility, and summation. Output gain, bypass,
The global EQ is applied after the sum and before output gain, bypass, and the
optional limiter.

```text
source L/R
   ├── convolver 1 ─ delay/gain/pan ─┐
   ├── convolver 2 ─ delay/gain/pan ─┤
   └── convolver N ─ delay/gain/pan ─┤
                                      ├─ sum ─ global EQ ─ output gain ─ bypass ─ limiter
dry source ───────────────────────────┘
```

The output adapter bridges arbitrary device callback sizes to fixed DSP blocks
using preallocated buffers. Current selectable DSP blocks are 64, 128, and 256
frames. The real-time engine reserves capacity for 16 active IRs.

The audio callback does not allocate, deallocate, touch the filesystem, perform
UI work, or lock a blocking mutex. Prepared objects transfer into the callback
through bounded `rtrb` queues. Replaced objects return through a retirement queue
and are dropped on the audio-host thread.

## IR and preview lifecycle

An IR load follows this path:

```text
native dialog
    → background WAV decode and validation
    → immutable native-rate AudioBuffer
    → direct resample to current engine rate
    → partition preparation
    → generation check
    → real-time slot installation
```

The native-rate buffer remains the source of truth for metadata, fingerprints,
analysis/export inputs, and later engine rebuilds. The prepared engine copy is
disposable. Output-device changes increment an engine generation; work prepared
for an older rate cannot enter the current stream.

Preview WAVs follow the same native-source/prepared-copy rule. Preview mode does
not open an input stream. Live mode opens the selected input only while input
monitoring is enabled.

## Parameter updates

Ordinary mix changes do not rebuild all convolvers:

- Gain, balance, mute, solo, pan, polarity, bypass, and output gain update
  real-time parameters.
- Delay uses preallocated delay lines.
- File replacement, output-rate changes, and other IR-data changes are prepared
  off-thread and swapped in.
- Audible discontinuities are smoothed or crossfaded at block/sample level.

Balance Mode is implemented in the application model. Percentages are kept at
100% and synchronized to dB gains before parameters reach the engine.

## Per-IR and global equalizers

Equalization is implemented as a first-release feature. Each IR slot owns a
non-destructive ordered dynamic band list, complete-EQ
bypass, and EQ output gain. The project owns another equalizer after the rack
sum. The monitoring path is:

```text
source
   → per-IR convolver
   → ordered EQ-band cascade
   → EQ output gain
   → delay / polarity / rack gain / pan
   → sum → global EQ → output gain → bypass → limiter
```

Filter coefficients and replacement chains are prepared and validated away
from the callback, then transferred through the existing real-time-safe control
boundary. Parameter and topology changes must be smoothed or crossfaded without
rebuilding convolution state. Stereo channels share controls but retain
independent filter history.

Both targets use the existing right-edge `SlidingPane` over the application
body, leaving persistent application bars visible. It will request
`min(920 points, available body width)` rather than the component's default
width. It will remain a controlled UI surface driven by `AppSnapshot` and typed
`AppCommand` values. Its deterministic component-gallery state will exercise
pane lifecycle, graph gestures and context menus, and selected-band
synchronization without performing DSP or file I/O. See
[equalizer.md](equalizer.md) for the full product, UI, state, and processing
contract.

## Analysis

Analysis never runs on the audio callback. The native backend keeps one analysis
job in flight and at most one pending latest-state request. Rapid edits replace
the pending request instead of growing the worker queue.

Offline analysis uses the same active-source filtering and transforms as export,
rendered at the monitoring engine rate without final normalization. Results
replace frequency, phase, and combined-waveform snapshots only after completion.
The previous graph stays visible in the meantime.

The real-time spectrum is derived from recent output data sent through a bounded
channel. UI refresh rate is independent of the audio callback rate.

## Export

Export takes a snapshot of native-rate source buffers and slot parameters, then
runs entirely on the background worker:

```text
active native-rate sources
    → resample each source directly to export rate
    → optional per-IR normalization
    → per-IR EQ cascade and EQ output gain
    → delay and polarity
    → gain and constant-power pan
    → sum
    → global EQ cascade and EQ output gain
    → output gain
    → optional trim/pad
    → mono downmix or stereo output
    → optional -1 dBFS final normalization
    → WAV encoding
```

Mono conversion is `0.5 × (left + right)`. Integer PCM encoding uses
deterministic triangular dither and rejects samples above full scale when final
normalization is disabled. Float32 retains headroom. Monitoring bypass and the
safety limiter are not exported.

## Preset storage and migration

Standalone presets are JSON files under:

```text
%APPDATA%\IR Mixer Pro\Presets
```

The current preset schema is version 5:

- Version 1 stored basic project and path state.
- Version 2 added `IrFileReference` with original path, optional preset-relative
  path, file size, and decoded-audio fingerprint.
- Version 3 added project Balance Mode and per-slot balance percentages.
- Version 4 added per-IR EQ bypass, output gain, and stable-ID dynamic bands.
- Version 5 added the project-level global EQ.

Versions 1 and 2 migrate in memory. Unsupported future versions fail explicitly.
Loading first tries an existing stored path, then a preset-relative reference,
then the original path. An unresolved reference remains an errored slot so its
mix settings survive.

Device, channel, monitoring, runtime loading state, waveforms, export destination,
and dirty state are transient and excluded from portable preset serialization.

Versions 1–3 migrate to empty transparent per-IR and global EQ chains. Version 4
preserves per-IR EQ and adds a transparent global EQ. Missing-file slots retain
their EQ state.

## Planned plugin adapters

VST3 and CLAP adapters must:

- Translate host lifecycle, sample rate, and block-size changes into shared
  engine preparation.
- Feed host buffers into the shared DSP engine without using CPAL.
- Instantiate `AppPage` in plugin mode and omit standalone routing controls.
- Serialize compatible project/preset state into host sessions.
- Keep file dialogs, analysis, and export off the host audio callback.
- Remain thin enough that standalone and plugin processing cannot diverge.

Plugin crate layout, parameter exposure, and bundle tooling will be chosen when
that roadmap milestone begins.

## Testing

- `ir-app` tests balance behavior and preset schema migration.
- `ir-core` tests resampling, analysis, export order, channel conversion, WAV
  encodings, and clipping behavior.
- `ir-dsp` compares partitioned convolution with direct convolution and asserts
  that warmed-up processing performs no heap operations.
- `ir-native` tests loading, export, device topology, rate changes, worker
  generations, preview transport, and analysis scheduling.
- `ir-ui` uses interaction and image-snapshot tests across cards, layouts,
  widgets, and DPI scales.
- `ir-ui-gallery` tests its independent dummy card state.

See [development.md](development.md) for commands and snapshot workflow.

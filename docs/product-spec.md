# IR Mixer Pro — Product Specification

This document defines the intended product contract. It is normative rather
than a statement that every requirement is already implemented. Delivery order
and remaining work are tracked in the [roadmap](roadmap.md).

## Product goal

IR Mixer Pro lets guitarists and audio engineers build a cabinet sound from
multiple impulse responses without constructing parallel DAW tracks. A user can
load IRs, adjust each contribution, hear the result from a dry recording or
live input, inspect the mix, save it, and export one reusable WAV IR.

The same project model and audio engine target three first-release formats:

- Windows standalone application
- VST3 plugin
- CLAP plugin

## Core workflows

### Build an IR mix

- Load any practical number of mono or stereo WAV IRs.
- Enable up to 16 IRs simultaneously in the real-time engine.
- Adjust gain or percentage balance, positive delay, polarity, pan,
  normalization, mute, solo, enabled state, and order per IR.
- Shape each IR with a non-destructive parametric equalizer.
- Replace or remove an IR without disturbing unrelated slots.
- Preserve each IR's identity color when slots move or other slots are removed.

The linear mix is conceptually:

```text
output = Σ gain_i × convolve(input, transformed_ir_i)
```

Mute, solo, enable, normalization, polarity, delay, and pan determine which
sources contribute and how they are transformed.

### Preview with a recording

- Load a mono or stereo WAV file, normally a dry guitar DI.
- Play, pause, stop, return to the beginning, and loop.
- Apply input gain and optional source normalization.
- Hear mixer changes without rebuilding the complete mixed IR.

### Monitor live input

- Select an input device and channel.
- Select an output device and mono or stereo output pair.
- Select a supported buffer size and enable monitoring.
- Preserve the current IR mix when switching between preview and live modes.

Windows shared-mode monitoring follows the selected output endpoint's default
sample rate. Live monitoring requires the selected input and output default
rates to match. Detailed behavior is specified in
[audio-processing.md](audio-processing.md).

### Work inside a host

VST3 and CLAP builds must accept audio, sample rate, block size, and routing
from the plugin host. They must hide standalone device controls while retaining
the IR rack, analysis, presets, state persistence, and export.

### Export a mixed IR

- Export mono or stereo WAV at 44.1, 48, or 96 kHz.
- Export 16-bit PCM, 24-bit PCM, or 32-bit float.
- Trim or pad to 1024, 2048, or 4096 frames, or retain the natural mixed length.
- Apply final normalization only when explicitly enabled.
- Produce a result that mathematically represents the configured linear mix as
  closely as possible.

## IR slot model

Every slot stores:

- Stable ID and palette index
- Filename, path, and relocatable file reference metadata
- Native sample-rate and length metadata
- Enabled, mute, and solo state
- Gain and balance percentage
- Delay in samples
- Polarity and per-IR normalization
- Stereo pan
- Per-IR equalizer bypass, output gain, and ordered dynamic band list
- Load/error state and display waveform

The project model may contain more than 16 slots. The 16-IR limit applies only
to simultaneously enabled real-time convolvers.

### Per-IR equalizer

Every IR supports a non-destructive parametric equalizer with an ordered dynamic
band list. Bands support bell, low-shelf, high-shelf, notch, high-pass, and
low-pass shapes. Users can add, remove, duplicate, reorder, enable/bypass, and
reset bands; bypass the complete EQ; adjust explicit EQ output gain; and copy an
EQ chain between IRs.

The equalizer opens in a right-edge sliding pane with an interactive response
graph, band controls, original/EQ comparison, and peak/headroom feedback. The
pane overlays the application body while leaving persistent application bars
visible. A new or migrated IR has an empty transparent EQ chain and unity EQ
output gain. EQ never normalizes or compensates gain implicitly.

Equalizer state must affect preview, live monitoring, analysis, presets, plugin
session state, and mixed-IR export consistently. Detailed behavior is specified
in [equalizer.md](equalizer.md).

### Balance Mode

Balance Mode expresses the active mix as unity-sum amplitude percentages:

- All loaded slots total exactly 100%.
- One IR is fixed at 100%.
- Enabling the mode derives percentages from the current linear gains.
- Editing one slot proportionally redistributes the remaining percentage.
- Adding or removing an IR equalizes the remaining slots.
- Percentages are converted to equivalent linear gains for monitoring,
  analysis, presets, and export.

### Delay and polarity

- Delay is non-negative and limited to 4096 engine samples.
- The UI shows samples and the equivalent milliseconds at the active engine rate.
- Polarity is a `+1` or `-1` multiplier.
- Negative delay and automatic phase alignment are post-release features.

### Solo, mute, and bypass

- Multiple IRs may be soloed.
- When any enabled slot is soloed, non-soloed slots do not contribute.
- Muted or disabled slots do not contribute.
- Output bypass crossfades to the dry source rather than changing stored mix
  parameters.

## Analysis

The application provides:

- Combined and per-IR frequency response
- Combined impulse response
- Combined and per-IR phase response
- A real-time output spectrum trace
- Stereo output level and peak meters
- Engine sample rate, mixed length, estimated latency, CPU load, active-IR
  count, and engine status

Mix-affecting changes schedule background analysis. Existing graphs remain
visible until the newest calculation completes. Per-IR EQ and EQ output gain are
included in frequency, phase, and combined impulse analysis.

## Presets and state

- Presets use a versioned JSON schema.
- Presets retain IR references, slot order and controls, per-IR equalizer state,
  Balance Mode, output controls, analysis selections, and export choices.
- Device and channel selections are machine-local and are not portable preset
  state.
- Missing IR files remain visible as errored slots so mixer settings are not
  silently discarded.
- Plugin builds must persist equivalent state inside host sessions.

Preset schema and migration behavior are documented in
[architecture.md](architecture.md#preset-storage-and-migration).

## Monitoring and output safety

- Standalone monitoring supports selectable output gain and bypass.
- The optional stereo-linked safety limiter prevents output clipping at a
  -0.3 dBFS ceiling with 1 ms lookahead and 50 ms release.
- Limiter delay is included in the displayed latency.
- The monitoring limiter and bypass are never rendered into exported IR data.
- Empty or inaudible racks pass the source through in the monitor path.

## File compatibility

Initial import support is WAV only:

- Mono or stereo
- 8-, 16-, 24-, or 32-bit integer PCM
- 32-bit floating point

Files with more than two channels or unsupported encodings produce an explicit
error. AIFF, FLAC, IR folder import, and library management are post-release
possibilities.

## Platform responsibilities

### Standalone

The application owns audio-device discovery, channel selection, buffer size,
preview transport, monitoring, and native file dialogs.

### VST3 and CLAP

The host owns audio I/O, sample rate, block size, routing, and editor window
lifecycle. Host adapters must remain thin and must not duplicate the shared mix
or export logic.

## Quality requirements

- Do not allocate, deallocate, block, lock a mutex, access files, or perform
  unbounded work on the audio callback.
- Smooth or crossfade audible real-time parameter changes.
- Keep file decoding, resampling, FFT preparation, analysis, preset I/O, and
  export off the callback.
- Preserve native-rate decoded IRs so monitoring-rate changes never become the
  source for later export resampling.
- Do not normalize implicitly.
- Reject integer PCM export that would clip when final normalization is off;
  32-bit float export may retain headroom above 0 dBFS.
- Remain responsive at supported desktop window sizes and DPI scales.

## First-release exclusions

- Amp, distortion, drive, or NAM modeling
- General-purpose reverb design controls
- AU and AAX
- Mobile builds
- Cloud sync, marketplace, or IR store
- Automatic phase alignment or minimum-phase conversion
- IR library tagging and search

An IR may still contain a captured room or reverb tail; that is ordinary linear
convolution material rather than a separate reverb feature.

# IR Mixer — Product Specification

## 1. Product Summary

IR Mixer is a desktop application and audio plugin for creating guitar cabinet impulse-response blends.

Its central purpose is to make multi-IR experimentation faster than using several DAW tracks or multiple convolution plugins. Users can load multiple IRs, adjust them independently, hear the result in real time, and export the result as one combined IR for use in hardware modelers or other IR loaders.

The first release targets:

- Standalone desktop use
- VST3
- CLAP

Primary implementation stack:

- Rust
- nice-plug
- egui

## 2. Core Use Cases

### 2.1 Build a cabinet blend

A user loads multiple cabinet IRs, for example:

- Close SM57 IR
- Ribbon IR
- Room IR
- Rear-cab IR

The user adjusts level, delay, polarity, and pan until the blend sounds right.

### 2.2 Preview without plugging in a guitar

The user selects a dry DI guitar recording and loops it while adjusting the mix.

The preview must react to parameter changes in real time.

### 2.3 Use live guitar input

In standalone mode the user selects an audio-interface input and hears it through the current IR mix using a selected output device.

This is intended for interactive tone design without requiring a DAW.

### 2.4 Use inside a DAW

The user inserts the VST3 or CLAP plugin on a guitar track and adjusts the same IR mix while playing or during playback.

The DAW supplies audio input, output, sample rate, transport context, and buffer size.

### 2.5 Export a combined IR

After building the desired blend, the user exports a single WAV impulse response that approximates the active mix.

Typical destination devices include guitar modelers such as Hotone Ampero 2.

## 3. N-Way IR Mixing

The product must support more than two IRs.

Conceptually, for N IRs:

```text
output = Σ gain_i × convolve(input, IR_i)
```

For exported IRs:

```text
mixed_ir = Σ gain_i × transformed(IR_i)
```

where `transformed()` incorporates supported per-IR operations such as delay and polarity.

The UI should not be designed around exactly two IRs.

## 4. IR Slot Model

Each IR slot contains at minimum:

- Unique ID
- File reference
- Display name
- Sample rate
- Channel count
- IR length
- Gain
- Delay samples
- Polarity state
- Mute state
- Solo state
- Enabled state
- Pan
- Order index

Optional future metadata:

- Manufacturer / pack
- Cabinet
- Speaker
- Microphone
- Mic position
- User notes
- Tags

## 5. Per-IR Controls

### Gain

- Display in dB.
- Should allow precise numeric entry.
- Suggested initial range: approximately -60 dB to +12 dB.
- Fader / knob scaling should emphasize useful mixing ranges around 0 to -24 dB.

### Delay

- Adjustable in samples.
- Also display an equivalent millisecond value.
- Allow positive delay in v1.
- Negative relative alignment may be supported later through reference-slot logic or pre-roll padding.

### Polarity

- Normal
- Inverted

### Mute / Solo

- Multiple soloed channels should be permitted.
- If one or more slots are soloed, non-soloed channels are excluded.

### Pan

- Relevant for stereo monitoring / plugin output.
- Export behavior must be explicit because many hardware cabinet IR formats are mono.

### Reordering

Users should be able to reorder IR slots by drag-and-drop or explicit move controls.

Order should not change the mathematical result when all processing is linear, but it affects usability and preset organization.

## 6. Source Modes

### Preview File

Supported initial source:

- WAV

Preferred input material:

- Dry / DI guitar

Controls:

- Browse / load
- Play
- Pause
- Stop
- Seek
- Loop
- Timeline / waveform

Potential later formats:

- AIFF
- FLAC

### Live Input

Standalone-only controls:

- Input device
- Input channel
- Output device
- Output channel configuration
- Sample rate
- Buffer size
- Monitor enable

The user should be able to switch between preview file and live input without losing the IR mix.

## 7. Monitoring Modes

Useful comparison modes:

- Dry
- Selected IR only
- Full mix
- Bypass

Potential later modes:

- Snapshot morphing

## 8. Analysis

### IR waveform

Show the impulse waveform for the selected IR or combined IR.

### Frequency response

Show magnitude response for:

- Selected IR
- Combined mix

Optional overlays may be added later.

### Phase

Show phase response or a simplified phase/alignment visualization.

### Spectrum

For live / preview audio, show a real-time output spectrum.

### Level meters

At minimum:

- Input level
- Output level

Optional later:

- Per-IR post-convolution levels

## 9. Presets

Preset selection and save actions live in the compact top bar under the
**Presets** label. There is no separate preset card.

A preset should store:

- IR slot list
- IR file references
- Slot order
- Gain
- Delay
- Polarity
- Mute / solo state
- Pan
- Output gain
- Relevant analysis / UI state where appropriate

Device selections should generally not be part of portable plugin presets unless explicitly configured as application preferences.

Preset state must be versioned.

## 10. Export

Export mixed IR as WAV.

Initial options:

- Sample rate
- Bit depth
- Mono / stereo where supported
- Output length
- Normalize toggle
- Optional peak target when normalization is enabled

Suggested hardware-friendly default:

- 48 kHz
- 24-bit PCM

Export must not silently normalize unless the user has enabled it.

If IRs use different sample rates, export processing should resample explicitly using a high-quality resampler.

## 11. Standalone vs Plugin Behavior

### Standalone

Contains:

- Audio-device selection
- Live input controls
- Preview-file transport
- Buffer settings
- Output routing

### VST3 / CLAP

Does not expose device selection or host buffer settings.

The host controls audio I/O.

The plugin retains:

- N-IR rack
- Analysis
- Mix presets
- Export mixed IR
- Plugin state persistence

## 12. Non-Goals for v1

Not required in the first release:

- Full amp modeling
- NAM model loading
- Distortion / drive modeling
- General-purpose reverb convolution
- AU
- AAX
- Mobile builds
- Cloud preset sync
- Marketplace / IR store

The architecture should not prevent future expansion into some of these areas.

## 13. Performance Goals

The product should feel suitable for live guitar monitoring on a typical modern desktop machine.

Goals:

- Stable operation at common 48 kHz configurations.
- Practical operation at 64–128 sample host / device buffers where the hardware permits it.
- Parameter changes should feel immediate.
- Avoid audio-thread allocations and locks.
- UI load should not impact audio stability.

No hard maximum active-IR count should be promised before benchmarking.

The UI may display CPU / load warnings if the active configuration becomes too expensive.

## 14. Error Handling

The product should gracefully handle:

- Missing preset IR files
- Unsupported WAV encodings
- Sample-rate mismatches
- Stereo / mono mismatches
- Device disconnects
- Audio-device errors
- Export destination errors
- Invalid or corrupted preset files

The UI should identify the affected IR slot rather than failing the whole project when possible.

## 15. Future Ideas

Potential post-v1 capabilities:

- Automatic phase alignment
- Correlation meter
- Delay estimation
- IR trim / crop editor
- Minimum-phase conversion
- Room-IR blending helpers
- IR tagging and browser
- Drag-and-drop entire folders
- Searchable IR library
- Snapshots
- Preset morphing
- Built-in test signals
- Batch export
- AU / AAX support
- Optional amp / NAM stage before the IR engine

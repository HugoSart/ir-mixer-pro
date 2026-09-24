# IR Mixer — UI Design Specification

> The normative colors, typography, spacing, geometry, and interaction tokens are
> defined in [UI_DESIGN_SYSTEM.md](UI_DESIGN_SYSTEM.md). This document defines
> product layout and behavior; the design-system document defines visual treatment.

## 1. Design Goal

The UI should reproduce the approved concept: a polished, modern, dark desktop audio application that feels closer to a professional DAW tool than a generic Rust desktop program.

The interface must not look like stock egui.

Create reusable custom controls and a dedicated theme system.

## 2. Visual Language

### Background

Use layered dark values rather than one flat black surface.

Suggested hierarchy:

- App background: very dark charcoal
- Main panels: slightly lighter charcoal
- Raised cards: another small step lighter
- Hover states: subtle lightening
- Active selections: dark surface with teal / cyan border or fill accent

### Accents

Primary:

- Cyan / teal

Secondary:

- Purple / violet

Warnings:

- Amber

Errors / clipping:

- Red

Success / active engine:

- Green or cyan depending on context

Avoid excessive saturation. The design should feel premium, not arcade-like.

### Typography

Use a clean sans-serif font with strong legibility at compact sizes.

Hierarchy:

- App / project title
- Section title
- Primary control label
- Secondary metadata
- Numeric value
- Status text

Numeric controls should use consistent width and alignment.

## 3. Main Window Structure

Approved large-screen structure:

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ Top Bar: Logo / Project / Preset / Save / Settings / CPU                │
├───────────────┬──────────────────────────────────────┬────────────────────┤
│ Source        │ N-IR Mixer Rack                     │ Output / Export     │
│               │                                      │                    │
│ Preview/Live  │ IR Slot 1                            │ Master meter       │
│ transport     │ IR Slot 2                            │ Output gain        │
│ device input  │ IR Slot 3                            │ Export Controls    │
│ waveform      │ ...                                  │ Export             │
│               │ + Add IR                             │                    │
├───────────────┴──────────────────────────────────────┴────────────────────┤
│ Analysis: Frequency / Phase / Spectrum / Combined Waveform              │
├───────────────────────────────────────────────────────────────────────────┤
│ Status Bar: Sample Rate / Buffer / CPU / Active IRs / Engine State      │
└───────────────────────────────────────────────────────────────────────────┘
```

The approved v4 layout has no top navigation tabs or separate preset card. At
wide widths, Input Source sits left of the IR rack, Output and Export form the
right rail, and Analysis & Preview spans the input and rack columns. The exact
panel proportions adapt to window width.

## 4. Top Bar

Contents:

- Product logo / name
- Current project or preset name
- Mix preset dropdown
- Save preset
- Undo / redo later
- Settings button
- Optional CPU indicator
- Standalone-only minimize, maximize / restore, and close controls

The top bar should remain compact.

The standalone build uses borderless native chrome. Empty top-bar space is the
window drag region, and invisible six-pixel edge handles provide native side and
corner resizing with the platform resize cursors. Plugin editors omit these
controls and continue to use their host window.

## 5. Source Panel

### Source selector

Use segmented controls or tabs:

```text
[ Preview File ] [ Live Input ]
```

### Preview File mode

Show:

- Loaded filename
- Browse button
- Waveform / timeline
- Play / pause
- Stop
- Loop
- Time position

The Browse control replaces its folder icon with an animated spinner and is
disabled from file-picker launch through decode completion. Existing filename,
metadata, and waveform content remain visible during replacement; load failures
continue to use inline error feedback.
While preview playback is active, the transport control shows Pause and pauses
the current position when activated. A just-issued Play command remains in the
playing state until the audio thread acknowledges it, avoiding a transient reset
back to the Play icon.

### Live Input mode

Show:

- Input device
- Input channel
- Output device
- Buffer size
- Sample rate if configurable
- Monitoring toggle

Plugin builds should hide device-specific controls cleanly rather than leaving disabled empty widgets.

## 6. IR Rack

This is the visual center of the application.

It must support N rows and vertical scrolling.

Each row should feel like a compact channel strip / rack module.

Suggested row layout:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ ≡  01  York MRSH Mix 01.wav                     [S] [M] [Ø] [×]   │
│                                                                      │
│ waveform thumbnail   Gain      Delay      Pan        Level meter     │
│ ────────────────      -3.0 dB   0 samp     C          █████░░        │
│                                                                      │
│ metadata: 48 kHz • mono • 1024 samples                              │
└──────────────────────────────────────────────────────────────────────┘
```

Controls:

- Drag handle
- Slot number
- File name
- File browse / replace action
- Solo
- Mute
- Polarity
- Remove
- Gain
- Delay
- Pan
- Small waveform
- Optional per-slot meter

Use tooltips for compact icon controls.

## 7. Add IR Interaction

At bottom of rack:

```text
[ + Add IR ]
```

The rack header also places a **Balance Mode** checkbox immediately to the left
of Add IR. When enabled, the Level column becomes Balance and shows percentage
knobs. Percentages sum to 100% across all loaded IRs; the sole knob for one IR
is disabled at 100%.

Support later:

- Drag-and-drop WAV files
- Multiple-file drop
- Folder drop

When new IRs are added, preserve existing scroll position sensibly.

While an Add IR batch is being prepared, the Add IR button shows an in-button
spinner and cannot be triggered again. A per-slot replacement uses the same
treatment in that row's action control. Do not replace file metadata with
temporary loading text; retain existing metadata or leave it blank for a new
slot until decoding completes.

## 8. Gain Control

IR rack levels use a compact rotary control. Its numeric dB value sits above the
dial, matching the compact Pan control's geometry. The level knob uses neutral
control colors; IR identity colors remain on waveforms and analysis traces.

Requirements:

- Drag interaction
- Fine adjustment modifier
- Double-click reset
- Numeric direct entry
- dB suffix

When Balance Mode is enabled, this same control uses a `%` suffix and a 0–100
range. Changes redistribute the other IRs immediately to retain a 100% total.

## 9. Delay Control

Display both:

```text
7 samples
0.146 ms
```

Primary editing can be sample-based.

Support:

- Drag adjustment
- Arrow keys
- Direct numeric input
- Reset to zero

## 10. Polarity

Use a recognizable Ø-style control.

States must be visually unmistakable.

Do not rely only on subtle color changes.

## 11. Mute / Solo

Use conventional audio semantics.

Suggested:

- `M` amber / highlighted when active
- `S` cyan or green when active

Solo logic should reflect multiple simultaneous solo states.

## 12. Output Panel

Contains:

- Large stereo or mono output meter
- Output gain
- Clip indicator
- Bypass
- Optional limiter / safety behavior only if explicitly added later

Do not silently process output beyond the configured mix.

## 13. Preset Controls

Preset controls live only in the compact top bar under **Presets**:

- Preset dropdown / searchable list
- Previous / next
- Save
- Save As
- Delete
- Favorite later

Display unsaved changes with a subtle dirty-state indicator.

## 14. Export Panel

Primary button:

```text
[ Export Mixed IR ]
```

Export dialog options:

- Destination
- Filename
- Sample rate
- Bit depth
- Mono / stereo
- Length / trim
- Normalize toggle

The dialog should display a concise summary of the result before export.

## 15. Analysis Area

Use tabs or a multi-panel arrangement depending on available width.

Suggested tabs:

```text
[ Frequency ] [ Phase ] [ Spectrum ] [ Waveform ]
```

### Frequency Response

- Logarithmic frequency axis
- dB vertical axis
- Combined response emphasized
- Optional selected-IR overlay
- Hover readout

### Phase

- Frequency vs phase
- Clear zero line
- Avoid overcomplicated presentation initially

### Spectrum

- Real-time FFT of preview / live output
- Smooth enough to be readable
- UI updates decoupled from the audio thread

### Waveform

- Combined impulse waveform
- Selected individual IR overlay or switch
- Zoom later

Frequency, phase, and combined impulse views update from the latest completed
background analysis whenever a mix-affecting IR control changes. Keep the
current graph visible while a newer result is calculated; never replace the
chart with a transient loading banner.

## 16. Meters

Meters should be custom-painted rather than generic progress bars.

Requirements:

- Fast attack
- Slower decay
- Peak hold
- Clip marker
- dB scale where space permits

The visual refresh rate does not need to match the audio callback rate.

## 17. egui Component Library

Create reusable custom widgets such as:

```text
IrPanel
IrSlotCard
AudioKnob
MiniFader
DbValueEditor
SampleDelayEditor
PolarityButton
MuteButton
SoloButton
LevelMeter
WaveformView
SpectrumView
FrequencyResponseView
PhaseView
SegmentedControl
SectionHeader
DeviceSelector
TransportBar
```

Centralize colors, spacing, radii, typography, and interaction constants.

Suggested module structure:

```text
ir-ui/src/
├── theme.rs
├── app.rs
├── layout.rs
├── widgets/
│   ├── knob.rs
│   ├── fader.rs
│   ├── meter.rs
│   ├── buttons.rs
│   └── value_editor.rs
├── components/
│   ├── top_bar.rs
│   ├── source_panel.rs
│   ├── ir_rack.rs
│   ├── ir_slot.rs
│   ├── output_panel.rs
│   ├── top_bar.rs
│   └── export_panel.rs
└── graphs/
    ├── waveform.rs
    ├── spectrum.rs
    ├── frequency.rs
    └── phase.rs
```

## 18. Mock-First UI Development

Before DSP exists, the UI should run with realistic mock data.

Default demo state may include:

```text
IR 1: York MRSH Mix 01.wav
IR 2: York MRSH 121.wav
IR 3: York MRSH Room.wav
IR 4: York Greenback 57.wav
```

Example levels:

```text
Mix 01      -1.5 dB
121         -7.0 dB
Room       -18.0 dB
Greenback  -12.0 dB
```

Mock behavior should include:

- Animated meters
- Fake live spectrum
- Deterministic frequency graph
- Functional transport controls
- IR reorder
- Add / remove
- Mute / solo
- Delay / polarity
- Preset switching
- Export dialog

This allows the product experience to be validated before audio-engine complexity is introduced.

## 19. Responsive Behavior

The target is primarily desktop, but window resizing should be graceful.

Large width:

- Source left
- IR rack center
- Output / Export right

Medium width:

- Source and output panels may narrow
- Analysis may move to tabs

Small supported width:

- Right-side panels may stack below the rack

Do not attempt phone-sized responsive design.

## 20. Interaction Details

Support common desktop conventions:

- Mouse wheel scrolling
- Ctrl/Cmd + click where useful
- Shift for fine adjustment
- Double-click reset
- Right-click context menus
- Keyboard navigation where practical
- Drag-and-drop reorder

Future keyboard shortcuts may include:

- Space: preview play / pause
- M: mute selected IR
- S: solo selected IR
- Delete: remove selected IR
- Ctrl/Cmd+S: save preset

## 21. Visual Quality Bar

Before considering the UI complete:

- No default egui-looking widgets should remain in the main workflow unless intentionally styled.
- Spacing should be consistent.
- Numeric baselines should align.
- Hover / active / disabled states should be designed.
- Graphs should remain readable on high-DPI displays.
- Empty states should look intentional.
- Loading and error states must be designed, not just logged.
- The UI should look credible beside modern commercial guitar plugins.

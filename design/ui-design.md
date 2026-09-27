# IR Mixer Pro — UI Design

This document describes the implemented application layout and interaction
contract. Visual tokens and reusable component rules are defined in
[ui-design-system.md](ui-design-system.md).

The current composed application screenshot is
[`docs/screenshots/app.png`](../docs/screenshots/app.png). Reviewed snapshots
under `crates/ir-ui/tests/snapshots` are the regression reference for supported
layouts and scales. Images under `design/mockups/` preserve the earlier design
direction but do not override current implementation behavior.

## Design goal

IR Mixer Pro uses a dense, professional dark audio-tool interface rather than
stock egui styling. The UI prioritizes fast scanning of many IRs, visible audio
state, and clear separation between source, mix, output, analysis, and export.

The visual language uses:

- Near-black and dark-charcoal layered surfaces
- Electric blue for primary interaction
- Stable per-IR identity colors for waveforms and graph traces
- Light neutral typography with subdued metadata
- Compact controls, restrained rounding, and clean one-pixel borders
- Explicit loading, disabled, error, clipping, and engine states

## Window and responsive layout

The standalone window opens at 1536 × 1024 logical pixels and supports a minimum
size of 880 × 680. It uses borderless Windows chrome with custom minimize,
maximize/restore, and close controls. Empty top-bar space is draggable and six-
pixel edge regions provide native resize behavior.

The application page has three responsive arrangements:

### Wide — 1320 points and above

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Top bar                                                            │
├──────────────┬───────────────────────────────┬──────────────────────┤
│ Input Source │ IR rack                       │ Output               │
│              │                               ├──────────────────────┤
│              │                               │ Export Mixed IR      │
├──────────────┴───────────────────────────────┤                      │
│ Analysis & Preview                           │                      │
├──────────────────────────────────────────────┴──────────────────────┤
│ Status bar                                                          │
└─────────────────────────────────────────────────────────────────────┘
```

Input Source is 280 points wide, the right rail is 300 points, and the IR rack
uses the remaining central width. Analysis spans the source and rack columns.

### Medium — 980 to 1319 points

Input Source, Output, and Export stack in a 300-point side column. The rack and
analysis remain in the main column.

### Narrow — below 980 points

Cards stack vertically. The rack and analysis preserve their useful content
width inside horizontal scrolling rather than compressing audio controls into
unreadable columns. The complete page remains vertically scrollable.

## Top bar

The top bar contains:

- Product logo, product name, build label, and short tagline
- CPU percentage
- Preset dirty indicator and preset selector
- Overflow menu for Save, Save As, and Delete
- Settings action
- Standalone window controls

The Settings action currently reports that settings are not implemented. It is
tracked as a release decision in the [roadmap](../docs/roadmap.md).

Plugin mode omits standalone window controls and relies on host window chrome.

## Input Source

The source card switches between **Preview File** and **Live Input** with a
segmented control. Both modes share input gain and normalization controls.

### Preview File

The implemented preview workflow contains:

- Filename and WAV metadata
- Browse action with loading spinner and disabled repeat activation
- Symmetric waveform envelope
- Return-to-start, play/pause, and stop controls
- Loop toggle
- Elapsed and total duration
- Inline decode/load errors

Replacing a preview keeps prior content visible until the new decode succeeds.
There is no timeline seeking in the current interface.

### Live Input

Standalone mode contains:

- Input-device selector
- Input-channel selector derived from the device topology
- Shared input/output buffer-size selector
- Monitor Input toggle

Output device and channels live in the Output card. The active monitoring rate
comes from the selected Windows output endpoint and is displayed in analysis and
status areas rather than exposed as an editable app control.

Plugin mode replaces device controls with a message that routing and buffer
configuration are supplied by the host.

## IR rack

The rack is the primary work area. It supports an arbitrary stored slot count,
vertical scrolling, and horizontal scrolling when the table cannot fit.

The header contains:

- Balance Mode
- Add IR
- Remove selected IR
- Clear All
- Normalize All

Each row contains these columns:

```text
# | Enable | IR File | Waveform | Level/Balance | Pan | Delay |
Polarity | Normalize | Solo | Mute | Actions
```

The filename block shows the WAV's native sample rate and frame count. The
waveform and analysis trace use the slot's stable identity color. Level and Pan
knobs remain neutral so color does not imply a semantic warning.

The row action menu provides Move Up, Move Down, Replace IR File, and Remove IR.
Reordering is explicit; drag-and-drop reorder is roadmap work.

Add and replace actions show an in-button spinner while background preparation
is active. Existing metadata remains visible during replacement. New slots show
an intentional loading state and errors stay associated with the affected row.

### Level and Balance Mode

Ordinary Level uses a compact dB knob. Balance Mode changes the column label and
control to a 0–100% contribution. The sole IR is disabled at 100%; edits with
multiple IRs immediately redistribute the remaining percentage.

Knobs support vertical drag, mouse wheel, keyboard arrows, Shift fine adjustment,
and double-click reset. Precise dB editing uses the adjacent numeric field where
the larger source/output controls provide one. Rack-level and balance values are
displayed directly on the compact knobs.

### Delay, pan, and channel actions

- Delay is entered in samples and displays milliseconds beneath it.
- Pan shows left, center, or right-oriented values through a constant-power law.
- Polarity uses an explicit Ø control.
- Solo and Mute use labeled toggles and permit multiple soloed rows.
- Normalize and Enable are checkboxes with visible checked states.

## Output

Standalone Output contains:

- Output device and topology-derived mono/stereo channel selector
- Output gain knob and precise dB editor
- Stereo level/peak meter
- Buffer-size selector
- **Limit Output (Prevent Clipping)** toggle
- Bypass toggle

The limiter is a real monitoring feature, not a visual placeholder. Bypass
crossfades to dry input. Plugin mode replaces device and buffer selectors with
host-routing status while retaining mix output controls.

## Export Mixed IR

Export settings are inline in the right-rail card; only destination selection
uses a native file dialog. The card contains:

- Output filename/destination action
- Sample rate
- Bit depth
- Mono/stereo mode
- Trim to Length toggle and length selector
- Final Normalize toggle
- Export action, progress, completion, and error state

While exporting, the action is disabled and progress is displayed. Monitoring
rate, bypass, and limiter state do not redefine export settings.

## Analysis & Preview

The analysis card contains four views:

- Frequency Response
- Impulse Response
- Phase
- Spectrogram

The current Spectrogram tab renders a real-time spectrum trace rather than a
time-frequency spectrogram. Renaming the tab or implementing a true spectrogram
is tracked in the roadmap.

Frequency and phase views show per-IR identity traces plus an emphasized mixed
trace. The impulse view shows the signed combined response. View and smoothing
selectors remain in the tab header. Analysis errors are shown inline; normal
recalculation keeps the previous completed graph visible.

The card footer displays monitoring sample rate, mixed IR length, estimated
latency, and CPU usage.

## Status bar

The bottom status bar shows:

- Engine status or current operation/error
- Active and total IR count
- Monitoring sample rate
- Buffer size and approximate duration

Success and error colors are paired with text and a status dot. Color alone is
never the only indication.

## Loading, empty, and error states

- Empty rack and preview areas have intentional instructional copy.
- Loading buttons retain their dimensions and replace icons with spinners.
- Decode, device, analysis, and export errors remain visible near the affected
  workflow and in global status when appropriate.
- Controls that cannot produce a valid action are disabled rather than silently
  accepting input.

## Responsive and accessibility requirements

- Tooltips and accessibility labels are mandatory for icon-only controls.
- Keyboard focus order follows visual order.
- Custom value widgets expose slider semantics through egui/AccessKit.
- Text truncation reveals full values on hover where filenames or devices may be
  long.
- Graphs, meters, and waveforms must remain sharp at tested scale factors.
- New UI behavior requires interaction tests and reviewed snapshots at affected
  widths/scales.

## Planned interactions

The following are deliberately not described as current behavior:

- Preview seeking
- Drag-and-drop reorder or file/folder drop
- Global keyboard shortcuts
- Right-click rack context menus
- Undo/redo
- Searchable preset browser and favorites
- Full Settings UI
- True time-frequency spectrogram

They are tracked in [roadmap.md](../docs/roadmap.md) and must update this contract
when implemented.

# IR Mixer Pro — Per-IR and Global Equalizers

This document defines the product and processing contract for the implemented
per-IR and post-mix global parametric equalizers. They are first-release
features built with the Rust
`biquad` crate.

The equalizer is deliberately narrower than a general impulse-response editor.
Trim, crop, fades, alignment, phase conversion, and other waveform editing are
outside this milestone.

## Product goal

Every loaded IR can have its own non-destructive parametric EQ, and the project
has one global EQ after the IR sum. A user can shape individual cabinet
responses before they are blended, shape the completed mix, compare either
stage with its unprocessed signal, save the settings, and hear the same result
in preview, live monitoring, analysis, and mixed-IR export.

Loading an IR does not create or enable any filters. An empty EQ chain, unity EQ
output gain, and EQ bypass off are sonically transparent.

## Equalizer model

Each IR slot and the project-level global EQ store:

- A complete-EQ bypass state
- EQ output gain in dB
- An ordered list of EQ bands
- A stable ID, enabled state, filter shape, and parameters for every band

Each equalizer supports up to 16 dynamic bands in v1. The prepared real-time
representation contains the finite band list for the current project;
adding or removing a band prepares replacement DSP state outside the audio
callback.

Stable band IDs survive preset round trips. Graph-node positions and display
order are not identities; selection and mutations address bands by stable ID.

### Filter shapes

| Shape | Controls | Purpose |
| --- | --- | --- |
| Bell | Frequency, gain, Q | Boost or cut a region centered on a frequency |
| Low shelf | Frequency, gain, Q | Boost or cut frequencies below a transition |
| High shelf | Frequency, gain, Q | Boost or cut frequencies above a transition |
| Notch | Frequency, Q | Reject a narrow frequency region |
| High-pass | Cutoff frequency, Q | Remove content below a cutoff |
| Low-pass | Cutoff frequency, Q | Remove content above a cutoff |

Frequency is constrained to 20 Hz–20 kHz and further constrained below Nyquist
for the representation being processed. Band and EQ-output gain use −12 dB to
+12 dB. Q uses 0.1–12 with a default of 1.0. High-pass and low-pass are
second-order 12 dB/octave filters whose Q controls resonance. Both per-IR and
global EQ use these limits.

Controls that do not apply to the selected shape are hidden or disabled. All
frequency values must remain valid for the active sample rate and below Nyquist.
Invalid restored or edited values are reported and safely constrained before
coefficient generation; they must never introduce non-finite audio.

### Band and chain actions

The equalizer supports:

- Create a Bell band from empty graph space at the chosen frequency and gain
- Change a selected band's shape or delete it from its graph-node context menu
- Enable or bypass an individual band without discarding its settings
- Bypass the complete EQ for original/EQ comparison
- Reset the complete EQ to an empty chain with unity output gain
- Copy the complete EQ chain and paste it onto another IR or the global EQ

Pasting creates new stable band IDs for the destination equalizer. It does not
copy an IR identity, rack gain, delay, pan, polarity, normalization, mute, solo,
or enabled state.

## Equalizer sliding pane

The rack remains compact. An EQ action on an IR row opens the pane for that
slot; the rack header's Global EQ action opens the same pane for the post-mix
target. Only one equalizer target is edited in the pane at a time.
Parameter changes update project state and audition immediately; closing the
pane does not discard them.

The pane uses the standard [`SlidingPane`](../design/ui-design-system.md#52-sliding-panes)
contract. Its explicit bounds cover the application body while leaving the
persistent top and status bars visible. The EQ pane overrides the standard pane
width with `min(920 points, available body width)`: it remains 920 points wide
when the body permits and shrinks to fit smaller windows without horizontal
overflow. Opening and closing retain the standard 240 ms cubic-out animation.
The header remains fixed while the EQ content scrolls vertically.

The pane closes from its header close button, Escape, or a click on the scrim.
Its content remains mounted throughout the closing animation and pane-specific
state is released only after `fully_closed` becomes true. Closing is not a
cancel operation because every edit has already been applied to project state.

The pane contains:

- The target name, target color, and IR metadata or global post-mix description
- Complete-EQ bypass and EQ output-gain controls
- An interactive logarithmic-frequency response graph
- One selected-band strip with Enabled, shape, frequency, gain when applicable,
  and Q controls
- Short interaction guidance for graph selection, dragging, scrolling, and
  context menus
- Processed peak/headroom information and an explicit clipping warning
- Reset-all and copy/paste-chain actions

The graph is the primary band editor:

- Clicking a node selects that band.
- Ordinary dragging changes frequency and gain together. For high-pass,
  low-pass, and notch bands, which have no gain parameter, vertical movement is
  ignored.
- Shift plus a predominantly horizontal drag changes frequency only. Shift plus
  a predominantly vertical drag changes gain only. This axis lock is specific to
  the EQ graph and overrides the general Shift fine-adjustment convention.
- The mouse wheel over a node changes Q logarithmically. Shift plus the mouse wheel changes
  gain in 0.1 dB steps. The gain gesture has no effect on high-pass, low-pass,
  or notch bands.
- Node wheel input is consumed so it does not scroll the sliding pane.
- Right-clicking a node opens a context menu containing the six supported filter
  shapes and Delete.
- Right-clicking empty graph space immediately creates and selects a Bell band
  at the clicked frequency and gain with Q 1.0.

Frequency follows the graph's logarithmic axis. Gain follows its dB axis. All
gestures update the curve, selected-band strip, monitoring parameters, and
background analysis request continuously while remaining safely constrained to
valid parameter and Nyquist limits.

The selected-band strip edits only the current graph selection. It contains an
Enabled checkbox, shape selector, frequency, gain when applicable, and Q.
When no band is selected, every field displays `None`, uses the disabled visual
treatment, and rejects mouse and keyboard interaction. There is no stacked band
list, Add Band button, band reorder, duplicate-band action, individual-band
reset action, or delete icon in the strip.

The graph displays the EQ filter transfer as an editing preview on labeled
20 Hz-20 kHz and -12 dB to +12 dB axes. The preview always shows the enabled
band cascade: complete-EQ bypass does not flatten it and EQ output gain does not
move it vertically. Those controls still affect auditioning, analysis, export,
and headroom calculations according to the processing contract.

Band nodes are colored and labeled by shape: Bell is blue and uses its
zero-based position in the complete band list (`00`-`99`); low shelf is cyan
`LS`; high shelf is violet `HS`; notch is red `NT`; high-pass is green `HP`;
and low-pass is yellow `LP`. High-pass and low-pass nodes also draw a dotted
full-height guide at their cutoff frequency in the node color at 10% opacity.
Selection adds the application focus ring without replacing the shape color.
Disabled nodes remain visible and selectable with reduced opacity but do not
contribute to the preview curve or audio.

Complete-EQ bypass is the original/EQ comparison control. Transitions must be
click-free, while the graph remains available for preview and editing. There is
no automatic level matching: EQ output gain is an explicit user control, rack
level remains a separate mix control, and neither changes the graph's vertical
position.

The component gallery must include deterministic equalizer-pane state using
`SlidingPane` and exercise graph creation, selection, dragging, axis locking,
wheel editing, context-menu type changes and deletion, bypass, empty selection,
reset-all, and chain copy/paste. The production pane remains a controlled UI
surface driven by application snapshots and typed commands.

## Processing contract

The monitoring path is conceptually:

```text
source
    → partitioned convolution
    → ordered EQ-band cascade
    → EQ output gain
    → delay / polarity / rack gain / pan
    → mix
    → global ordered EQ-band cascade
    → global EQ output gain
    → master output gain / bypass / limiter
```

Each channel has independent filter state. Mono and stereo IRs share the same
controls; stereo IRs do not share left/right delay elements or filter history.
Independent left/right EQ controls are outside this milestone.

The real-time implementation uses `biquad` 0.6 `DirectForm1<f32>` second-order
IIR sections. Coefficients and replacement filter banks are prepared and validated
outside the callback. The audio callback must not allocate, deallocate, block,
lock a mutex, access the filesystem, or evaluate unbounded coefficient-design
work. Parameter and topology changes are smoothed or crossfaded so that adding,
removing, bypassing, changing shape, or retuning a band does not click.

Ordinary EQ changes do not rebuild partitioned convolution state. The immutable
native-rate decoded IR remains the source of truth for later monitoring-rate
changes, analysis, and export.

No stage applies implicit normalization, automatic gain compensation, or
limiting. The pane reports processed peak and headroom. Existing monitoring
limiting and export clipping rules remain unchanged.

## Analysis and export parity

Frequency, phase, and combined impulse analysis include every active per-IR EQ
chain and its EQ output gain, followed by the global EQ and its output gain.
Background analysis follows the existing
latest-state scheduling rule: completed graphs remain visible until the newest
calculation replaces them.

Mixed-IR export applies the same logical transforms at the selected export
sample rate:

```text
native-rate source
    → direct resample to export rate
    → per-IR normalization when enabled
    → ordered EQ-band cascade and EQ output gain
    → delay / polarity / rack gain / pan
    → sum
    → global EQ cascade and global EQ output gain
    → existing output/export stages
```

Coefficient generation uses the rate of the representation being processed.
Export does not reuse monitoring-rate coefficients or prepared audio. Because an
IIR response is theoretically infinite, offline rendering feeds silence until
both channels remain below −120 dBFS for 256 consecutive frames, capped at two
seconds, before the existing final trim/pad choice is applied.
Monitoring bypass and the safety limiter are still excluded from exported IRs.

The milestone does not add a separate single-IR export workflow. The existing
mixed-IR export is the required export surface and must reproduce the configured
EQ mix.

## Presets and plugin state

Per-IR and global EQ bypass, output gain, ordered bands, stable band IDs, shapes,
enabled states, and parameters are portable project state. They are saved in
standalone presets and future plugin sessions. Missing IR files retain their EQ
settings so the project can recover without losing edits when the file is
relinked.

Preset schema version 5 stores global EQ state; version 4 already stores per-IR
EQ state. Older presets migrate missing targets to an empty, enabled, unity-gain
EQ chain. Plugin automation mapping for
dynamic bands is a plugin-adapter design decision; complete session-state
persistence is required regardless of automation exposure.

## Acceptance criteria

- A new or migrated IR with no bands is sample-equivalent to the pre-EQ path.
- Every supported shape produces finite, sample-rate-correct coefficients and
  the expected magnitude response.
- Graph creation, selection, drag, wheel, type change, deletion, bypass,
  reset-all, and copy/paste preserve the documented stable-ID behavior.
- Warmed-up processing performs no heap operations on the audio callback.
- Mono and stereo processing preserve channel topology and independent state.
- Parameter, bypass, and topology changes are free of audible discontinuities.
- Monitoring, background analysis, and mixed export agree within documented
  floating-point and finite-tail tolerances.
- Preset migration and round trips preserve per-IR and global EQ state and
  missing-file slots.
- The pane is 920 points wide when space permits and shrinks to the available
  body width at smaller supported window sizes.
- Graph and selected-strip interaction tests cover normal drag, Shift axis lock,
  wheel consumption, Shift-wheel gain steps, context menus, no-gain shapes, and
  the disabled no-selection state.
- Pane opening, closing, dismissal, interaction, and visual snapshots cover
  supported layouts and DPI scales, with keyboard and accessibility behavior
  matching the design system.

## Explicit exclusions

- IR trim, crop, fades, gates, or envelope editing
- Automatic alignment, polarity recommendations, or fractional delay
- Minimum-phase conversion or manual phase rotation
- Independent left/right EQ controls
- Target-curve matching or batch editing
- Single edited-IR export
- General editor undo/redo

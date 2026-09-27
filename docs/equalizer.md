# IR Mixer Pro — Per-IR Equalizer

This document defines the product and processing contract for the planned
per-IR parametric equalizer. The equalizer is the next implementation milestone
and a first-release requirement. It is not part of the currently implemented
audio path.

The equalizer is deliberately narrower than a general impulse-response editor.
Trim, crop, fades, alignment, phase conversion, and other waveform editing are
outside this milestone.

## Product goal

Every loaded IR can have its own non-destructive parametric EQ. A user can shape
individual cabinet responses before they are blended, compare the result with
the unprocessed IR, inspect the response, save the settings, and hear the same
result in preview, live monitoring, analysis, and mixed-IR export.

Loading an IR does not create or enable any filters. An empty EQ chain, unity EQ
output gain, and EQ bypass off are sonically transparent.

## Equalizer model

Each IR slot stores:

- A complete-EQ bypass state
- EQ output gain in dB
- An ordered list of EQ bands
- A stable ID, enabled state, filter shape, and parameters for every band

The project model does not impose a fixed number of bands. The prepared
real-time representation contains the finite band list for the current project;
adding, removing, duplicating, or reordering bands prepares replacement DSP
state outside the audio callback.

Stable band IDs survive reorder operations and preset round trips. Display
positions are not identities. Reordering changes the stored cascade order and
must not recreate unrelated band IDs.

### Filter shapes

| Shape | Controls | Purpose |
| --- | --- | --- |
| Bell | Frequency, gain, Q | Boost or cut a region centered on a frequency |
| Low shelf | Frequency, gain, Q/slope | Boost or cut frequencies below a transition |
| High shelf | Frequency, gain, Q/slope | Boost or cut frequencies above a transition |
| Notch | Frequency, Q | Reject a narrow frequency region |
| High-pass | Cutoff frequency, Q/slope | Remove content below a cutoff |
| Low-pass | Cutoff frequency, Q/slope | Remove content above a cutoff |

Controls that do not apply to the selected shape are hidden or disabled. All
frequency values must remain valid for the active sample rate and below Nyquist.
Invalid restored or edited values are reported and safely constrained before
coefficient generation; they must never introduce non-finite audio.

### Band and chain actions

The equalizer supports:

- Add, remove, duplicate, and reorder bands
- Enable or bypass an individual band without discarding its settings
- Reset an individual band to a neutral default for its shape
- Bypass the complete EQ for original/EQ comparison
- Reset the complete EQ to an empty chain with unity output gain
- Copy the complete EQ chain and paste it onto another IR

Pasting creates new stable band IDs for the destination IR. It does not copy the
source IR identity, rack gain, delay, pan, polarity, normalization, mute, solo,
or enabled state.

## Equalizer sliding pane

The rack remains compact. An EQ action on an IR row opens a right-edge sliding
pane for that slot. Only one IR equalizer is edited in the pane at a time.
Parameter changes update project state and audition immediately; closing the
pane does not discard them.

The pane uses the standard [`SlidingPane`](../design/ui-design-system.md#52-sliding-panes)
contract. Its explicit bounds cover the application body while leaving the
persistent top and status bars visible. It uses the standard 460-point width and
240 ms cubic-out opening and closing animation. The header remains fixed while
the EQ content scrolls vertically.

The pane closes from its header close button, Escape, or a click on the scrim.
Its content remains mounted throughout the closing animation and pane-specific
state is released only after `fully_closed` becomes true. Closing is not a
cancel operation because every edit has already been applied to project state.

The pane contains:

- The IR filename, identity color, and native WAV metadata
- Complete-EQ bypass and EQ output-gain controls
- An interactive logarithmic-frequency response graph
- An ordered, scrollable band list with shape-specific controls
- Add, duplicate, reorder, remove, enable/bypass, and reset actions
- Processed peak/headroom information and an explicit clipping warning
- Reset-all and copy/paste-chain actions

Selecting a graph node selects its band row, and selecting a band row highlights
its graph node. The graph displays the original IR response, processed IR
response, and combined EQ transfer curve. It uses the IR's stable identity color
without treating that color as a warning or status.

Complete-EQ bypass is the original/EQ comparison control. Transitions must be
click-free. There is no automatic level matching: EQ output gain is an explicit
user control, and rack level remains a separate mix control.

The component gallery must include deterministic equalizer-pane state using
`SlidingPane` and exercise adding, duplicating, removing, reordering, selecting,
changing shape,
bypassing, resetting, and copying/pasting bands. The production pane remains a
controlled UI surface driven by application snapshots and typed commands.

## Processing contract

The monitoring path for each active IR is conceptually:

```text
source
    → partitioned convolution
    → ordered EQ-band cascade
    → EQ output gain
    → delay / polarity / rack gain / pan
    → mix
```

Each channel has independent filter state. Mono and stereo IRs share the same
controls; stereo IRs do not share left/right delay elements or filter history.
Independent left/right EQ controls are outside this milestone.

The real-time implementation uses second-order IIR sections for the supported
shapes. Coefficients and replacement filter banks are prepared and validated
outside the callback. The audio callback must not allocate, deallocate, block,
lock a mutex, access the filesystem, or evaluate unbounded coefficient-design
work. Parameter and topology changes are smoothed or crossfaded so that adding,
removing, bypassing, reordering, or retuning a band does not click.

Ordinary EQ changes do not rebuild partitioned convolution state. The immutable
native-rate decoded IR remains the source of truth for later monitoring-rate
changes, analysis, and export.

No stage applies implicit normalization, automatic gain compensation, or
limiting. The pane reports processed peak and headroom. Existing monitoring
limiting and export clipping rules remain unchanged.

## Analysis and export parity

Frequency, phase, and combined impulse analysis include every active per-IR EQ
chain and its EQ output gain. Background analysis follows the existing
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
    → sum and existing output/export stages
```

Coefficient generation uses the rate of the representation being processed.
Export does not reuse monitoring-rate coefficients or prepared audio. Because an
IIR response is theoretically infinite, offline rendering must include a finite,
documented tail policy before the existing final trim/pad choice is applied.
Monitoring bypass and the safety limiter are still excluded from exported IRs.

The milestone does not add a separate single-IR export workflow. The existing
mixed-IR export is the required export surface and must reproduce the configured
EQ mix.

## Presets and plugin state

Per-IR EQ bypass, output gain, ordered bands, stable band IDs, shapes, enabled
states, and parameters are portable project state. They are saved in standalone
presets and future plugin sessions. Missing IR files retain their EQ settings so
the project can recover without losing edits when the file is relinked.

The preset schema must be versioned when EQ state is implemented. Older presets
migrate to an empty, enabled, unity-gain EQ chain. Plugin automation mapping for
dynamic bands is a plugin-adapter design decision; complete session-state
persistence is required regardless of automation exposure.

## Acceptance criteria

- A new or migrated IR with no bands is sample-equivalent to the pre-EQ path.
- Every supported shape produces finite, sample-rate-correct coefficients and
  the expected magnitude response.
- Band reorder, duplication, removal, bypass, reset, and copy/paste preserve the
  documented stable-ID behavior.
- Warmed-up processing performs no heap operations on the audio callback.
- Mono and stereo processing preserve channel topology and independent state.
- Parameter, bypass, and topology changes are free of audible discontinuities.
- Monitoring, background analysis, and mixed export agree within documented
  floating-point and finite-tail tolerances.
- Preset migration and round trips preserve all EQ state and missing-file slots.
- Pane opening, closing, dismissal, interaction, and visual snapshots cover
  supported layouts and DPI scales, with keyboard and accessibility behavior
  matching the design system.

## Explicit exclusions

- Master or output equalization
- IR trim, crop, fades, gates, or envelope editing
- Automatic alignment, polarity recommendations, or fractional delay
- Minimum-phase conversion or manual phase rotation
- Independent left/right EQ controls
- Target-curve matching or batch editing
- Single edited-IR export
- General editor undo/redo

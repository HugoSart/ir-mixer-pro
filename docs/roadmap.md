# IR Mixer Pro — Roadmap

This roadmap organizes delivery work without serving as a field-by-field status
matrix. The [product specification](product-spec.md) remains the normative
first-release contract.

## Shared standalone foundation

The current foundation includes the shared project model and UI, native Windows
standalone host, partitioned convolution, preview and live monitoring, native
IR preservation, mixed-rate export, analysis, presets, component gallery, and
visual regression coverage.

Continue hardening this foundation as plugin work exposes host-specific block
sizes, rates, lifecycle transitions, and state restoration cases.

## Per-IR and global equalizers

The per-IR and post-mix global equalizers are implemented as first-release
features. Their product and
processing contract is defined in [equalizer.md](equalizer.md).

- Add a responsive right-edge equalizer pane targeting 920 points and shrinking
  to the available body width on smaller windows.
- Support an ordered dynamic band list with bell, shelf, notch, high-pass, and
  low-pass shapes.
- Make the response graph the primary editor with node dragging, Shift axis
  lock, wheel Q/gain editing, and context-menu creation, type changes, and
  deletion.
- Add one selected-band value strip, complete-EQ and per-band bypass, EQ output
  gain, headroom feedback, reset-all, and copy/paste workflows.
- Persist per-IR and global EQ state in presets and future plugin sessions with
  schema migration.
- Apply equivalent EQ behavior to monitoring, analysis, and mixed-IR export.
- Keep coefficient preparation and topology changes outside the audio callback,
  with click-free real-time transitions and allocation-free warmed-up processing.
- Add deterministic equalizer-pane gallery coverage, sliding-pane interaction
  tests, DSP tests, state round trips, analysis/export parity tests, and reviewed
  UI snapshots.

## VST3 and CLAP adapters

These adapters are required to complete the multi-format first release:

- Add a plugin crate and nice-plug VST3/CLAP entrypoints.
- Map host sample rate, block size, process buffers, and lifecycle into the
  shared engine without CPAL.
- Define automatable parameters and stable IDs for practical mix controls.
- Persist project state and IR references in host sessions.
- Render `AppPage` in plugin mode with host-routing status.
- Validate project reload, missing IRs, editor reopen, bypass, and export in
  representative hosts.
- Build and package VST3 and CLAP bundles alongside the standalone executable.

## Release hardening

- Derive the displayed application version from package/build metadata instead
  of the current hardcoded UI label.
- Keep formatting and strict workspace Clippy checks clean as features evolve.
- Add CI for formatting, Clippy, tests, release compilation, and artifact checks.
- Choose and add the repository/distribution license before public packaging.
- Extend release tooling to package and checksum standalone, VST3, and CLAP
  artifacts as one release set.
- Decide whether Settings is required for first release; implement it or remove
  the inactive top-bar action.
- Resolve the current “Spectrogram” label: rename it to Spectrum for the existing
  trace or implement a true time-frequency spectrogram.
- Complete clean-machine and multi-host smoke tests.
- Add Authenticode signing and verify signatures before archive creation.

## Interaction and workflow polish

- Drag-and-drop IR reorder and multi-file/folder drop
- Preview timeline seeking
- Keyboard shortcuts and fuller keyboard navigation
- Confirmation for destructive preset/rack operations where appropriate
- IR relinking and configurable search roots for missing files
- Clearer device-disconnect recovery and rescan behavior
- Measured performance guidance for long room IRs and high sample rates

## Post-release exploration

- Automatic delay estimation and phase alignment
- Correlation and alignment visualization
- IR trim/crop and minimum-phase conversion
- Searchable/tagged IR library
- Snapshots and preset morphing
- Batch export and built-in test signals
- AIFF and FLAC preview/import
- AU or AAX adapters
- Optional amp or NAM processing before the IR stage

Amp modeling, drive modeling, cloud sync, marketplace features, and mobile builds
remain outside the first-release plan.

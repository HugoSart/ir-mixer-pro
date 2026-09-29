# Developing IR Mixer Pro

## Toolchain

Use the current stable Rust MSVC toolchain on 64-bit Windows. Install Visual
Studio Build Tools with **Desktop development with C++** and a current Windows
SDK. The root build script uses the SDK resource compiler to embed the desktop
icon in release builds.

Confirm the environment:

```powershell
rustc -Vv
cargo -V
```

## Run modes

Native standalone application:

```powershell
cargo run --release
```

Use release mode for audio validation. Debug-mode convolution and UI work can
miss real-time deadlines at small buffers and sound glitchy.

Mock application state without native audio or file I/O:

```powershell
$env:IR_MIXER_MOCK = "1"
cargo run
```

Component gallery:

```powershell
cargo run -p ir-ui-gallery
```

The gallery owns independent dummy state and is intended for interaction and
component review, not audio behavior.

## Validation commands

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --release -p ir-mixer-pro
```

The release packaging script runs the first three checks before building unless
`-SkipChecks` is supplied. Do not use that switch to bypass failures that have
not already passed for the same revision.

Current release-hardening work is tracked in [roadmap.md](roadmap.md).

## Test organization

- Pure state and preset migrations: `ir-app`
- Audio buffers, WAV formats, analysis, resampling, and export: `ir-core`
- Convolution and allocation-free warmed-up processing: `ir-dsp`
- Native worker, device, preview, export, and backend behavior: `ir-native`
- Component interaction, responsive pages, and rendered snapshots: `ir-ui`
- Gallery dummy-state behavior: `ir-ui-gallery`

The DSP benchmark is available with:

```powershell
cargo bench -p ir-dsp --bench realtime
```

Use benchmark results before changing active-IR limits, partition strategy, or
supported block sizes.

## Visual regression workflow

Snapshot baselines live under `crates/ir-ui/tests/snapshots`. Run them normally:

```powershell
cargo test -p ir-ui
```

When a visual change is intentional, render and inspect every affected image at
its tested scale before updating baselines:

```powershell
$env:UPDATE_SNAPSHOTS = "force"
cargo test -p ir-ui
```

Commit only the reviewed final PNGs. Remove generated `.old` and `.diff`
artifacts after comparison.

## Architecture rules

- Keep UI widgets controlled and side-effect free.
- Keep native devices, files, dialogs, and workers behind `AudioBackend`.
- Keep all unbounded work away from the audio callback.
- Preserve native-rate audio sources; prepare disposable engine-rate copies.
- Use stable IDs across UI actions and model mutations.
- Update architecture, product, audio, or design documentation in the same
  change when an implementation contract moves.

## Common diagnostics

### Glitchy monitoring

1. Reproduce in a release build.
2. Increase the buffer from 64 to 128 or 256 frames.
3. Check the xrun count and CPU display.
4. Reduce the number or length of active IRs.
5. Confirm the selected Windows input/output default formats match for live mode.

Improvement at larger buffers or higher rates can indicate deadline scheduling
rather than bad WAV data. Compare the same input and IR in offline export to
separate real-time performance from mix correctness.

### Live input will not start

- Confirm both endpoints still exist and the chosen channels are valid.
- Match input and output shared-mode sample rates in Windows or the device panel.
- Remember that input capture is opened only in Live Input mode with monitoring
  enabled.

### IR metadata looks different from engine status

The rack intentionally shows each WAV's native rate and frame count. The status
and analysis strip show the monitoring engine rate. See
[audio-processing.md](audio-processing.md).

### Export clipping

Enable final normalization, lower IR/output gains, or use Float32 when headroom
above full scale is intentional. PCM export refuses to clip silently.

### Release build cannot find `rc.exe`

Install a Windows SDK with Visual Studio Build Tools or set `RC` to the exact
resource compiler executable. The root `build.rs` searches installed Windows 10
Kit versions when `RC` is not set.

### Snapshot mismatch

Inspect the generated current, old, and diff images. Update baselines only if
the implementation change is intentional; otherwise fix the rendering change.

## Documentation conventions

- Markdown filenames under `docs/` and `design/` use lowercase kebab-case.
- README and internal documents remain version agnostic; package metadata is
  authoritative for builds.
- Product requirements, current architecture, design contracts, and roadmap
  work must remain visibly distinct.
- The current application screenshot and reviewed `ir-ui` snapshots describe
  implemented visuals. Historical mockups remain references, not current truth.

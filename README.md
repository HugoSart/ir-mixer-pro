# IR Mixer Pro

IR Mixer Pro is a Rust desktop application and audio plugin for loading,
blending, previewing, analyzing, and exporting guitar cabinet impulse responses.

The project is currently in its UI-first phase. The reusable egui design system
and native component gallery are implemented; the application shell, audio
backend, convolution engine, VST3, and CLAP integrations come next.

![Approved IR Mixer Pro interface](docs/mockups/main-screen.png)

## Run the component gallery

Install the current stable Rust toolchain, open a terminal in the repository
root, and run:

```powershell
cargo run -p ir-ui-gallery
```

The gallery is the executable reference for colors, typography, buttons,
checkboxes, dropdown and list selectors, toggles, value controls, waveforms,
meters, graphs, and component states. Its icon-only actions include both
outlined and borderless variants.

Running plain `cargo run` starts the root placeholder binary in `src/main.rs`,
not the component gallery.

The **Cards** tab shows Input Source (Preview and Live), Output, and Export Mixed
IR cards. Controls update dummy gallery state; Browse cycles example filenames and
Play advances a simulated clock. The Output meter is animated; device, file, and
export actions are only simulated—no audio devices or files are opened.

## Development checks

Run the full workspace test suite:

```powershell
cargo test --workspace
```

Run formatting and lint checks:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Visual regression baselines live in `crates/ir-ui/tests/snapshots`. Update them
only after reviewing the rendered changes:

```powershell
$env:UPDATE_SNAPSHOTS = "force"
cargo test -p ir-ui
```

## Repository layout

```text
crates/ir-ui/       Reusable theme, palette, fonts, and egui widgets
tools/ui-gallery/   Native Storybook-style component gallery
docs/               Product, architecture, and UI specifications
src/main.rs         Placeholder for the future standalone application
```

Start with these documents:

- [Product specification](docs/PRODUCT_SPEC.md)
- [Architecture](docs/ARCHITECTURE.md)
- [UI design](docs/UI_DESIGN.md)
- [UI design system](docs/UI_DESIGN_SYSTEM.md)

## Planned product targets

- Standalone desktop application
- VST3 plugin
- CLAP plugin

All targets will share the same application model, DSP engine, and UI component
library where host constraints allow it.

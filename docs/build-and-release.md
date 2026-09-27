# Building and Releasing IR Mixer Pro

## Artifact scope

The product's first-release artifact set is:

- Windows x64 standalone application
- VST3 plugin bundle
- CLAP plugin bundle

The repository currently implements and packages only the standalone executable.
The existing ZIP is suitable for internal standalone testing, but it is not the
complete multi-format product release. Plugin adapters and bundle packaging are
tracked in the [roadmap](roadmap.md).

## Prerequisites

Install on 64-bit Windows:

1. The current stable Rust MSVC toolchain.
2. Visual Studio Build Tools with **Desktop development with C++**.
3. A current Windows SDK, including `rc.exe` and signing tools when signing.
4. Git for building from a clean, identifiable revision.

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustc -Vv
cargo -V
```

The root `build.rs` generates a multi-size `.ico`, compiles it as a Windows
resource, and links it into the standalone executable. Set `RC` to an exact
resource-compiler path only when Windows SDK discovery is insufficient.

## Required validation gate

Every candidate revision must pass:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --locked -p ir-mixer-pro
```

Do not redefine the documented gate to hide failures. Known cleanup preventing
a fully green gate is listed in [roadmap.md](roadmap.md#release-hardening).

Plugin adapters must eventually add bundle builds, host validation, and state
round-trip tests to this gate.

## Current standalone package

From the repository root:

```powershell
.\tools\build-release.ps1
```

Unless `-SkipChecks` is supplied, the script runs formatting, strict Clippy, and
the full workspace test suite. It then:

1. Builds `ir-mixer-pro` in release mode with the lockfile.
2. Reads the package version from Cargo metadata.
3. Copies the executable as `IR Mixer Pro.exe` into a staging directory.
4. Adds `README.md` and `LICENSE` when a license file exists.
5. Creates a versioned Windows x64 ZIP under `dist\`.
6. Writes a lowercase SHA-256 checksum beside the archive.

Use `-SkipChecks` only when the same revision has already passed the complete
gate in a trusted environment:

```powershell
.\tools\build-release.ps1 -SkipChecks
```

Manual standalone build:

```powershell
cargo build --release --locked -p ir-mixer-pro
```

Output:

```text
target\release\ir-mixer-pro.exe
```

Release builds use thin LTO, one code-generation unit, stripped symbols, and the
Windows GUI subsystem. They display no console window.

## Version source

Cargo package metadata is authoritative. General documentation intentionally
contains no fixed product version. Before producing artifacts:

1. Set the intended semantic version in the root package manifest.
2. Keep workspace package versions coherent where they are released together.
3. Update dependencies only when intentional and commit `Cargo.lock`.
4. Remove hardcoded UI version labels in favor of build metadata.
5. Build from a clean, tagged revision.

Preset schema versions are independent of package versions and must not be
changed merely because a release number changes.

## Signing

Unsigned executables and plugin bundles may trigger reputation or security
warnings. Sign every binary artifact before final archives are created. With
`signtool.exe` from the Windows SDK:

```powershell
signtool sign /fd SHA256 /tr https://timestamp.digicert.com /td SHA256 /a "target\release\ir-mixer-pro.exe"
signtool verify /pa /v "target\release\ir-mixer-pro.exe"
```

Extend the same policy to plugin binaries. Never commit certificates, private
keys, PINs, passwords, or hardware-token credentials. CI must obtain signing
access through its secret store.

## Standalone smoke test

Run the packaged executable on a clean Windows account or VM:

- Product name and icon appear correctly in the window, taskbar, and Alt-Tab.
- No console window opens.
- Preview WAV browse, play, pause, restart, stop, EOF, and loop work.
- Live input monitoring works when Windows input/output default rates match.
- Device/channel changes rebuild streams and preserve valid selections.
- Native IR rate and frame count remain correct in rack metadata.
- Add, replace, enable, balance, gain, delay, pan, polarity, normalize, mute,
  solo, reorder, remove, and clear operations work.
- An empty or inaudible rack passes the monitored source through.
- Output gain, bypass, limiter, meters, CPU, latency, and xrun status respond.
- Frequency, impulse, phase, and spectrum views update after mix changes.
- Preset save, overwrite, load, migration, missing-file state, and delete work
  from paths containing spaces and non-ASCII characters.
- Export works for every offered rate, encoding, channel mode, and length with
  trimming and normalization on and off.
- A 96 kHz native IR remains native quality in a 96 kHz export while monitoring
  at a different rate.
- Integer clipping errors are explicit; Float32 headroom exports successfully.
- Exported WAVs reopen in IR Mixer Pro and an independent audio tool.
- Resize, minimize, maximize, restore, and close work at common Windows scaling
  factors, including the supported 880 × 680 minimum window.

Verify the published ZIP checksum after upload and scan artifacts with the
organization's normal malware/reputation tooling.

## Plugin smoke-test requirements

Before a multi-format release, validate both VST3 and CLAP in representative
hosts:

- Discovery, scanning, instantiation, and editor lifecycle
- Mono/stereo routing and varying host block sizes
- Sample-rate changes and offline rendering
- Automation for exposed parameters
- Session save/reload with available and missing IR files
- Multiple instances and rapid create/destroy cycles
- Host bypass, suspend/resume, and transport transitions
- Preset and mixed-IR export dialogs without blocking the process callback

## Distribution

The current ZIP has no installer. A public distribution decision must cover:

- Repository and binary license
- Install locations for standalone, VST3, and CLAP artifacts
- Start Menu and uninstall behavior
- Upgrade handling and optional file associations
- Signing and reputation strategy
- Whether a portable ZIP remains available beside an installer

Choose installer technology only after these requirements and plugin bundle
locations are finalized.

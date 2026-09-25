# Building and Shipping IR Mixer Pro

## Supported MVP artifact

The current production artifact is the Windows x64 standalone application:

```text
IR Mixer Pro.exe
```

VST3 and CLAP are product targets, but host adapters and plugin bundles are not
implemented in this milestone. Do not label the present build as containing
VST3 or CLAP plugins.

## Prerequisites

Install on a 64-bit Windows machine:

1. The current stable Rust MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`).
2. Visual Studio Build Tools with **Desktop development with C++** and a recent
   Windows SDK. Rust uses the MSVC linker from this installation.
3. Git, for obtaining a clean release revision.

Confirm the toolchain:

```powershell
rustc -Vv
cargo -V
```

## Automated release package

From the repository root:

```powershell
.\tools\build-release.ps1
```

This performs:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo build --release --locked -p ir-mixer-pro`
5. Packaging of the executable, README, license when present, and SHA-256 file
   into a versioned ZIP under `dist\`.

Use `-SkipChecks` only after the same commit has passed the checks in CI:

```powershell
.\tools\build-release.ps1 -SkipChecks
```

## Manual build

For an unpackaged executable:

```powershell
cargo build --release --locked -p ir-mixer-pro
```

Output:

```text
target\release\ir-mixer-pro.exe
```

Release builds use thin LTO, one code-generation unit, and stripped symbols.
They use the Windows GUI subsystem, so no console window appears. The native
window title is `IR Mixer Pro`; the taskbar and Alt-Tab icon are generated from
`design\logos\desktop-icon.png` when the window is created.

## Versioning

Before a public build:

1. Set the intended semantic version in the root `Cargo.toml` package section.
2. Run `cargo update --workspace` only when dependency changes are intentional.
3. Commit `Cargo.lock`; production builds use `--locked`.
4. Build from a clean, tagged revision, for example `v0.1.0`.

## Code signing

Unsigned executables can trigger Windows SmartScreen warnings. For distribution,
sign both the executable and final installer (when an installer is introduced)
with an Authenticode certificate. With `signtool.exe` from the Windows SDK:

```powershell
signtool sign /fd SHA256 /tr https://timestamp.digicert.com /td SHA256 /a "target\release\ir-mixer-pro.exe"
signtool verify /pa /v "target\release\ir-mixer-pro.exe"
```

Signing must happen before ZIP creation. Never commit certificates, private
keys, passwords, or hardware-token credentials. CI secrets should provide
signing access.

## Release smoke test

Test the packaged executable on a clean Windows user account or VM:

- The taskbar and Alt-Tab show the IR Mixer Pro icon and name.
- The app opens without a console window.
- A preview WAV plays, pauses, loops, stops, and restarts after reaching EOF.
- Input and output device selection survives stream restart.
- Loading, enabling, muting, soloing, reordering, and removing IRs works.
- An empty IR rack passes preview/live input through.
- Export works for mono and stereo, 16-bit, 24-bit, and 32-bit float, all sample
  rates and lengths, with normalization both enabled and disabled.
- Exported WAV files reopen in IR Mixer Pro and a second independent audio tool.
- Preset save, load, and delete work from a path containing spaces and non-ASCII
  characters.
- Resize, minimize, maximize, restore, and close work at 100%, 150%, and 200%
  Windows display scaling.

Also scan the ZIP with the organization's normal malware/reputation tooling and
verify the published SHA-256 checksum after upload.

## Distribution notes

The ZIP is suitable for MVP/test distribution. A production installer should
later add Start Menu shortcuts, uninstall support, installed executable icon and
metadata, optional file associations, and signed upgrade handling. Choose an
installer technology only when those requirements are finalized.

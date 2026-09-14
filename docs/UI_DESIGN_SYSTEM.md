# IR Mixer — UI Design System

## 1. Purpose and Source of Truth

This document is the normative visual contract for IR Mixer. The approved
[`main-screen.png`](mockups/main-screen.png) mockup is the visual source of truth.
[`UI_DESIGN.md`](UI_DESIGN.md) defines product layout and behavior; this document
defines how the interface is rendered.

Use the token names below in Rust. Do not scatter unnamed colors, spacing values,
corner radii, or text sizes throughout widget code.

All dimensions are logical pixels (egui points) and must scale with the platform
pixel density.

## 2. Color System

### 2.1 Neutral surfaces

| Token | Value | Use |
| --- | --- | --- |
| `surface.canvas` | `#071117` | Window canvas and status-bar background |
| `surface.toolbar` | `#0E1A24` | Top toolbar |
| `surface.panel` | `#0E1F29` | Primary panels and cards |
| `surface.inset` | `#0B161D` | Rack rows, graphs, and recessed areas |
| `surface.control` | `#17222D` | Inputs and idle buttons |
| `surface.hover` | `#1D2D3A` | Hovered neutral controls |
| `surface.pressed` | `#101B24` | Pressed neutral controls |

### 2.2 Structure and text

| Token | Value | Use |
| --- | --- | --- |
| `border.subtle` | `#20313E` | Dividers and low-emphasis outlines |
| `border.control` | `#30485A` | Interactive control outlines |
| `border.strong` | `#416077` | Hovered or emphasized outlines |
| `text.primary` | `#D5E4F5` | Titles and primary values |
| `text.secondary` | `#9BB0C7` | Labels and metadata |
| `text.muted` | `#61768C` | Disabled and tertiary information |
| `text.on_accent` | `#F7FBFF` | Text/icons on the blue accent |

### 2.3 Application accent

The application accent is the electric blue visible in the approved mockup. It
belongs to interaction, not to audio-channel identity.

| Token | Value | Use |
| --- | --- | --- |
| `accent.default` | `#0F82EC` | Primary actions and selected controls |
| `accent.hover` | `#2498FF` | Hovered primary actions |
| `accent.pressed` | `#0870C8` | Pressed primary actions |
| `accent.focus` | `#54B6FF` | Keyboard focus ring |
| `accent.soft` | `#153957` | Selected backgrounds without solid fill |

### 2.4 IR identity palette

IR colors are categorical identities only. A red, yellow, or orange IR is not an
error or warning. Apply the assigned color consistently to the IR waveform, fader
fill, meter accent, graph curve, and legend marker.

| Index | Name | Value |
| ---: | --- | --- |
| 0 | Blue | `#0E7EE9` |
| 1 | Violet | `#9845E7` |
| 2 | Green | `#5BE245` |
| 3 | Yellow | `#E9D71A` |
| 4 | Orange | `#EC9134` |
| 5 | Red | `#EC5D6A` |
| 6 | Cyan | `#22C7DD` |
| 7 | Magenta | `#D568C4` |

Assign the next palette index when an IR is added. Reordering or deleting slots
must not recolor remaining IRs. Store the index in project and preset state. After
index 7, cycle from index 0; do not generate unreviewed colors at runtime.

### 2.5 Semantic feedback

Semantic colors are scoped to status components and never determine an IR's
identity. Always pair them with an icon, label, meter position, or other non-color
cue.

| Token | Value | Use |
| --- | --- | --- |
| `status.success` | `#62D889` | Completed operations and healthy state |
| `status.warning` | `#E6B94E` | Recoverable warnings |
| `status.danger` | `#F05B6A` | Errors, clipping, and destructive emphasis |

Disabled widgets use `text.muted` and 45% content opacity. Do not reduce critical
error or clipping indicators below 70% opacity.

## 3. Typography

Bundle Inter with the application; never depend on an operating-system font.

| Role | Family | Size | Line height |
| --- | --- | ---: | ---: |
| Product title | Inter Semibold | 20 | 26 |
| Page/section title | Inter Semibold | 15 | 20 |
| Control label | Inter Medium | 12 | 16 |
| Body/value | Inter Regular | 12 | 16 |
| Metadata/status | Inter Regular | 11 | 15 |
| Small graph label | Inter Regular | 10 | 14 |

Use fixed-width value fields and right alignment for numeric controls. Use Inter's
tabular numeral feature where the renderer supports it; fixed field geometry is
still required so values do not move while editing or metering.

## 4. Spacing and Geometry

Use a four-point grid: `4`, `8`, `12`, `16`, `24`, and `32`.

| Token | Value | Use |
| --- | ---: | --- |
| `control.compact_height` | 28 | Dense rack controls |
| `control.height` | 32 | Standard buttons and inputs |
| `control.large_height` | 44 | Primary export and transport actions |
| `icon.small` | 14 | Dense row actions |
| `icon.default` | 16 | Standard controls |
| `icon.large` | 20 | Transport and section actions |
| `radius.control` | 4 | Inputs and buttons |
| `radius.card` | 8 | Panels and cards |
| `radius.dialog` | 10 | Dialogs and floating surfaces |

Borders are one logical pixel. Selected controls may add a one-pixel accent border
but must not change their external dimensions. Shadows are limited to dialogs and
floating menus: black at 28% opacity, blur 16, vertical offset 6.

## 5. Interaction States

- Hover: move to the documented hover fill and stronger border; do not resize.
- Pressed: use the pressed fill and a one-point inward content offset.
- Selected: use the accent fill or `accent.soft` plus an accent border.
- Keyboard focus: draw a two-point `accent.focus` ring outside the widget bounds.
- Disabled: remove hover/press behavior and use muted text at 45% content opacity.
- Destructive: remain neutral until confirmation unless the action is immediately
  destructive; include a text label or recognizable icon.
- Fine adjustment: Shift + drag uses one tenth of normal sensitivity.
- Reset: double-click restores the widget's declared default value.
- Numeric entry: clicking the value opens a compact editor; Enter commits and
  Escape cancels.

Tooltips are required for icon-only actions. Keyboard focus order follows visual
order. Custom widgets must expose useful labels through egui/AccessKit.

### 5.1 Selection controls

- Segmented controls use the standard control radius only on the outside edge of
  the group. Corners touching another segment are square, and separators do not
  introduce spacing between segments.
- Checkboxes use a solid application-accent background and visible checkmark when
  checked. They may hide their visible text label in compact layouts, but must
  retain that label for tooltips and accessibility.
- Dropdown selectors may be unlabeled, labeled on the left, or labeled above.
  These arrangements share identical field and popup styling.
- List selectors are single-select, always-visible collections of flat, text-only
  rows inside one bordered container. Each row contains a key and value. Keys are
  left-aligned; every value begins after the widest rendered key plus a 12-point
  gap. Rows have no individual border, radius, or spacing. Selection changes may
  be guarded before commit so application workflows can defer a change while
  requesting confirmation.
- Icon-only buttons have either an outlined control frame or a borderless ghost
  treatment. Ghost buttons gain a visible frame on hover or press. Every
  icon-only button requires a tooltip and accessibility label.

## 6. Audio Visuals

- Waveforms use their IR identity color over `surface.inset`, with a subtle center
  line and no glow wider than two pixels.
- Waveform thumbnails and timelines use a symmetric magnitude envelope mirrored
  around the centerline. Polarity-sensitive analysis uses the signed sample trace.
  `WaveformView` defaults to signed rendering, so thumbnail callers must select
  the symmetric render mode explicitly.
- Graphs use `border.subtle` for the minor grid, `border.control` for major axes,
  and `text.secondary` for labels. Selected or combined traces are two pixels;
  other traces are one pixel.
- Meters use a fast attack, slower decay, peak hold, and an explicit clip marker.
  Meter position and labels convey state even when colors overlap IR hues.
- Animation is deterministic from a supplied time value. Use 120 ms for hover
  transitions, 80 ms for pressed transitions, 750 ms peak hold, and approximately
  18 dB/s visual decay.
- Graphs, meters, and waveforms must remain sharp at 1×, 1.5×, and 2× scale.

## 7. Rust Token Mapping

| Document namespace | Rust field/type |
| --- | --- |
| `surface.*` | `DesignSystem::colors.surface_*` |
| `border.*` | `DesignSystem::colors.border_*` |
| `text.*` | `DesignSystem::colors.text_*` |
| `accent.*` | `DesignSystem::colors.accent_*` |
| `status.*` | `DesignSystem::colors.status_*` |
| IR palette index | `IrColorId` and `IR_COLORS` |
| Spacing and sizes | `DesignSystem::metrics` |
| Typography roles | `TextRole` |

The component gallery is the executable reference for these tokens. Any deliberate
visual change must update this document, the Rust token, and the affected snapshot
in the same change.

## 8. Component Gallery Workflow

Composed cards live in `ir_ui::components`. `CardFrame` owns shared chrome and
offers body and header-action closures, primary/inset variants, optional numbered
headers, width, and minimum height. Width includes padding and borders.
`InputSourceCard`, `OutputCard`, and `ExportMixedIrCard` consume borrowed views
and return typed actions; the caller owns selections, transport, and side effects.
Give each card instance a stable ID. Cards support widths of 280 points and above
(300 by default). The Cards gallery applies these actions to independent dummy
states. In particular, choosing an output device, choosing an export destination,
and exporting are intentions only; the components never access devices or files.

Run the native Storybook-style gallery with:

```text
cargo run -p ir-ui-gallery
```

Run the component, interaction, and visual-regression tests with:

```text
cargo test -p ir-ui
```

Only update snapshot baselines after visually reviewing the change. In PowerShell:

```powershell
$env:UPDATE_SNAPSHOTS = "force"
cargo test -p ir-ui
```

The gallery is intentionally limited to isolated components in this milestone.
Application panels, the full window shell, mock backend, and DSP integration begin
only after the gallery and its design tokens are approved.

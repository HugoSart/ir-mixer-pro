//! Reusable egui design system and audio-oriented widgets for IR Mixer.
//!
//! The crate deliberately depends on egui rather than nice-plug. Host and
//! standalone adapters can therefore render exactly the same widgets.

pub mod components;
pub mod palette;
pub mod theme;
pub mod widgets;

pub use palette::{IR_COLORS, IrColorId};
pub use theme::{DesignSystem, TextRole, install_theme};

/// Install resources that every IR Mixer egui renderer needs.
pub fn install(ctx: &egui::Context) {
    install_theme(ctx);
}

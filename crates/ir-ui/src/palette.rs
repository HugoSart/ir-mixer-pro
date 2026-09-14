use egui::Color32;

/// Curated channel colors from the approved design, in assignment order.
pub const IR_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x0E, 0x7E, 0xE9),
    Color32::from_rgb(0x98, 0x45, 0xE7),
    Color32::from_rgb(0x5B, 0xE2, 0x45),
    Color32::from_rgb(0xE9, 0xD7, 0x1A),
    Color32::from_rgb(0xEC, 0x91, 0x34),
    Color32::from_rgb(0xEC, 0x5D, 0x6A),
    Color32::from_rgb(0x22, 0xC7, 0xDD),
    Color32::from_rgb(0xD5, 0x68, 0xC4),
];

/// Stable, serializable-ready index into [`IR_COLORS`].
///
/// The application model should persist this index instead of the raw color.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub struct IrColorId(u8);

impl IrColorId {
    pub const COUNT: usize = IR_COLORS.len();

    pub const fn new(index: usize) -> Self {
        Self((index % Self::COUNT) as u8)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn color(self) -> Color32 {
        IR_COLORS[self.index()]
    }

    pub const fn next(self) -> Self {
        Self::new(self.index() + 1)
    }
}

impl From<usize> for IrColorId {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl From<IrColorId> for u8 {
    fn from(value: IrColorId) -> Self {
        value.0
    }
}

impl TryFrom<u8> for IrColorId {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (value as usize) < Self::COUNT {
            Ok(Self(value))
        } else {
            Err("IR color index must be between 0 and 7")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_cycles_after_eight_entries() {
        assert_eq!(IrColorId::new(0), IrColorId::new(8));
        assert_eq!(IrColorId::new(7).next(), IrColorId::new(0));
    }

    #[test]
    fn persisted_indices_are_validated() {
        assert!(IrColorId::try_from(7_u8).is_ok());
        assert!(IrColorId::try_from(8_u8).is_err());
    }
}

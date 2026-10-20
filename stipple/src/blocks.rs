//! Block glyphs that split a cell into two colours.

/// How much picture detail the background layer carries inside each cell.
/// Finer detail needs block characters the terminal can draw.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Detail {
    /// One colour per cell.
    Flat,
    /// Two colours, split into top and bottom halves (`▀`). Every terminal
    /// draws these.
    Halves,
    /// Two colours over a 2×2 grid (`▘`, `▚`, `▙`, …). Nearly every terminal
    /// draws these.
    #[default]
    Quadrants,
    /// Two colours over the full 2×4 dot grid, using Unicode 16 octants.
    /// Needs a terminal that draws them itself, such as Ghostty, or a font
    /// that has them.
    Octants,
}

impl Detail {
    /// The cell's dots (indexed `row * 2 + column`) grouped into the blocks
    /// this detail can colour separately.
    pub(crate) const fn groups(self) -> &'static [&'static [usize]] {
        match self {
            Self::Flat => &[&[0, 1, 2, 3, 4, 5, 6, 7]],
            Self::Halves => &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
            Self::Quadrants => &[&[0, 2], &[1, 3], &[4, 6], &[5, 7]],
            Self::Octants => &[&[0], &[1], &[2], &[3], &[4], &[5], &[6], &[7]],
        }
    }
}

/// The glyph that fills exactly the dots set in `bits` (bit `row * 2 +
/// column`) with the foreground colour. Patterns made of halves and
/// quadrants map to the long-standing block elements; the rest are octants.
pub(crate) const BLOCKS: [&str; 256] = [
    " ", "𜺨", "𜺫", "🮂", "𜴀", "▘", "𜴁", "𜴂", "𜴃", "𜴄", "▝", "𜴅", "𜴆", "𜴇", "𜴈", "▀", "𜴉", "𜴊", "𜴋", "𜴌", "🯦", "𜴍", "𜴎",
    "𜴏", "𜴐", "𜴑", "𜴒", "𜴓", "𜴔", "𜴕", "𜴖", "𜴗", "𜴘", "𜴙", "𜴚", "𜴛", "𜴜", "𜴝", "𜴞", "𜴟", "🯧", "𜴠", "𜴡", "𜴢", "𜴣", "𜴤",
    "𜴥", "𜴦", "𜴧", "𜴨", "𜴩", "𜴪", "𜴫", "𜴬", "𜴭", "𜴮", "𜴯", "𜴰", "𜴱", "𜴲", "𜴳", "𜴴", "𜴵", "🮅", "𜺣", "𜴶", "𜴷", "𜴸", "𜴹",
    "𜴺", "𜴻", "𜴼", "𜴽", "𜴾", "𜴿", "𜵀", "𜵁", "𜵂", "𜵃", "𜵄", "▖", "𜵅", "𜵆", "𜵇", "𜵈", "▌", "𜵉", "𜵊", "𜵋", "𜵌", "▞", "𜵍",
    "𜵎", "𜵏", "𜵐", "▛", "𜵑", "𜵒", "𜵓", "𜵔", "𜵕", "𜵖", "𜵗", "𜵘", "𜵙", "𜵚", "𜵛", "𜵜", "𜵝", "𜵞", "𜵟", "𜵠", "𜵡", "𜵢", "𜵣",
    "𜵤", "𜵥", "𜵦", "𜵧", "𜵨", "𜵩", "𜵪", "𜵫", "𜵬", "𜵭", "𜵮", "𜵯", "𜵰", "𜺠", "𜵱", "𜵲", "𜵳", "𜵴", "𜵵", "𜵶", "𜵷", "𜵸", "𜵹",
    "𜵺", "𜵻", "𜵼", "𜵽", "𜵾", "𜵿", "𜶀", "𜶁", "𜶂", "𜶃", "𜶄", "𜶅", "𜶆", "𜶇", "𜶈", "𜶉", "𜶊", "𜶋", "𜶌", "𜶍", "𜶎", "𜶏", "▗",
    "𜶐", "𜶑", "𜶒", "𜶓", "▚", "𜶔", "𜶕", "𜶖", "𜶗", "▐", "𜶘", "𜶙", "𜶚", "𜶛", "▜", "𜶜", "𜶝", "𜶞", "𜶟", "𜶠", "𜶡", "𜶢", "𜶣",
    "𜶤", "𜶥", "𜶦", "𜶧", "𜶨", "𜶩", "𜶪", "𜶫", "▂", "𜶬", "𜶭", "𜶮", "𜶯", "𜶰", "𜶱", "𜶲", "𜶳", "𜶴", "𜶵", "𜶶", "𜶷", "𜶸", "𜶹",
    "𜶺", "𜶻", "𜶼", "𜶽", "𜶾", "𜶿", "𜷀", "𜷁", "𜷂", "𜷃", "𜷄", "𜷅", "𜷆", "𜷇", "𜷈", "𜷉", "𜷊", "𜷋", "𜷌", "𜷍", "𜷎", "𜷏", "𜷐",
    "𜷑", "𜷒", "𜷓", "𜷔", "𜷕", "𜷖", "𜷗", "𜷘", "𜷙", "𜷚", "▄", "𜷛", "𜷜", "𜷝", "𜷞", "▙", "𜷟", "𜷠", "𜷡", "𜷢", "▟", "𜷣", "▆",
    "𜷤", "𜷥", "█",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_patterns_use_classic_blocks() {
        assert_eq!(BLOCKS[0], " ");
        assert_eq!(BLOCKS[0b1111], "▀");
        assert_eq!(BLOCKS[0b1111_0000], "▄");
        assert_eq!(BLOCKS[0b0101_0101], "▌");
        assert_eq!(BLOCKS[0b0000_0101], "▘");
        assert_eq!(BLOCKS[0b1010_0101], "▚");
        assert_eq!(BLOCKS[255], "█");
    }

    #[test]
    fn every_pattern_has_its_own_glyph() {
        let mut seen = std::collections::HashSet::new();
        assert!(BLOCKS.iter().all(|g| g.chars().count() == 1 && seen.insert(*g)));
    }
}

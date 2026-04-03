use ratatui::style::Color;

pub const BG: Color = Color::Rgb(0x13, 0x15, 0x1c);
#[allow(unused)]
pub const BG_SURFACE: Color = Color::Rgb(0x1d, 0x20, 0x29);
pub const FG: Color = Color::Rgb(0xcd, 0xc8, 0xbc);
pub const FG_MUTED: Color = Color::Rgb(0x81, 0x7c, 0x72);
pub const ACCENT: Color = Color::Rgb(0xc9, 0x94, 0x3e);
pub const BORDER: Color = Color::Rgb(0x28, 0x2c, 0x38);
pub const SUCCESS: Color = Color::Rgb(0x7a, 0xb8, 0x7f);
pub const WARNING: Color = Color::Rgb(0xd4, 0xa8, 0x43);
pub const CRITICAL: Color = Color::Rgb(0xc4, 0x5c, 0x5c);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ember_palette_values_match_spec() {
        assert_eq!(BG, Color::Rgb(0x13, 0x15, 0x1c));
        assert_eq!(BG_SURFACE, Color::Rgb(0x1d, 0x20, 0x29));
        assert_eq!(FG, Color::Rgb(0xcd, 0xc8, 0xbc));
        assert_eq!(FG_MUTED, Color::Rgb(0x81, 0x7c, 0x72));
        assert_eq!(ACCENT, Color::Rgb(0xc9, 0x94, 0x3e));
        assert_eq!(BORDER, Color::Rgb(0x28, 0x2c, 0x38));
        assert_eq!(SUCCESS, Color::Rgb(0x7a, 0xb8, 0x7f));
        assert_eq!(WARNING, Color::Rgb(0xd4, 0xa8, 0x43));
        assert_eq!(CRITICAL, Color::Rgb(0xc4, 0x5c, 0x5c));
    }
}

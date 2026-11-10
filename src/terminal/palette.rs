//! The terminal's colours, one set for the light and one for the dark appearance.

use ide_terminal::{Palette, Rgb};

/// The colours a terminal view draws with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// What the session resolves cell colours from.
    pub palette: Palette,
    /// Behind selected text.
    pub selection: Rgb,
}

const fn hex(rgb: u32) -> Rgb {
    let [_, r, g, b] = rgb.to_be_bytes();
    Rgb::new(r, g, b)
}

const DARK_ANSI: [u32; 16] = [
    0x3a3a3c, 0xf0716b, 0x8bcb6e, 0xe3c26a, 0x6aa7f5, 0xc98be6, 0x5cc6cf, 0xcfcfd2, 0x7f7f85, 0xff8e88, 0xa6e08c,
    0xf5d98a, 0x8dbeff, 0xdda6f5, 0x7ddce3, 0xffffff,
];

/// Darker than the usual terminal colours, so every one of them stays readable on white.
const LIGHT_ANSI: [u32; 16] = [
    0x1d1d1f, 0xc5302b, 0x2e7d32, 0x8f6a00, 0x1e5bc6, 0x9339b0, 0x0b7a86, 0xa9a9ae, 0x6e6e73, 0xe0463f, 0x3b9a45,
    0xb08400, 0x3478f6, 0xb157d1, 0x1597a6, 0xd5d5da,
];

/// The colours for the dark or the light appearance; `high_contrast` follows the system's Increase Contrast.
pub fn theme(dark: bool, high_contrast: bool) -> Theme {
    let (foreground, background, ansi, selection) = match (dark, high_contrast) {
        (true, false) => (0xd6d6d8, 0x1e1e1e, DARK_ANSI, 0x3b5277),
        (true, true) => (0xffffff, 0x000000, DARK_ANSI, 0x2f5fa8),
        (false, false) => (0x1d1d1f, 0xffffff, LIGHT_ANSI, 0xb3d4fc),
        (false, true) => (0x000000, 0xffffff, LIGHT_ANSI, 0x8fbcf5),
    };
    Theme {
        palette: Palette {
            foreground: hex(foreground),
            background: hex(background),
            cursor: None,
            ansi: ansi.map(hex),
            bold_is_bright: true,
        },
        selection: hex(selection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contrast ratio of two colours as WCAG defines it.
    fn contrast(a: Rgb, b: Rgb) -> f64 {
        let luminance = |c: Rgb| {
            let channel = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.039_28 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
        };
        let (x, y) = (luminance(a) + 0.05, luminance(b) + 0.05);
        x.max(y) / x.min(y)
    }

    #[test]
    fn text_and_the_coloured_text_programs_print_stay_readable() {
        for dark in [false, true] {
            let theme = theme(dark, false);
            let palette = &theme.palette;
            assert!(contrast(palette.foreground, palette.background) > 7.0, "WCAG AAA");
            // Red, green, yellow, blue, magenta and cyan in both forms; black and white are meant for one appearance.
            for index in (1..7).chain(9..15) {
                let ratio = contrast(palette.ansi[index], palette.background);
                assert!(ratio > 3.0, "colour {index} in dark={dark}: {ratio:.2}");
            }
            assert!(contrast(palette.foreground, theme.selection) > 4.5);
        }
    }
}

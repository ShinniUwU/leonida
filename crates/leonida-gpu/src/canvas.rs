//! Pictures the F1 menu and the F3 panel are drawn as: a box of one colour
//! with filled rectangles and text on it, in Leonida's colours and a font of
//! its own (5 by 7 dots a glyph, upper case only: lower case is drawn as
//! upper). A picture is a list of operations, cheap to make and compare at
//! every frame; it is turned into pixels only when it changes
//! (`crate::overlay`).

/// A colour as sRGB bytes.
pub(crate) type Rgb = [u8; 3];

const fn hex(v: u32) -> Rgb {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// The dark panel, rgba(13, 9, 22, 0.86), as it shows over black.
// shortcut: drawn opaque. The box is copied onto the frame, and a copy
// cannot blend; over a bright frame it is darker than the design's.
pub(crate) const PANEL: Rgb = [11, 8, 19];
/// Text.
pub(crate) const PAPER: Rgb = hex(0xf5f0e8);
/// Text that says less: paper at about half over the panel.
pub(crate) const DIM: Rgb = [128, 124, 125];
/// The selected row, the frame and the name.
pub(crate) const FLAMINGO: Rgb = hex(0xc8306a);
/// The selected tab.
pub(crate) const NEON: Rgb = hex(0xff4f9a);
/// A switch that is on.
pub(crate) const TEAL: Rgb = hex(0x5ff2d6);
/// Warnings.
pub(crate) const AMBER: Rgb = hex(0xffb547);

/// Dots a glyph takes across, with the gap after it.
pub(crate) const ADVANCE: u32 = 6;
/// Dots a glyph takes down.
pub(crate) const GLYPH_HEIGHT: u32 = 7;

/// One thing drawn, in pixels from the picture's top left corner.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Op {
    Fill {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        rgb: Rgb,
    },
    /// Text with its top left corner at `x`, `y`, `scale` pixels a dot.
    Text {
        x: u32,
        y: u32,
        scale: u32,
        rgb: Rgb,
        text: String,
    },
}

/// A box of `width` by `height` pixels of `background` with `ops` drawn on
/// it in order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Picture {
    pub width: u32,
    pub height: u32,
    pub background: Rgb,
    pub ops: Vec<Op>,
}

impl Picture {
    pub(crate) fn new(width: u32, height: u32, background: Rgb) -> Picture {
        Picture {
            width,
            height,
            background,
            ops: Vec::new(),
        }
    }

    pub(crate) fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: Rgb) {
        self.ops.push(Op::Fill { x, y, w, h, rgb });
    }

    pub(crate) fn text(&mut self, x: u32, y: u32, scale: u32, rgb: Rgb, text: &str) {
        self.ops.push(Op::Text {
            x,
            y,
            scale,
            rgb,
            text: text.to_owned(),
        });
    }

    /// The pixels, row by row from the top. What lies outside the box is cut.
    pub(crate) fn render(&self) -> Vec<Rgb> {
        let (width, height) = (self.width as usize, self.height as usize);
        let mut out = vec![self.background; width * height];
        let mut fill = |x: u32, y: u32, w: u32, h: u32, rgb: Rgb| {
            let (x0, y0) = (x as usize, y as usize);
            let x1 = (x0 + w as usize).min(width);
            let y1 = (y0 + h as usize).min(height);
            for row in y0.min(y1)..y1 {
                out[row * width + x0.min(x1)..row * width + x1].fill(rgb);
            }
        };
        for op in &self.ops {
            match op {
                &Op::Fill { x, y, w, h, rgb } => fill(x, y, w, h, rgb),
                Op::Text {
                    x,
                    y,
                    scale,
                    rgb,
                    text,
                } => {
                    for (i, c) in text.chars().enumerate() {
                        let left = x + i as u32 * ADVANCE * scale;
                        for (row, bits) in glyph(c).into_iter().enumerate() {
                            for column in 0..5u32 {
                                if bits >> (4 - column) & 1 != 0 {
                                    let top = y + row as u32 * scale;
                                    fill(left + column * scale, top, *scale, *scale, *rgb);
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// Pixels `text` takes across at `scale` pixels a dot.
pub(crate) fn text_width(text: &str, scale: u32) -> u32 {
    match text.chars().count() as u32 {
        0 => 0,
        n => (n * ADVANCE - 1) * scale,
    }
}

/// `text` in lines of at most `columns` characters, broken at spaces (a word
/// longer than a line is cut).
pub(crate) fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let word: String = word.chars().take(columns).collect();
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > columns {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A glyph's seven rows, five dots each (bit 4 is the left one). Lower case
/// is drawn as upper case; a character the font lacks as a question mark.
pub(crate) fn glyph(c: char) -> [u8; 7] {
    known(c.to_ascii_uppercase()).unwrap_or_else(|| known('?').unwrap_or_default())
}

fn known(c: char) -> Option<[u8; 7]> {
    Some(match c {
        ' ' => [0; 7],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        ';' => [0, 0b01100, 0b01100, 0, 0b01100, 0b00100, 0b01000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '/' => [0, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0],
        '\\' => [0, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '%' => [
            0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '#' => [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
        '*' => [0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0],
        '\'' => [0b00100, 0b00100, 0b01000, 0, 0, 0, 0],
        '"' => [0b01010, 0b01010, 0, 0, 0, 0, 0],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_the_menu_shows_has_a_glyph() {
        let mut texts: Vec<&str> = Vec::new();
        for s in leonida_core::switches::SWITCHES {
            texts.extend([s.label, s.help]);
            texts.extend(s.values.iter().map(|v| v.label));
        }
        for g in leonida_core::switches::Group::ALL {
            texts.push(g.label());
        }
        for text in texts {
            for c in text.chars() {
                assert!(known(c.to_ascii_uppercase()).is_some(), "{c:?} in {text:?}");
            }
        }
        // Every glyph but the space lights a dot; lower case is upper case.
        for c in ('A'..='Z')
            .chain('0'..='9')
            .chain(".,:;!?-+=_/\\()[]<>%&#*'\"".chars())
        {
            assert!(glyph(c).iter().any(|&r| r != 0), "{c}");
        }
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('\u{e9}'), glyph('?'));
    }

    #[test]
    fn text_and_fills_land_where_asked_and_are_cut_at_the_edge() {
        let mut p = Picture::new(40, 20, PANEL);
        p.fill(30, 15, 50, 50, FLAMINGO);
        p.text(1, 1, 2, PAPER, "I");
        let px = p.render();
        assert_eq!(px.len(), 40 * 20);
        let at = |x: usize, y: usize| px[y * 40 + x];
        assert_eq!(at(0, 0), PANEL);
        assert_eq!(at(39, 19), FLAMINGO);
        assert_eq!(at(29, 19), PANEL);
        // "I": its top row is dots 1 to 3, two pixels each, from x = 1.
        assert_eq!(at(1 + 2, 1), PAPER);
        assert_eq!(at(1, 1), PANEL);
        assert_eq!(at(1 + 2, 1 + 2), PANEL, "the stem only, row 1");
        assert_eq!(at(1 + 4, 1 + 2), PAPER);
        // A picture with text past its edge does not panic.
        let mut q = Picture::new(4, 4, PANEL);
        q.text(2, 2, 3, PAPER, "WWW");
        assert_eq!(q.render().len(), 16);
    }

    #[test]
    fn widths_and_wrapping() {
        assert_eq!(text_width("", 3), 0);
        assert_eq!(text_width("AB", 2), (2 * ADVANCE - 1) * 2);
        assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
        assert_eq!(wrap("  ", 9), Vec::<String>::new());
        assert_eq!(wrap("abcdefghijk", 4), ["abcd"]);
    }
}

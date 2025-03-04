use ratatui::style;

#[derive(Debug, Copy, Clone)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

type Hsv = (f32, f32, f32);

impl Colour {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Colour { r, g, b }
    }

    pub fn desaturate(&self, level: f32) -> Self {
        let (h, s, v) = self.to_hsv();
        Colour::from_hsv((h, s * (1. - level), v))
    }

    fn to_hsv(self) -> Hsv {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let h = if hue < 0.0 { hue + 360.0 } else { hue };
        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }

    fn from_hsv(hsv: Hsv) -> Colour {
        let h = hsv.0;
        let s = hsv.1;
        let v = hsv.2;

        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Colour::new(
            ((r + m) * 255.0).round() as u8,
            ((g + m) * 255.0).round() as u8,
            ((b + m) * 255.0).round() as u8,
        )
    }
}

impl From<Colour> for style::Color {
    fn from(colour: Colour) -> style::Color {
        style::Color::Rgb(colour.r, colour.g, colour.b)
    }
}

impl TryFrom<style::Color> for Colour {
    type Error = ();
    fn try_from(color: style::Color) -> Result<Self, ()> {
        use style::Color::*;
        let c = Colour::new;
        const F: u8 = 255;
        const M: u8 = 170;
        const L: u8 = 85;
        Ok(match color {
            Reset => return Err(()),
            Indexed(_) => return Err(()),
            Rgb(r, g, b) => Colour::new(r, g, b),
            Black => c(0, 0, 0),
            Red => c(M, 0, 0),
            Green => c(0, M, 0),
            Yellow => c(M, L, 0),
            Blue => c(0, 0, M),
            Magenta => c(M, 0, M),
            Cyan => c(0, M, M),
            White => c(F, F, F),
            Gray => c(M, M, M),
            DarkGray => c(L, L, L),
            LightRed => c(F, L, L),
            LightGreen => c(L, F, L),
            LightYellow => c(F, F, L),
            LightBlue => c(L, L, F),
            LightMagenta => c(F, L, F),
            LightCyan => c(L, F, F),
        })
    }
}

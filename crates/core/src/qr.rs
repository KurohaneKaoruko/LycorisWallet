use crate::error::{Error, Result};
use qrcode::{EcLevel, QrCode};

fn encode(text: &str) -> Result<QrCode> {
    if text.is_empty() {
        return Err(Error::InvalidAddress("nothing to encode".into()));
    }
    QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| Error::Crypto(format!("qr encode failed: {e}")))
}

/// Geometry of the rendered symbol, in SVG user units.
const QUIET: usize = 2;
const SCALE: usize = 8;
/// Data modules are drawn slightly smaller than their cell so they read as
/// separate tiles instead of a solid block.
const DATA_FILL: f64 = 0.88;

struct Painter {
    width: usize,
    /// `true` marks modules that belong to a finder or alignment pattern and
    /// are drawn as a custom shape instead of individual tiles.
    special: Vec<bool>,
    body: String,
}

impl Painter {
    fn at(&self, x: usize, y: usize) -> bool {
        self.special[y * self.width + x]
    }

    fn mark(&mut self, x: usize, y: usize, size: usize) {
        for dy in 0..size {
            for dx in 0..size {
                let px = x + dx;
                let py = y + dy;
                if px < self.width && py < self.width {
                    self.special[py * self.width + px] = true;
                }
            }
        }
    }

    /// Top-left corner of a module cell in SVG coordinates.
    fn cell(&self, x: usize, y: usize) -> (f64, f64) {
        (
            ((x + QUIET) * SCALE) as f64,
            ((y + QUIET) * SCALE) as f64,
        )
    }

    /// A filled rounded square covering `size` modules.
    fn block(&mut self, x: usize, y: usize, size: usize, radius: f64) {
        let (px, py) = self.cell(x, y);
        let extent = (size * SCALE) as f64;
        self.body.push_str(&format!(
            "<rect x=\"{px}\" y=\"{py}\" width=\"{extent}\" height=\"{extent}\" rx=\"{radius}\"/>"
        ));
    }

    /// A hollow rounded square: one module thick, drawn as a stroked rect so
    /// the corners round the way the finder patterns look.
    fn ring(&mut self, x: usize, y: usize, size: usize, radius: f64, stroke: &str) {
        let (px, py) = self.cell(x, y);
        let half = SCALE as f64 / 2.0;
        let extent = (size * SCALE) as f64 - SCALE as f64;
        self.body.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{extent}\" height=\"{extent}\" rx=\"{}\" \
             fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{SCALE}\"/>",
            px + half,
            py + half,
            radius.max(0.0) - half
        ));
    }
}

/// True when the 5×5 block at (ox, oy) is an alignment pattern
/// (dark ring, light gap, dark centre).
fn is_alignment(grid: &[bool], width: usize, ox: usize, oy: usize) -> bool {
    let dark = |x: usize, y: usize| grid[y * width + x];
    for i in 0..5 {
        if !dark(ox + i, oy) || !dark(ox + i, oy + 4) {
            return false;
        }
    }
    for i in 1..4 {
        if !dark(ox, oy + i) || !dark(ox + 4, oy + i) {
            return false;
        }
    }
    for i in 1..4 {
        for j in 1..4 {
            if (i == 2 && j == 2) == dark(ox + i, oy + j) {
                return false;
            }
        }
    }
    true
}

/// Render a QR code as an inline SVG string.
///
/// The three finder patterns (and the alignment patterns) are drawn as one
/// connected rounded shape, the data area as separate rounded tiles. The
/// symbol still encodes exactly the same payload — `tools/verify_qr_svg.py`
/// rasterises this SVG and decodes it with an unrelated scanner. Colours are
/// baked in so the code can follow the light or the dark theme.
pub fn svg(text: &str, dark: &str, light: &str) -> Result<String> {
    let code = encode(text)?;
    let width = code.width();
    let colors = code.to_colors();

    let mut grid = vec![false; width * width];
    for y in 0..width {
        for x in 0..width {
            grid[y * width + x] = colors[y * width + x] == qrcode::Color::Dark;
        }
    }

    let mut painter = Painter {
        width,
        special: vec![false; width * width],
        body: String::new(),
    };

    // The three finder patterns, drawn as a rounded ring plus a solid core.
    let finders = [
        (0usize, 0usize),
        (0usize, width - 7),
        (width - 7, 0usize),
    ];
    for (ox, oy) in finders {
        painter.mark(ox, oy, 7);
        painter.ring(ox, oy, 7, 2.6 * SCALE as f64, dark);
        painter.block(ox + 2, oy + 2, 3, 0.9 * SCALE as f64);
    }

    // Alignment patterns: 5×5 rings anywhere the pattern shows up.
    for oy in 0..=width.saturating_sub(5) {
        for ox in 0..=width.saturating_sub(5) {
            if finders.iter().any(|(fx, fy)| {
                let (fx, fy) = (*fx, *fy);
                // skip the finder plus its separator
                ox + 5 > fx.saturating_sub(1)
                    && ox < fx + 8
                    && oy + 5 > fy.saturating_sub(1)
                    && oy < fy + 8
            }) {
                continue;
            }
            if !is_alignment(&grid, width, ox, oy) {
                continue;
            }
            painter.mark(ox, oy, 5);
            painter.ring(ox, oy, 5, 2.0 * SCALE as f64, dark);
            painter.block(ox + 2, oy + 2, 1, 0.3 * SCALE as f64);
        }
    }

    // Data area: one rounded tile per dark module.
    let tile = (SCALE as f64 * DATA_FILL).round();
    let offset = ((SCALE as f64 - tile) / 2.0).round();
    let radius = (tile * 0.3).round();
    for y in 0..width {
        for x in 0..width {
            if !grid[y * width + x] || painter.at(x, y) {
                continue;
            }
            let (px, py) = painter.cell(x, y);
            painter.body.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{tile}\" height=\"{tile}\" rx=\"{radius}\"/>",
                px + offset,
                py + offset
            ));
        }
    }

    let extent = ((width + QUIET * 2) * SCALE) as u32;
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {extent} {extent}\" shape-rendering=\"geometricPrecision\">\
         <rect width=\"{extent}\" height=\"{extent}\" fill=\"{light}\"/>\
         <g fill=\"{dark}\">{}</g></svg>",
        painter.body
    ))
}

/// The raw module matrix as `(width, modules)` with `true` meaning dark,
/// quiet zone excluded. Used by `examples/qr_fixtures.rs` so an independent
/// scanner can verify what the app actually encodes.
pub fn matrix(text: &str) -> Result<(usize, Vec<bool>)> {
    let code = encode(text)?;
    let width = code.width();
    let modules = code
        .to_colors()
        .into_iter()
        .map(|color| color == qrcode::Color::Dark)
        .collect();
    Ok((width, modules))
}

#[cfg(test)]
mod tests {
    #[test]
    fn qr_contains_payload() {
        let svg = super::svg("bc1qexample", "#000", "#fff").unwrap();
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<rect"));
        assert!(svg.contains("rx="));
        assert!(svg.contains("fill=\"#000\""));
        assert!(svg.contains("fill=\"#fff\""));
    }

    #[test]
    fn finder_patterns_are_drawn_as_rings() {
        let svg = super::svg("bc1qexample", "#000", "#fff").unwrap();
        // Three finder rings (stroked rects) plus their solid cores.
        assert_eq!(svg.matches("stroke-width").count(), 3);
    }

    #[test]
    fn qr_matrix_is_square() {
        let (width, modules) = super::matrix("bc1qexample").unwrap();
        assert_eq!(width * width, modules.len());
        assert!(modules.iter().any(|dark| *dark));
    }

    #[test]
    fn qr_rejects_empty_payload() {
        assert!(super::svg("", "#000", "#fff").is_err());
        assert!(super::matrix("").is_err());
    }
}

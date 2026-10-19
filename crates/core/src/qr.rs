use crate::error::{Error, Result};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

fn encode(text: &str) -> Result<QrCode> {
    if text.is_empty() {
        return Err(Error::InvalidAddress("nothing to encode".into()));
    }
    QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| Error::Crypto(format!("qr encode failed: {e}")))
}

/// Render a QR code as an inline SVG string. The palette matches the app's
/// paper-and-ink theme and can be overridden by CSS via currentColor? SVG
/// colours are baked in for reliable screenshots, so callers pass explicit
/// colours.
pub fn svg(text: &str, dark: &str, light: &str) -> Result<String> {
    let image = encode(text)?
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .dark_color(svg::Color(dark))
        .light_color(svg::Color(light))
        .build();
    Ok(image)
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
        assert!(svg.contains("<path"));
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

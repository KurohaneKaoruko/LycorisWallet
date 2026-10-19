use crate::error::{Error, Result};

/// Parse a human decimal string into the smallest unit (satoshi, wei, lamport).
pub fn parse_units(input: &str, decimals: u8) -> Result<u128> {
    let value = input.trim();
    if value.is_empty() {
        return Err(Error::InvalidAmount("amount is empty".into()));
    }
    if value.starts_with('-') || value.starts_with('+') {
        return Err(Error::InvalidAmount("amount must be positive".into()));
    }
    if value.contains(['e', 'E']) {
        return Err(Error::InvalidAmount(
            "scientific notation is not supported".into(),
        ));
    }
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(Error::InvalidAmount(
            "amount has too many decimal points".into(),
        ));
    }
    if integer.is_empty() && fraction.is_empty() {
        return Err(Error::InvalidAmount("amount is empty".into()));
    }
    if !integer.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return Err(Error::InvalidAmount(
            "amount contains invalid characters".into(),
        ));
    }
    if fraction.len() > decimals as usize {
        return Err(Error::InvalidAmount(format!(
            "at most {decimals} decimal places are supported"
        )));
    }
    let integer_value: u128 = if integer.is_empty() {
        0
    } else {
        integer
            .parse()
            .map_err(|_| Error::InvalidAmount("amount is too large".into()))?
    };
    let mut fraction_value: u128 = 0;
    if !fraction.is_empty() {
        let padded = format!("{fraction:0<width$}", width = decimals as usize);
        fraction_value = padded
            .parse()
            .map_err(|_| Error::InvalidAmount("amount is too large".into()))?;
    }
    let scale = 10u128
        .checked_pow(decimals as u32)
        .ok_or_else(|| Error::InvalidAmount("unsupported decimals".into()))?;
    integer_value
        .checked_mul(scale)
        .and_then(|v| v.checked_add(fraction_value))
        .ok_or_else(|| Error::InvalidAmount("amount is too large".into()))
}

/// Format a smallest-unit integer as a trimmed decimal string.
pub fn format_units(value: u128, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let scale = 10u128.pow(decimals as u32);
    let integer = value / scale;
    let fraction = value % scale;
    let mut fraction = format!("{fraction:0>width$}", width = decimals as usize);
    while fraction.ends_with('0') {
        fraction.pop();
    }
    if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    }
}

/// Format a decimal string with a maximum number of fractional digits,
/// stripping trailing zeros.
pub fn truncate_decimal(value: &str, max_fraction: usize) -> String {
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or("0");
    let fraction = parts.next().unwrap_or("");
    let trimmed = if fraction.len() > max_fraction {
        &fraction[..max_fraction]
    } else {
        fraction
    };
    let trimmed = trimmed.trim_end_matches('0');
    if trimmed.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{trimmed}")
    }
}

/// Convert a decimal string to f64 for display purposes only.
pub fn to_f64(value: &str) -> Option<f64> {
    value.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        assert_eq!(parse_units("1.5", 8).unwrap(), 150_000_000);
        assert_eq!(parse_units("0.00000001", 8).unwrap(), 1);
        assert_eq!(parse_units("12", 18).unwrap(), 12 * 10u128.pow(18));
        assert!(parse_units("1.123456789", 8).is_err());
        assert_eq!(format_units(150_000_000, 8), "1.5");
        assert_eq!(format_units(1, 8), "0.00000001");
        assert_eq!(format_units(10u128.pow(18), 18), "1");
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate_decimal("1.23456789", 4), "1.2345");
        assert_eq!(truncate_decimal("1.2000", 4), "1.2");
        assert_eq!(truncate_decimal("2.0001", 2), "2");
    }
}

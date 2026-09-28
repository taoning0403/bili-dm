/// Parse a single HTTP byte range. Multiple ranges are deliberately unsupported.
pub fn byte_range(header: Option<&str>, size: u64) -> Result<(u64, u64, bool), &'static str> {
    let Some(value) = header else {
        return Ok((0, size, false));
    };
    let (start, end) = value
        .strip_prefix("bytes=")
        .and_then(|value| value.split_once('-'))
        .ok_or("invalid range")?;
    if size == 0 || end.contains(',') {
        return Err("unsatisfiable range");
    }
    if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| "invalid suffix")?;
        if suffix == 0 {
            return Err("zero suffix");
        }
        let length = suffix.min(size);
        return Ok((size - length, length, true));
    }
    let start: u64 = start.parse().map_err(|_| "invalid start")?;
    if start >= size {
        return Err("start beyond EOF");
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| "invalid end")?.min(size - 1)
    };
    if end < start {
        return Err("inverted range");
    }
    Ok((start, end - start + 1, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supports_mpv_ranges_and_suffixes() {
        assert_eq!(byte_range(None, 100), Ok((0, 100, false)));
        assert_eq!(byte_range(Some("bytes=40-"), 100), Ok((40, 60, true)));
        assert_eq!(byte_range(Some("bytes=-8"), 100), Ok((92, 8, true)));
        assert_eq!(byte_range(Some("bytes=1-999"), 100), Ok((1, 99, true)));
    }
    #[test]
    fn rejects_invalid_and_multi_ranges_without_overflow() {
        for value in [
            "bytes=100-",
            "bytes=4-3",
            "bytes=-0",
            "bytes=0-1,5-7",
            "bytes=18446744073709551616-",
            "items=1-2",
        ] {
            assert!(byte_range(Some(value), 100).is_err());
        }
        assert!(byte_range(Some("bytes=0-"), 0).is_err());
    }
}

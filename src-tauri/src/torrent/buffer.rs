use super::models::BufferedRange;
/// Verified file byte fractions; not an exact time map for variable bitrate media.
pub fn file_ranges(
    have: impl IntoIterator<Item = bool>,
    piece_size: u64,
    offset: u64,
    size: u64,
) -> Vec<BufferedRange> {
    let mut result: Vec<BufferedRange> = vec![];
    if size == 0 || piece_size == 0 {
        return result;
    }
    let end = offset.saturating_add(size);
    for (index, verified) in have.into_iter().enumerate() {
        if !verified {
            continue;
        }
        let piece_start = (index as u64).saturating_mul(piece_size);
        let start = piece_start.max(offset);
        let stop = piece_start.saturating_add(piece_size).min(end);
        if start >= stop {
            continue;
        }
        let range = BufferedRange {
            start: (start - offset) as f64 / size as f64,
            end: (stop - offset) as f64 / size as f64,
        };
        if let Some(last) = result
            .last_mut()
            .filter(|r| (r.end - range.start).abs() < f64::EPSILON)
        {
            last.end = range.end;
        } else {
            result.push(range);
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_shared_boundary_pieces_and_keeps_holes() {
        let ranges = file_ranges([true, true, false, true, true], 10, 15, 20);
        assert_eq!(ranges.len(), 2);
        assert_eq!((ranges[0].start, ranges[0].end), (0.0, 0.25));
        assert_eq!((ranges[1].start, ranges[1].end), (0.75, 1.0));
        assert_eq!(file_ranges([true, true, true], 10, 5, 20).len(), 1);
    }
}

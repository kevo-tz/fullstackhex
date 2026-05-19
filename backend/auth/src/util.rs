/// Constant-time equality check for two byte slices.
///
/// Returns `true` only if both slices are the same length and all bytes
/// match. Timing does not vary with the position of a mismatch.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Constant-time equality check for two string slices.
pub fn constant_time_str_eq(a: &str, b: &str) -> bool {
    constant_time_eq(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings_match() {
        assert!(constant_time_str_eq("hello", "hello"));
    }

    #[test]
    fn different_strings_dont_match() {
        assert!(!constant_time_str_eq("hello", "world"));
    }

    #[test]
    fn different_lengths_dont_match() {
        assert!(!constant_time_str_eq("short", "longer"));
    }

    #[test]
    fn empty_strings_match() {
        assert!(constant_time_str_eq("", ""));
    }

    #[test]
    fn equal_bytes_match() {
        assert!(constant_time_eq(&[1, 2, 3], &[1, 2, 3]));
    }

    #[test]
    fn different_bytes_dont_match() {
        assert!(!constant_time_eq(&[1, 2, 3], &[1, 2, 4]));
    }
}

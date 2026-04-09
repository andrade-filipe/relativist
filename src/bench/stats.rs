//! Statistical functions for benchmark aggregation (SPEC-09 R31-R34).

/// Arithmetic mean. Returns 0.0 for empty slices.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Sample standard deviation (Bessel's correction, N-1).
/// Returns 0.0 for slices with fewer than 2 elements.
pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance =
        values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Median. Returns 0.0 for empty slices.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Minimum value. Returns f64::INFINITY for empty slices.
pub fn min_f64(values: &[f64]) -> f64 {
    values
        .iter()
        .copied()
        .fold(f64::INFINITY, |a, b| if b < a { b } else { a })
}

/// Maximum value. Returns f64::NEG_INFINITY for empty slices.
pub fn max_f64(values: &[f64]) -> f64 {
    values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |a, b| if b > a { b } else { a })
}

/// Coefficient of variation: std_dev / mean.
/// Returns 0.0 if mean is zero.
pub fn coeff_of_variation(values: &[f64]) -> f64 {
    let m = mean(values);
    if m == 0.0 {
        return 0.0;
    }
    std_dev(values) / m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
    }

    #[test]
    fn test_std_dev() {
        let sd = std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert!((sd - 2.138).abs() < 0.001, "std_dev = {sd}, expected ~2.138");
    }

    #[test]
    fn test_median_odd() {
        assert_eq!(median(&[1.0, 3.0, 5.0]), 3.0);
    }

    #[test]
    fn test_median_even() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_std_dev_single() {
        assert_eq!(std_dev(&[42.0]), 0.0);
    }

    #[test]
    fn test_cv_no_variance() {
        assert_eq!(coeff_of_variation(&[10.0, 10.0, 10.0]), 0.0);
    }

    #[test]
    fn test_cv_empty() {
        assert_eq!(coeff_of_variation(&[]), 0.0);
    }

    #[test]
    fn test_min_max() {
        let v = &[3.0, 1.0, 4.0, 1.5, 9.0, 2.6];
        assert_eq!(min_f64(v), 1.0);
        assert_eq!(max_f64(v), 9.0);
    }

    #[test]
    fn test_min_max_empty() {
        assert_eq!(min_f64(&[]), f64::INFINITY);
        assert_eq!(max_f64(&[]), f64::NEG_INFINITY);
    }

    #[test]
    fn test_median_unsorted() {
        assert_eq!(median(&[5.0, 1.0, 3.0]), 3.0);
    }

    #[test]
    fn test_std_dev_all_same() {
        assert_eq!(std_dev(&[7.0, 7.0, 7.0]), 0.0);
    }
}

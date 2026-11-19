//! 측정값 묶음을 요약한다.

/// 지연 측정의 요약. 시간은 ns 다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Summary {
    /// 값을 얻은 횟수.
    pub count: usize,
    /// 시간 안에 결과가 없던 횟수.
    pub missed: usize,
    pub min: u64,
    pub median: u64,
    pub p90: u64,
    pub max: u64,
}

/// 정렬된 값에서 `percent` 번째 백분위수(가장 가까운 순위).
fn rank(sorted: &[u64], percent: usize) -> u64 {
    let position = sorted
        .len()
        .saturating_mul(percent)
        .div_ceil(100)
        .saturating_sub(1);
    sorted.get(position).copied().unwrap_or(0)
}

/// 놓친 것(`None`)은 세기만 하고 통계에서 뺀다. 값이 하나도 없으면 `None`.
#[must_use]
pub fn summarize(samples: &[Option<u64>]) -> Option<Summary> {
    let mut values: Vec<u64> = samples.iter().copied().flatten().collect();
    values.sort_unstable();
    let (min, max) = (*values.first()?, *values.last()?);
    Some(Summary {
        count: values.len(),
        missed: samples.len().saturating_sub(values.len()),
        min,
        median: rank(&values, 50),
        p90: rank(&values, 90),
        max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_values_give_nearest_rank_percentiles() {
        let samples: Vec<Option<u64>> = [70, 10, 30, 100, 20, 90, 40, 60, 50, 80]
            .into_iter()
            .map(Some)
            .collect();
        assert_eq!(
            summarize(&samples),
            Some(Summary {
                count: 10,
                missed: 0,
                min: 10,
                median: 50,
                p90: 90,
                max: 100
            })
        );
    }

    #[test]
    fn missed_samples_are_counted_and_left_out() {
        let summary = summarize(&[Some(5), None, Some(1), None, Some(3)]).unwrap();
        assert_eq!((summary.count, summary.missed), (3, 2));
        assert_eq!((summary.min, summary.median, summary.max), (1, 3, 5));
    }

    #[test]
    fn one_value_is_every_statistic_and_none_is_no_summary() {
        let summary = summarize(&[Some(7)]).unwrap();
        assert_eq!(
            (summary.min, summary.median, summary.p90, summary.max),
            (7, 7, 7, 7)
        );
        assert_eq!(summarize(&[None, None]), None);
        assert_eq!(summarize(&[]), None);
    }
}

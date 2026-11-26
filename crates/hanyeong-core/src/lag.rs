//! 실제로 누른 한영 키가 hanyeong 의 탭에 닿기까지 기다린 시간.
//!
//! 기기가 바쁠 때 전환이 가끔 늦다는 보고(2026-11-26)를 실제 사용 중에 숫자로 잡으려는 것이다.
//! 키가 만들어진 시각과 콜백이 불린 시각의 차이라, 시스템(WindowServer, 앞선 탭)과 hanyeong 이
//! CPU 를 받기까지의 대기가 들어 있다. 전환 자체(시스템이 입력 소스를 바꾸는 일)는 들어 있지 않다.
//!
//! 만든 시각(`CGEventGetTimestamp`)은 나노초라고 문서에 있지만, 실기기의 합성 이벤트에서는 mach
//! 틱이었다(2026-11-16: 400 ms 간격이 9,600,000). 하드웨어 키가 어느 쪽인지 확인하지 못했으므로 두
//! 해석 가운데 말이 되는 것을 고른다. 기기가 켜진 지 몇 분만 지나면 둘 중 하나만 말이 된다.

/// 기다린 시간이 이보다 길면 그 해석은 틀린 것으로 본다.
pub const PLAUSIBLE_NS: u64 = 10_000_000_000;
/// 분포 칸의 위쪽 경계(ms). 마지막 칸은 그 위 전부다.
pub const BOUNDS_MS: [u64; 7] = [5, 10, 20, 50, 100, 200, 500];
pub const BUCKETS: usize = BOUNDS_MS.len() + 1;
/// 이만큼(ms) 이상 기다린 누름을 늦은 누름으로 본다. 사람이 늦었다고 느끼기 시작하는 정도로 잡았다.
pub const SLOW_MS: u64 = 50;
/// `SLOW_MS` 부터 시작하는 칸.
pub const SLOW_BUCKET: usize = 4;

/// mach 틱을 나노초로 바꾸는 비율(`mach_timebase_info`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timebase {
    pub numer: u32,
    pub denom: u32,
}

impl Timebase {
    #[must_use]
    pub fn to_ns(self, ticks: u64) -> Option<u64> {
        if self.denom == 0 {
            return None;
        }
        let ns = u128::from(ticks) * u128::from(self.numer) / u128::from(self.denom);
        u64::try_from(ns).ok()
    }

    /// 나노초를 mach 틱으로 바꾼다. `u32` 에 들지 않으면 `None`.
    #[must_use]
    pub fn ticks(self, ns: u64) -> Option<u32> {
        if self.numer == 0 {
            return None;
        }
        let ticks = u128::from(ns) * u128::from(self.denom) / u128::from(self.numer);
        u32::try_from(ticks).ok()
    }
}

/// 키가 만들어진 뒤 `now_ns` 까지 기다린 시간. 만든 시각을 나노초로도 틱으로도 읽을 수 없으면 `None`.
#[must_use]
pub fn waited_ns(now_ns: u64, created: u64, timebase: Timebase) -> Option<u64> {
    if created == 0 {
        return None;
    }
    let plausible = |at: u64| {
        now_ns
            .checked_sub(at)
            .filter(|waited| *waited < PLAUSIBLE_NS)
    };
    plausible(created).or_else(|| timebase.to_ns(created).and_then(plausible))
}

/// 기다린 시간이 들어갈 분포 칸.
#[must_use]
pub fn bucket(waited_ns: u64) -> usize {
    let ms = waited_ns / 1_000_000;
    BOUNDS_MS
        .iter()
        .position(|bound| ms < *bound)
        .unwrap_or(BOUNDS_MS.len())
}

/// 누름들의 대기 분포.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Lag {
    /// 칸마다의 누름 수.
    pub counts: [u64; BUCKETS],
    /// 만든 시각을 읽을 수 없던 누름 수.
    pub unknown: u64,
    pub max_ns: u64,
}

impl Lag {
    /// 누름 하나를 더한다.
    pub fn record(&mut self, waited: Option<u64>) {
        match waited {
            Some(waited) => {
                if let Some(count) = self.counts.get_mut(bucket(waited)) {
                    *count = count.saturating_add(1);
                }
                self.max_ns = self.max_ns.max(waited);
            }
            None => self.unknown = self.unknown.saturating_add(1),
        }
    }

    /// 다른 구간의 분포를 더한다.
    pub fn add(&mut self, other: &Self) {
        for (mine, theirs) in self.counts.iter_mut().zip(other.counts) {
            *mine = mine.saturating_add(theirs);
        }
        self.unknown = self.unknown.saturating_add(other.unknown);
        self.max_ns = self.max_ns.max(other.max_ns);
    }

    /// 시간을 잰 누름 수.
    #[must_use]
    pub fn measured(&self) -> u64 {
        self.counts
            .iter()
            .fold(0, |sum, count| sum.saturating_add(*count))
    }

    /// 50 ms 이상 기다린 누름 수.
    #[must_use]
    pub fn slow(&self) -> u64 {
        self.counts
            .get(SLOW_BUCKET..)
            .unwrap_or_default()
            .iter()
            .fold(0, |sum, count| sum.saturating_add(*count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;
    /// Apple Silicon 의 비율. 1 틱이 41.67 ns 다.
    const ARM: Timebase = Timebase {
        numer: 125,
        denom: 3,
    };
    /// 15일 켜져 있던 기기의 지금(ns).
    const NOW: u64 = 15 * 24 * 3600 * 1_000 * MS;

    #[test]
    fn a_creation_time_in_nanoseconds_is_read_as_is() {
        assert_eq!(waited_ns(NOW, NOW - 7 * MS, ARM), Some(7 * MS));
    }

    #[test]
    fn a_creation_time_in_mach_ticks_is_converted() {
        // 7 ms 전에 만든 이벤트의 틱 값.
        let created = (NOW - 7 * MS) * 3 / 125;
        let waited = waited_ns(NOW, created, ARM).expect("틱으로 읽혀야 한다");
        assert!(waited.abs_diff(7 * MS) < 1_000, "{waited}");
    }

    #[test]
    fn nanoseconds_become_ticks_for_thread_policies() {
        assert_eq!(ARM.ticks(MS), Some(24_000));
        assert_eq!(Timebase { numer: 1, denom: 1 }.ticks(MS), Some(1_000_000));
        assert_eq!(Timebase { numer: 0, denom: 1 }.ticks(MS), None);
        assert_eq!(ARM.ticks(u64::MAX), None);
    }

    #[test]
    fn zero_future_and_far_past_creation_times_are_unknown() {
        assert_eq!(waited_ns(NOW, 0, ARM), None);
        assert_eq!(waited_ns(NOW, NOW + MS, ARM), None);
        assert_eq!(waited_ns(NOW, NOW - PLAUSIBLE_NS, ARM), None);
        assert_eq!(waited_ns(NOW, 1, Timebase { numer: 1, denom: 0 }), None);
    }

    #[test]
    fn buckets_split_at_their_bounds() {
        assert_eq!(bucket(0), 0);
        assert_eq!(bucket(4 * MS + MS / 2), 0);
        assert_eq!(bucket(5 * MS), 1);
        assert_eq!(bucket(49 * MS), 3);
        assert_eq!(bucket(SLOW_MS * MS), SLOW_BUCKET);
        assert_eq!(bucket(SLOW_MS * MS - 1), SLOW_BUCKET - 1);
        assert_eq!(bucket(499 * MS), BUCKETS - 2);
        assert_eq!(bucket(3_000 * MS), BUCKETS - 1);
    }

    #[test]
    fn slow_presses_are_those_of_fifty_milliseconds_or_more() {
        let mut lag = Lag::default();
        for waited in [3, 12, 49, 50, 340, 900] {
            lag.record(Some(waited * MS));
        }
        lag.record(None);
        assert_eq!(lag.measured(), 6);
        assert_eq!(lag.slow(), 3);
        assert_eq!(lag.unknown, 1);
        assert_eq!(lag.max_ns, 900 * MS);
    }

    #[test]
    fn intervals_add_up_to_the_whole_run() {
        let mut first = Lag::default();
        first.record(Some(3 * MS));
        first.record(Some(120 * MS));
        let mut second = Lag::default();
        second.record(Some(60 * MS));
        second.record(None);
        let mut total = Lag::default();
        total.add(&first);
        total.add(&second);
        assert_eq!(total.measured(), 3);
        assert_eq!(total.slow(), 2);
        assert_eq!(total.unknown, 1);
        assert_eq!(total.max_ns, 120 * MS);
    }
}

//! 키 매핑을 언제 다시 확인할지 정한다.
//!
//! 매핑은 키보드가 다시 붙으면 사라진다. 1초마다 훑는 대신, 키보드가 붙었다는 알림을 받았을 때
//! 몇 번 촘촘히 확인하고, 그 밖에는 드물게만 확인한다.

/// 키보드가 붙은 뒤 첫 확인까지. HID 시스템이 새 키보드를 목록에 올릴 시간을 준다.
pub const FIRST_MS: u32 = 300;
/// 붙은 직후 이어서 확인하는 간격.
pub const BURST_GAP_MS: u32 = 1_000;
/// 붙은 직후 이어서 확인하는 횟수.
pub const BURST: u8 = 3;
/// 아무 일 없을 때의 확인 간격. 잠자기 복귀처럼 알림 없이 매핑이 사라지는 경우를 덮는다.
pub const IDLE_MS: u32 = 30_000;

#[derive(Debug, Default)]
pub struct Recheck {
    burst: u8,
}

impl Recheck {
    #[must_use]
    pub const fn new() -> Self {
        Self { burst: 0 }
    }

    /// 키보드가 붙었다. 다음 확인까지의 시간(ms)을 돌려준다.
    pub const fn device_changed(&mut self) -> u32 {
        self.burst = BURST;
        FIRST_MS
    }

    /// 확인을 한 번 마쳤다. 다음 확인까지의 시간(ms)을 돌려준다.
    pub const fn checked(&mut self) -> u32 {
        if self.burst > 1 {
            self.burst = self.burst.saturating_sub(1);
            BURST_GAP_MS
        } else {
            self.burst = 0;
            IDLE_MS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_checks_are_far_apart() {
        let mut recheck = Recheck::new();
        assert_eq!(recheck.checked(), IDLE_MS);
        assert_eq!(recheck.checked(), IDLE_MS);
    }

    #[test]
    fn a_new_keyboard_is_checked_soon_then_a_few_times_then_rarely() {
        let mut recheck = Recheck::new();
        let mut delays = vec![recheck.device_changed()];
        for _ in 0..4 {
            delays.push(recheck.checked());
        }
        assert_eq!(
            delays,
            [FIRST_MS, BURST_GAP_MS, BURST_GAP_MS, IDLE_MS, IDLE_MS]
        );
    }

    #[test]
    fn a_second_keyboard_during_a_burst_starts_the_burst_over() {
        let mut recheck = Recheck::new();
        recheck.device_changed();
        recheck.checked();
        recheck.checked();
        assert_eq!(recheck.device_changed(), FIRST_MS);
        assert_eq!(recheck.checked(), BURST_GAP_MS);
        assert_eq!(recheck.checked(), BURST_GAP_MS);
        assert_eq!(recheck.checked(), IDLE_MS);
    }
}

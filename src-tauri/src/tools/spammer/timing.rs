use crate::tools::input::SpamCycleTiming;

const MIN_POST_DELAY_MS: u64 = 16;
const DEADLINE_BUDGET_MS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpammerTimingPlan {
    pub cycle: SpamCycleTiming,
    pub post_delay_ms: u64,
    pub deadline_budget_ms: u64,
    pub nominal_period_ms: u64,
}

impl SpammerTimingPlan {
    pub fn new(configured_delay_ms: u64) -> Self {
        let cycle = SpamCycleTiming::stable();
        let post_delay_ms = configured_delay_ms.max(MIN_POST_DELAY_MS);
        Self {
            cycle,
            post_delay_ms,
            deadline_budget_ms: DEADLINE_BUDGET_MS,
            nominal_period_ms: cycle.repeated_work_ms() + post_delay_ms,
        }
    }

    pub fn log_line(self) -> String {
        format!(
            "key_rearm_ms={} key_to_click_ms={} click_hold_ms={} post_delay_ms={} nominal_period_ms={} deadline_budget_ms={}",
            self.cycle.key_rearm_settle().as_millis(),
            self.cycle.key_to_click_settle().as_millis(),
            self.cycle.click_hold().as_millis(),
            self.post_delay_ms,
            self.nominal_period_ms,
            self.deadline_budget_ms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stable_timing_clamps_to_the_proven_seventy_millisecond_period() {
        let plan = SpammerTimingPlan::new(10);

        assert_eq!(plan.cycle.key_rearm_settle(), Duration::from_millis(17));
        assert_eq!(plan.cycle.key_to_click_settle(), Duration::from_millis(20));
        assert_eq!(plan.cycle.click_hold(), Duration::from_millis(17));
        assert_eq!(plan.cycle.repeated_work_ms(), 54);
        assert_eq!(plan.post_delay_ms, 16);
        assert_eq!(plan.nominal_period_ms, 70);
        assert_eq!(plan.deadline_budget_ms, 10);
    }

    #[test]
    fn configured_delay_can_make_the_stable_cycle_slower_but_not_faster() {
        let plan = SpammerTimingPlan::new(23);
        assert_eq!(plan.post_delay_ms, 23);
        assert_eq!(plan.deadline_budget_ms, 10);
        assert_eq!(plan.nominal_period_ms, 77);
    }
}

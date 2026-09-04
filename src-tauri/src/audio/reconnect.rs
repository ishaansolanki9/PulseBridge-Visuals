use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutePolicy {
    AutomaticOutput,
    SelectedOutput,
}

#[derive(Clone, Copy, Debug)]
pub struct ReconnectPolicy {
    pub output_probe_grace: Duration,
    pub output_retry: Duration,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            output_probe_grace: Duration::from_secs(2),
            output_retry: Duration::from_secs(30),
        }
    }
}

impl ReconnectPolicy {
    pub fn should_retry(
        self,
        route: RoutePolicy,
        signal_was_seen: bool,
        silence_age: Duration,
    ) -> bool {
        match route {
            RoutePolicy::AutomaticOutput if signal_was_seen => silence_age >= self.output_retry,
            RoutePolicy::AutomaticOutput => silence_age >= self.output_probe_grace,
            RoutePolicy::SelectedOutput => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_policy_distinguishes_automatic_and_selected_outputs() {
        let policy = ReconnectPolicy::default();
        assert!(!policy.should_retry(RoutePolicy::AutomaticOutput, false, Duration::from_secs(1)));
        assert!(policy.should_retry(RoutePolicy::AutomaticOutput, false, Duration::from_secs(2)));
        assert!(!policy.should_retry(RoutePolicy::AutomaticOutput, true, Duration::from_secs(29)));
        assert!(policy.should_retry(RoutePolicy::AutomaticOutput, true, Duration::from_secs(30)));
        assert!(!policy.should_retry(RoutePolicy::SelectedOutput, false, Duration::from_secs(300)));
    }
}

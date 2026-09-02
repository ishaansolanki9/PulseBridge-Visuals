use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutePolicy {
    Process,
    DefaultFallback,
    SelectedOutput,
}

#[derive(Clone, Copy, Debug)]
pub struct ReconnectPolicy {
    pub process_initial_grace: Duration,
    pub process_loss_grace: Duration,
    pub fallback_retry: Duration,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            process_initial_grace: Duration::from_secs(3),
            process_loss_grace: Duration::from_secs(5),
            fallback_retry: Duration::from_secs(30),
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
            RoutePolicy::Process if signal_was_seen => silence_age >= self.process_loss_grace,
            RoutePolicy::Process => silence_age >= self.process_initial_grace,
            RoutePolicy::DefaultFallback => silence_age >= self.fallback_retry,
            RoutePolicy::SelectedOutput => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_policy_distinguishes_process_fallback_and_selected_routes() {
        let policy = ReconnectPolicy::default();
        assert!(!policy.should_retry(RoutePolicy::Process, false, Duration::from_secs(2)));
        assert!(policy.should_retry(RoutePolicy::Process, false, Duration::from_secs(3)));
        assert!(!policy.should_retry(RoutePolicy::DefaultFallback, false, Duration::from_secs(29)));
        assert!(policy.should_retry(RoutePolicy::DefaultFallback, true, Duration::from_secs(30)));
        assert!(!policy.should_retry(RoutePolicy::SelectedOutput, false, Duration::from_secs(300)));
    }
}

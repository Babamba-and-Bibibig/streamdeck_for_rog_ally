use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ReconnectBackoff {
    initial: Duration,
    maximum: Duration,
    current: Duration,
}

impl ReconnectBackoff {
    pub fn new(initial: Duration, maximum: Duration) -> Self {
        let initial = initial.min(maximum);
        Self {
            initial,
            maximum,
            current: initial,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = self.current.saturating_mul(2).min(self.maximum);
        delay
    }

    pub fn reset(&mut self) {
        self.current = self.initial;
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new(Duration::from_millis(500), Duration::from_secs(15))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_backoff_grows_caps_and_resets() {
        let mut backoff =
            ReconnectBackoff::new(Duration::from_millis(100), Duration::from_millis(450));

        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
        assert_eq!(backoff.next_delay(), Duration::from_millis(200));
        assert_eq!(backoff.next_delay(), Duration::from_millis(400));
        assert_eq!(backoff.next_delay(), Duration::from_millis(450));
        assert_eq!(backoff.next_delay(), Duration::from_millis(450));
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }
}

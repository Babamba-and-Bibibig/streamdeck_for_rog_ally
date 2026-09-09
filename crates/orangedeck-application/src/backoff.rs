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

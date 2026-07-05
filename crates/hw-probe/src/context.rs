use hw_source::SourceRunner;
use std::time::Duration;

pub struct ProbeContext<'a> {
    pub runner: &'a dyn SourceRunner,
    pub timeout: Duration,
}

impl<'a> ProbeContext<'a> {
    pub fn new(runner: &'a dyn SourceRunner, timeout: Duration) -> Self {
        Self { runner, timeout }
    }
}

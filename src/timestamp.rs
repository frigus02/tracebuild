use std::{
    fmt::Display,
    str::FromStr,
    time::{Duration, SystemTime},
};

pub(crate) struct Timestamp(SystemTime);

impl Timestamp {
    pub(crate) fn now() -> Self {
        Self(SystemTime::now())
    }

    pub(crate) fn system_time(&self) -> SystemTime {
        self.0
    }
}

impl FromStr for Timestamp {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let secs_since_epoch = u64::from_str_radix(s, 10)?;
        let since_epoch = Duration::from_secs(secs_since_epoch);
        Ok(Timestamp(
            SystemTime::UNIX_EPOCH
                .checked_add(since_epoch)
                .ok_or("secs is too large")?,
        ))
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secs = self
            .0
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("System time before UNIX EPOCH")
            .as_secs();
        write!(f, "{}", secs)
    }
}

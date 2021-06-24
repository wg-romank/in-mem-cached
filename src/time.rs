use std::time::Instant;

pub trait Time {
    fn get_time(&self) -> Instant;
}

pub struct Realtime {}

pub static REALTIME: Realtime = Realtime {};

impl Time for Realtime {
    fn get_time(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
pub mod time_fixtures {
    use crate::time::Time;

    use std::ops::Add;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;

    pub struct TestTime {
        now: Instant,
        seconds_passed: AtomicUsize,
    }

    impl TestTime {
        pub fn new(now: Instant) -> TestTime {
            TestTime {
                now,
                seconds_passed: AtomicUsize::new(0),
            }
        }

        pub fn add_secs(&self, duration: Duration) {
            self.seconds_passed
                .store(duration.as_secs() as usize, Ordering::SeqCst);
        }
    }

    impl Time for TestTime {
        fn get_time(&self) -> Instant {
            self.now.add(Duration::from_secs(
                self.seconds_passed.load(Ordering::SeqCst) as u64,
            ))
        }
    }
}

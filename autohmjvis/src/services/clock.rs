//! src/services/clock.rs
//!
//! very basic software ticker for testing
//! perfect accuracy is not guaranteed
//! for better accuracy, use Ableton Link

use std::time::{Duration, Instant};

pub struct ClockService {
    start_time: Instant,
    last_tick: Instant,
    beat_count: usize,
    beat_duration: Duration,
    next_tick: Instant,
}

impl ClockService {
    pub fn new(bpm: u32) -> Self {
        let now = Instant::now();
        let duration = Duration::from_secs_f64(60.0 / bpm as f64);
        Self {
            start_time: now,
            last_tick: now,
            beat_count: 0,
            beat_duration: duration,
            next_tick: now + duration,
        }
    }

    pub fn tick(&mut self) -> bool {
        let now = Instant::now();

        if now >= self.next_tick {
            self.beat_count += 1;
            self.last_tick = now;
            self.next_tick = self.start_time + self.beat_duration.mul_f64(self.beat_count as f64);

            return true;
        }

        false
    }
}

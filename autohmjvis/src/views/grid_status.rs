//! src/views/grid_status.rs
//!
//! A little status indicator for a grid
//!

use chrono::prelude::*;
use std::time::{Instant, SystemTime};

// I-ching symbols range from U+4DC0 to U+4DFF

pub struct GridStatusBar {
    pub start: Instant,
    pub status: GridStatus,
}

impl GridStatusBar {
    pub fn init() -> Self {
        Self {
            start: Instant::now(),
            status: GridStatus::Idle,
        }
    }

    pub fn set_idle(&mut self, now: Instant) {
        if self.status != GridStatus::Idle {
            self.set_status(GridStatus::Idle, now);
        }
    }

    pub fn set_waiting(&mut self, now: Instant) {
        if self.status != GridStatus::Waiting {
            self.set_status(GridStatus::Waiting, now);
        }
    }

    /// Returns `true` if the typing animation has just begun
    pub fn set_typing(&mut self, now: Instant) -> bool {
        if self.status != GridStatus::Typing {
            self.set_status(GridStatus::Typing, now);
            true
        } else {
            false
        }
    }

    pub fn set_timestamp(&mut self, now: Instant) {
        if self.status != GridStatus::Timestamp {
            self.set_status(GridStatus::Timestamp, now);
        }
    }

    fn set_status(&mut self, status: GridStatus, now: Instant) {
        self.status = status;
        self.start = now;
    }

    pub fn get_content(&self, now: Instant) -> String {
        match self.status {
            GridStatus::Idle => "".to_string(),
            GridStatus::Waiting => self.get_i_ching_sigil(now).to_string(),
            GridStatus::Typing => self.get_typing_indicator(now),
            GridStatus::Timestamp => self.get_current_time(),
        }
    }

    fn get_current_time(&self) -> String {
        // Calculate how long ago self.start was
        let elapsed_since_start = self.start.elapsed();

        // Get current system time and subtract the elapsed duration
        let system_time_at_start = SystemTime::now() - elapsed_since_start;

        // Convert to DateTime and format
        let datetime: DateTime<Local> = system_time_at_start.into();
        datetime.format("%H:%M:%S").to_string()
    }

    fn get_typing_indicator(&self, now: Instant) -> String {
        let elapsed_secs = now.duration_since(self.start).as_secs_f32();

        // Alternate every 500ms (0.5 seconds)
        let index = (elapsed_secs / 0.5) as usize % 2;

        if index == 0 {
            " ... ".to_string()
        } else {
            ". . .".to_string()
        }
    }

    fn get_i_ching_sigil(&self, now: Instant) -> char {
        use nannou::rand::{rngs::StdRng, Rng, SeedableRng};

        let elapsed_secs = now.duration_since(self.start).as_secs_f32();

        // First, calculate the total duration of one complete cycle through all 64 hexagrams
        let mut cycle_duration = 0.0;
        for i in 0..64 {
            let mut rng = StdRng::seed_from_u64(i);
            let speed_factor = 1.0 + rng.gen_range(-0.8..1.2);
            cycle_duration += 0.1 * speed_factor;
        }

        // Find where we are within the current cycle
        let time_in_cycle = elapsed_secs % cycle_duration;

        // Now find which hexagram we should be showing
        let mut accumulated_time = 0.0;
        let mut current_index = 0;

        for i in 0..64 {
            let mut rng = StdRng::seed_from_u64(i);
            let speed_factor = 1.0 + rng.gen_range(-0.5..1.2);
            let period = 0.1 * speed_factor;

            if accumulated_time + period > time_in_cycle {
                current_index = i;
                break;
            }
            accumulated_time += period;
        }

        // I Ching hexagrams range from U+4DC0 to U+4DFF (64 symbols total)
        let code_point = 0x4DC0 + (current_index % 64) as u32;
        char::from_u32(code_point).unwrap_or('?')
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GridStatus {
    Idle,
    Waiting,
    Typing,
    Timestamp,
}

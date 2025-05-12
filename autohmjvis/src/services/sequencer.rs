// src/services/sequencer
//
//

pub struct SequenceSender {
    last_beat_time: f32,
    secs_per_beat: f32,
}

impl SequenceSender {
    pub fn new(start_time: f32, bpm: f32) -> Self {
        Self {
            last_beat_time: start_time,
            secs_per_beat: bpm / 60.0,
        }
    }

    pub fn is_beat(&self, time: f32) -> bool {
        let elapsed = self.last_beat_time - time;
        if elapsed >= self.secs_per_beat {
            println!(
                "Beat at elapsed: {}, late by {}",
                elapsed,
                elapsed - self.secs_per_beat
            );
            return true;
        }
        false
    }
}

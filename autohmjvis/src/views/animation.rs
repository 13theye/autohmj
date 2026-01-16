//! src/services/animation.rs
//!
//! Applies the text typing animation
use nannou::rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::views::{anim_hangeul::HangeulAnimator, GridCellChar};

// Track animation state for each message
pub struct TypingAnimationController {
    animations: HashMap<usize, TypingAnimation>, // <message_key, animation>
    reveal_speed: f32,                           // Chars per second
    variation: f32,                              // Speed variation

    hangeul_animator: HangeulAnimator,
}

struct TypingAnimation {
    total_chars: usize,
    revealed_chars: usize,
    complete: bool,
    char_progress: Vec<f32>, // track animation progress for each char (0.0-1.0)
    char_speed_factors: Vec<f32>, // speed variations at the char level
    char_start_times: Vec<Option<Instant>>, // start time of character reveal; None if not revealed
}

// Event interface
#[derive(Clone, Debug)]
pub enum TypingAnimationEvent {
    ResponseReceived,
}

impl TypingAnimationController {
    pub fn new(reveal_speed: f32, variation: f32) -> Self {
        Self {
            animations: HashMap::new(),
            reveal_speed,
            variation,

            hangeul_animator: HangeulAnimator::default(),
        }
    }

    // Tick and apply a particular animation, then remove it if finished
    pub fn update(
        &mut self,
        message_key: usize,
        grid_cell_chars: &mut [GridCellChar],
        now: Instant,
    ) {
        self.tick(now, message_key);
        self.apply(message_key, grid_cell_chars);
    }

    // Register a new message for animation
    pub fn register(
        &mut self,
        message_id: usize,
        characters: &[GridCellChar],
        start_time: Instant,
    ) {
        let char_count = characters.len();

        self.hangeul_animator.analyze(characters);
        let char_progress = vec![0.0; char_count];

        // Generate random timing variations for each character
        let mut char_speed_factors = Vec::with_capacity(char_count);
        let mut char_start_times = vec![None; char_count];

        let mut rng = thread_rng();

        // Generate random timing variations for each character
        for _ in 0..char_count {
            // Generate a random factor between (1 - variation/2) and (1 + variation)
            let speed_factor = 1.0 + rng.gen_range(-self.variation..self.variation);

            char_speed_factors.push(speed_factor);
        }

        // Initialize the first character to start immediately
        if char_count > 0 {
            char_start_times[0] = Some(start_time);
        }

        self.animations.insert(
            message_id,
            TypingAnimation {
                total_chars: char_count,
                revealed_chars: 1,
                complete: false,
                char_progress,
                char_speed_factors,
                char_start_times,
            },
        );
    }

    pub fn check_animation_key_finished(&self, message_key: usize) -> bool {
        let Some(animation) = self.animations.get(&message_key) else {
            return true;
        };

        animation.complete
    }

    fn tick(&mut self, now: Instant, message_key: usize) {
        if let Some(animation) = self.animations.get_mut(&message_key) {
            if !animation.complete {
                let next_reveal_index = animation.revealed_chars;

                // 1. Update progress for all currently visible characters
                for i in 0..next_reveal_index.min(animation.char_progress.len()) {
                    // Calculate how long this character has been visible

                    if let Some(char_start_time) = animation.char_start_times[i] {
                        // Calculate how long this char has been visible
                        let char_elapsed = now - char_start_time;

                        if char_elapsed.as_secs_f32() > 0.0 {
                            // Get this character's speed factor
                            let char_factor = animation.char_speed_factors.get(i).unwrap_or(&1.0);

                            // Update progress
                            animation.char_progress[i] = (char_elapsed.as_secs_f32()
                                / (self.reveal_speed / 4.0 * char_factor))
                                .min(1.0);
                        }

                        // Check if this character is complete and should trigger the next one
                        if i == animation.revealed_chars - 1
                            && animation.char_progress[i] >= 0.99
                            && next_reveal_index < animation.total_chars
                        {
                            let mut rng = thread_rng();
                            let delay = Duration::from_secs_f32(0.03 * rng.gen_range(0.85..5.0)); //  delay(s) between characters
                            animation.char_start_times[next_reveal_index] = Some(now + delay);
                            animation.revealed_chars += 1;
                        }
                    }

                    // The animation is done when the last character completes
                    if animation.revealed_chars >= animation.total_chars {
                        animation.complete =
                            animation.char_progress[animation.total_chars - 1] >= 0.99;
                    }
                }
            }
        }
    }

    fn apply(&mut self, message_key: usize, grid_cell_chars: &mut [GridCellChar]) {
        let cleanup = match self.animations.get(&message_key) {
            Some(animation) => {
                for (idx, character) in grid_cell_chars.iter_mut().enumerate() {
                    character.is_visible = idx < animation.revealed_chars;

                    // If visible and a Hangul character, update the display character based on progress
                    if character.is_visible && character.is_hangeul {
                        if let Some(&progress) = animation.char_progress.get(idx) {
                            character.display = if progress < 1.0 {
                                self.hangeul_animator.get_char_state(character.c, progress)
                            } else {
                                character.c
                            };
                        }
                    }
                }

                animation.complete // flag complete animation for cleanup
            }
            None => {
                for character in grid_cell_chars.iter_mut() {
                    character.is_visible = true;
                    character.display = character.c;
                }
                false // no animation to clean up
            }
        };

        if cleanup {
            self.animations.remove(&message_key);
        }
    }
}

// Animation curve function for flexible pulse effects
pub fn animation_curve(
    progress: f32,
    attack_ratio: f32,
    exp_attack: f32,
    exp_decay: f32,
    amplitude: f32,
) -> f32 {
    let peak_point = attack_ratio;

    let pulse_value = if progress < peak_point {
        // Attack phase - rise to peak
        let attack_progress = progress / peak_point;
        attack_progress.powf(exp_attack) // Lower values = faster initial attack
    } else {
        // Decay phase - fall from peak
        let decay_progress = (progress - peak_point) / (1.0 - peak_point);
        (1.0 - decay_progress).powf(exp_decay) // Higher values = longer tail
    };

    // Scale and offset (base = 1.0, add amplitude * pulse_value)
    1.0 + amplitude * pulse_value
}

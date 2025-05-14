// src/services/animation.rs
//
// Handle text typing animation
use nannou::rand::{thread_rng, Rng};
use std::collections::HashMap;

use crate::views::{anim_hangeul::HangeulAnimator, grid::CharacterEntity};

// Track animation state for each message
pub struct AnimationController {
    animations: HashMap<usize, MessageAnimation>, // <message_key, animation>
    reveal_speed: f32,                            // Chars per second
    variation: f32,                               // Speed variation

    hangeul_animator: HangeulAnimator,
}

struct MessageAnimation {
    total_chars: usize,
    revealed_chars: usize,
    complete: bool,
    char_progress: Vec<f32>, // track animation progress for each char (0.0-1.0)
    char_speed_factors: Vec<f32>, // speed variations at the char level
    char_start_times: Vec<Option<f32>>, // start time of character reveal; None if not revealed
}

// Event interface
#[derive(Clone, Debug)]
pub enum AnimationEvent {
    ResponseReceived,
}

impl AnimationController {
    pub fn new(reveal_speed: f32, variation: f32) -> Self {
        Self {
            animations: HashMap::new(),
            reveal_speed,
            variation,

            hangeul_animator: HangeulAnimator::default(),
        }
    }

    // Tick and apply a particular animation, then remove it if finished
    pub fn update(&mut self, time: f32, message_key: usize, entities: &mut [CharacterEntity]) {
        self.tick(time, message_key);
        self.apply(message_key, entities);
    }

    // Register a new message for animation
    pub fn register(
        &mut self,
        message_id: usize,
        text_vec: &[CharacterEntity],
        char_count: usize,
        start_time: f32,
    ) {
        self.hangeul_animator.analyze(text_vec);
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
            MessageAnimation {
                total_chars: char_count,
                revealed_chars: 1,
                complete: false,
                char_progress,
                char_speed_factors,
                char_start_times,
            },
        );
    }

    fn tick(&mut self, time: f32, message_key: usize) {
        if let Some(animation) = self.animations.get_mut(&message_key) {
            if !animation.complete {
                let next_reveal_index = animation.revealed_chars;

                // 1. Update progress for all currently visible characters
                for i in 0..next_reveal_index.min(animation.char_progress.len()) {
                    // Calculate how long this character has been visible

                    if let Some(char_start_time) = animation.char_start_times[i] {
                        // Calculate how long this char has been visible
                        let char_elapsed = time - char_start_time;

                        if char_elapsed > 0.0 {
                            // Get this character's speed factor
                            let char_factor = animation.char_speed_factors.get(i).unwrap_or(&1.0);

                            // Update progress
                            animation.char_progress[i] =
                                (char_elapsed / (self.reveal_speed / 4.0 * char_factor)).min(1.0);
                        }

                        // Check if this character is complete and should trigger the next one
                        if i == animation.revealed_chars - 1
                            && animation.char_progress[i] >= 0.99
                            && next_reveal_index < animation.total_chars
                        {
                            let mut rng = thread_rng();
                            let delay = 0.03 * rng.gen_range(0.85..5.0); //  delay(s) between characters
                            animation.char_start_times[next_reveal_index] = Some(time + delay);
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

    fn apply(&mut self, message_key: usize, entities: &mut [CharacterEntity]) {
        let cleanup = match self.animations.get(&message_key) {
            Some(animation) => {
                for (idx, entity) in entities.iter_mut().enumerate() {
                    entity.is_visible = idx < animation.revealed_chars;

                    // If visible and a Hangul character, update the display character based on progress
                    if entity.is_visible && entity.is_hangeul {
                        if let Some(&progress) = animation.char_progress.get(idx) {
                            entity.display_char = if progress < 1.0 {
                                self.hangeul_animator
                                    .get_char_state(entity.character, progress)
                            } else {
                                entity.character
                            };
                        }
                    }
                }

                animation.complete // flag complete animation for cleanup
            }
            None => {
                for entity in entities.iter_mut() {
                    entity.is_visible = true;
                    entity.display_char = entity.character;
                }
                false // no animation to clean up
            }
        };

        if cleanup {
            self.animations.remove(&message_key);
        }
    }
}

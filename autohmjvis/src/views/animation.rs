// src/services/animation.rs
//
// Handle text typing animation

use crate::views::{anim_hangeul::HangeulAnimator, grid::CharacterEntity};
use std::collections::HashMap;

// Track animation state for each message
pub struct AnimationController {
    animations: HashMap<usize, MessageAnimation>, // <message_key, animation>
    reveal_speed: f32,                            // Chars per second
    variation: f32,                               // Speed variation

    hangeul_animator: HangeulAnimator,
}

struct MessageAnimation {
    message_key: usize,
    total_chars: usize,
    revealed_chars: usize,
    complete: bool,
    start_time: f32,
    variation: f32,
    char_progress: Vec<f32>, // track animation progress for each char (0.0-1.0)
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
        message: &str,
        char_count: usize,
        start_time: f32,
    ) {
        self.hangeul_animator.analyze(message);
        let char_progress = vec![0.0; char_count];

        self.animations.insert(
            message_id,
            MessageAnimation {
                message_key: message_id,
                total_chars: char_count,
                revealed_chars: 0,
                complete: false,
                start_time,
                variation: self.variation,
                char_progress,
            },
        );
    }

    fn tick(&mut self, time: f32, message_key: usize) {
        if let Some(animation) = self.animations.get_mut(&message_key) {
            if !animation.complete {
                // Calculate how many chars should be revealed by now
                let elapsed = time - animation.start_time;
                let target_revealed = (elapsed * self.reveal_speed) as usize;

                // Update progress for each visible character
                // for i in 0..animation.revealed_chars.min(animation.char_progress.len()) {
                for i in 0..target_revealed.min(animation.char_progress.len()) {
                    // Calculate how long this character has been visible
                    let char_start_time = (i as f32 / self.reveal_speed) + animation.start_time;
                    let char_elapsed = time - char_start_time;

                    if char_elapsed > 0.0 {
                        // For Hangeul: progress through jamo states over a short time
                        // For non-Hangul: instantly complete (progress = 1.0)
                        // 4x faster for individual jamo
                        animation.char_progress[i] =
                            (char_elapsed / (self.reveal_speed / 4.0)).min(1.0);
                    }
                }

                // Update revealed count
                animation.revealed_chars = target_revealed.max(animation.revealed_chars);

                // Check if animation is complete
                if animation.revealed_chars >= animation.total_chars {
                    animation.revealed_chars = animation.total_chars;
                    // Check if all characters have completed their individual animations
                    let all_chars_complete = animation
                        .char_progress
                        .iter()
                        .all(|&progress| progress >= 0.99);

                    if all_chars_complete {
                        // Only set complete flag when all characters have finished animating
                        animation.complete = true;

                        // Ensure all characters show their final form
                        for progress in &mut animation.char_progress {
                            *progress = 1.0;
                        }
                    }
                }
            }
        }
    }

    fn apply(&mut self, message_key: usize, entities: &mut [CharacterEntity]) {
        let cleanup = match self.animations.get(&message_key) {
            Some(animation) => {
                for (i, entity) in entities.iter_mut().enumerate() {
                    entity.is_visible = i < animation.revealed_chars;

                    // If visible and a Hangul character, update the display character based on progress
                    if entity.is_visible && entity.is_hangeul && i < animation.char_progress.len() {
                        let progress = animation.char_progress[i];
                        if progress < 1.0 {
                            // Get the appropriate Jamo state based on progress
                            entity.display_char = self
                                .hangeul_animator
                                .get_char_state(entity.character, progress);
                        } else {
                            // Show the complete character
                            entity.display_char = entity.character;
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

// src/services/animation.rs
//
// Handle text typing animation

use crate::views::grid::CharacterEntity;
use std::collections::HashMap;

// Track animation state for each message
pub struct AnimationController {
    animations: HashMap<usize, MessageAnimation>, // <message_key, animation>
    reveal_speed: f32,                            // Chars per second
    variation: f32,                               // Speed variation
}

struct MessageAnimation {
    message_key: usize,
    total_chars: usize,
    revealed_chars: usize,
    complete: bool,
    start_time: f32,
    variation: f32,
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
        }
    }

    // Tick and apply a particular animation, then remove it if finished
    pub fn update(&mut self, time: f32, message_key: usize, entities: &mut [CharacterEntity]) {
        self.tick(time, message_key);
        self.apply(message_key, entities);
    }

    // Register a new message for animation
    pub fn register(&mut self, message_id: usize, char_count: usize, start_time: f32) {
        self.animations.insert(
            message_id,
            MessageAnimation {
                message_key: message_id,
                total_chars: char_count,
                revealed_chars: 0,
                complete: false,
                start_time,
                variation: self.variation,
            },
        );
    }

    fn tick(&mut self, time: f32, message_key: usize) {
        if let Some(animation) = self.animations.get_mut(&message_key) {
            if !animation.complete {
                // Calculate how many chars should be revealed by now
                let elapsed = time - animation.start_time;
                let target_revealed = (elapsed * self.reveal_speed) as usize;

                // Update revealed count
                animation.revealed_chars = target_revealed.max(animation.revealed_chars);

                // Check if animation is complete
                if animation.revealed_chars >= animation.total_chars {
                    animation.complete = true;
                }
            }
        }
    }

    fn apply(&mut self, message_key: usize, entities: &mut [CharacterEntity]) {
        let cleanup = match self.animations.get(&message_key) {
            Some(animation) => {
                for (i, entity) in entities.iter_mut().enumerate() {
                    entity.is_visible = i < animation.revealed_chars;
                }
                animation.complete // flag complete animation for cleanup
            }
            None => {
                for entity in entities.iter_mut() {
                    entity.is_visible = true;
                }
                false // no animation to clean up
            }
        };

        if cleanup {
            self.animations.remove(&message_key);
        }
    }
}

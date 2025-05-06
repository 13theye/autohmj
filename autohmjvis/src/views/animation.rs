// src/services/animation.rs
//
// Handle text typing animation

use crate::views::grid::CharacterEntity;
use std::collections::HashMap;

// Track animation state for each message
pub struct AnimationController {
    animations: HashMap<usize, MessageAnimation>,
    reveal_speed: f32, // Chars per second
    variation: f32,    // Speed variation
}

struct MessageAnimation {
    message_id: usize,
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

    // Register a new message for animation
    pub fn register(
        &mut self,
        message_id: usize,
        char_count: usize,
        start_time: f32,
        variation: f32,
    ) {
        self.animations.insert(
            message_id,
            MessageAnimation {
                message_id,
                total_chars: char_count,
                revealed_chars: 0,
                complete: false,
                start_time,
                variation,
            },
        );
    }

    pub fn update(&mut self, time: f32) {
        for animation in self.animations.values_mut() {
            if !animation.complete {
                // Calculate how many chars should be revealed by now
                let elapsed = time - animation.start_time;
                let target_revealed = (elapsed * self.reveal_speed) as usize;

                // Update revealed count
                animation.revealed_chars = target_revealed.min(animation.revealed_chars);

                // Check if animation is complete
                if animation.revealed_chars >= animation.total_chars {
                    animation.complete = true;
                }
            }
        }
    }

    pub fn apply(&self, message_id: usize, entities: &mut [CharacterEntity]) {
        if let Some(animation) = self.animations.get(&message_id) {
            for (i, entity) in entities.iter_mut().enumerate() {
                entity.is_visible = i < animation.revealed_chars;
            }
        } else {
            // If no animation exists, make all chars visible
            for entity in entities.iter_mut() {
                entity.is_visible = true;
            }
        }
    }
}

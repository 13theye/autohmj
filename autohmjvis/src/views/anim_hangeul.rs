// src/services/hangeul.rs
//
// Hangeul animation functionality

use std::collections::HashMap;

// The state of a Hangeul character being typed
#[derive(Debug, Clone)]
pub enum HangeulState {
    // just the leading consonant
    Choseong(char),
    // Leading consonant + vowel
    ChoJungseong(char),
    // Complete syllable
    Complete(char),
}

// Handles progressive typing animation of Hangeul characters
#[derive(Default, Debug)]
pub struct HangeulAnimator {
    // Cached maps from complete hangeul character to its progressive states
    jamo_states: HashMap<char, Vec<HangeulState>>,
}

impl HangeulAnimator {
    pub fn analyze(&mut self, text: &str) {
        for ch in text.chars() {
            if !self.jamo_states.contains_key(&ch) && is_hangeul_char(ch) {
                self.jamo_states.insert(ch, self.create_states(ch));
            }
        }
    }

    pub fn get_char_state(&self, ch: char, progress: f32) -> char {
        if let Some(states) = self.jamo_states.get(&ch) {
            // If no states defined, return original as fallback
            if states.is_empty() {
                return ch;
            }

            // Convert progress to a state index
            let state_index = if progress < 0.4 {
                0
            } else if progress < 0.8 {
                1
            } else {
                2
            };

            match &states[state_index] {
                HangeulState::Choseong(c) => *c,
                HangeulState::ChoJungseong(c) => *c,
                HangeulState::Complete(_) => ch,
            }
        } else {
            // not a Hangeul character or not analyzed
            ch
        }
    }

    // Populate typing states of a Hangeul character
    fn create_states(&self, ch: char) -> Vec<HangeulState> {
        let mut states = Vec::new();

        if let Ok((choseong, jungseong, _jeongseong)) = hangeul::decompose_char(&ch) {
            // Push leading consonant
            states.push(HangeulState::Choseong(choseong));

            // Push leading + vowel
            if let Ok(partial) = hangeul::compose_char(&choseong, &jungseong, None) {
                states.push(HangeulState::ChoJungseong(partial));
            }

            // Push final state
            states.push(HangeulState::Complete(ch));
        }

        states
    }
}

fn is_hangeul_char(ch: char) -> bool {
    hangeul::is_hangeul(ch as u32)
}

// src/services/sequencer
//
//

use crate::{config::OscSendConfig, osc::OscSender, views::grid::CharacterEntity};
use std::time::Instant;

pub struct Sequencer {
    id: String,
    pub send: bool,
    beat_count: usize,
    last_clock_time: Instant,
    pub current_idx: usize, // this increments at the START OF BEAT, not after read.
    characters: Vec<CharacterEntity>,
    osc_sender: OscSender,
}

impl Sequencer {
    pub fn new(id: &str, osc_config: &OscSendConfig) -> Self {
        let osc_sender = OscSender::new(osc_config).expect("Failed to create OSC Sender");
        Self {
            id: id.to_owned(),
            send: true,
            beat_count: 0,
            last_clock_time: Instant::now(),
            current_idx: 0,
            characters: Vec::new(),
            osc_sender,
        }
    }

    // Updates the sequencer state,
    // Sends OSC out,
    // Returns the index of the current character
    pub fn update(&mut self, chars: &[CharacterEntity]) {
        self.characters = chars.to_vec();

        // track the time
        self.increment();

        // process character
        if let Some(ch) = self.get_character() {
            // do nothing if not hangeul
            if hangeul::is_hangeul(ch as u32) {
                self.send_commands(decompose_character(ch));

            // punctuation that makes it here has been pre-filtered by the grid.
            // so we can just send it as is
            } else if ch.is_ascii_punctuation() {
                self.send_commands(vec![ch]);
            }
        }
    }

    fn increment(&mut self) {
        self.beat_count += 1;
        self.current_idx += 1;
        self.last_clock_time = Instant::now();
    }

    fn get_character(&mut self) -> Option<char> {
        let visible_chars: Vec<_> = self
            .characters
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_visible)
            .collect();

        // return early if no chars are visible
        if visible_chars.is_empty() {
            return None;
        }

        // go back to the first char is index is greater than length
        if self.current_idx >= visible_chars.len() {
            self.current_idx = 0;
        }

        let (_, entity) = visible_chars[self.current_idx];

        Some(entity.character)
    }

    fn send_commands(&self, chars: Vec<char>) {
        let length = chars.len();

        // if chars vec is invalid, return early
        if chars.is_empty() || length > 3 {
            return;
        }

        // Don't send if the self flag is false
        if !self.send {
            return;
        }

        let id = self.id.as_str();

        // send the num_letters message
        //self.osc_sender.send_num_letters(max_id, length as i32);

        // the length of the Vec is how many jamo are in the character.
        // send the seq message
        match chars.len() {
            0 => {}
            1 => {
                self.osc_sender
                    .send_seq(id, hangeul_to_i32(chars[0]), -1, 0);
            }
            2 => {
                self.osc_sender
                    .send_seq(id, hangeul_to_i32(chars[0]), hangeul_to_i32(chars[1]), 0);
            }
            _ => {
                self.osc_sender.send_seq(
                    id,
                    hangeul_to_i32(chars[0]),
                    hangeul_to_i32(chars[1]),
                    hangeul_to_i32(chars[2]),
                );
            }
        }
    }
}

fn decompose_character(ch: char) -> Vec<char> {
    if !hangeul::is_syllable(ch as u32) {
        return vec![ch];
    }

    println!("Original char: {}", ch);

    // Unwrap the hangeul::Decomposed type
    if let Ok(result) = hangeul::decompose_char(&ch) {
        // process (char, char, Option<char>)
        if result.2.is_none() {
            return vec![result.0, result.1];
        } else {
            return vec![result.0, result.1, result.2.unwrap()];
        }
    }

    Vec::new()
}

// 12592 is the hangeul offset
fn hangeul_to_i32(ch: char) -> i32 {
    let result = ch as u32; //- 12592;
    println!("Character {}: {}", ch, result);

    result as i32
}

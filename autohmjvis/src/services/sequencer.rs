// src/services/sequencer
//
//
// Desconstruct and parse hangeul characters and send OSC messages

use crate::{osc::OscSender, settings::OscSendConfig, views::GridCellChar};
use std::time::Instant;

pub struct Sequencer {
    id: String,             // id should be same as id of the grid that owns this sequencer
    pub is_sending: bool,   // when false, no OSC messages are sent
    beat_count: usize,      // the number of beats that have passed
    last_tick: Instant,     // the time of the last clock tick
    pub current_idx: usize, // this increments at the START OF BEAT, not after read.
    characters: Vec<GridCellChar>, // the characters to display
    osc_sender: OscSender,  // the OSC sender to send messages to the grid
}

impl Sequencer {
    pub fn new(id: &str, osc_config: &OscSendConfig) -> Self {
        let osc_sender = OscSender::new(osc_config).expect("Failed to create OSC Sender");
        Self {
            id: id.to_owned(),
            is_sending: true,
            beat_count: 0,
            last_tick: Instant::now(),
            current_idx: 0,
            characters: Vec::new(),
            osc_sender,
        }
    }

    // Updates the sequencer state,
    // Sends OSC out,
    // Returns the index of the current character
    pub fn update(&mut self, chars: &[GridCellChar]) {
        self.characters = chars.to_vec();

        // track the time
        self.increment();

        // process character: deconstruct the word into component jamo and send OSC message of each jamo
        if let Some(ch) = self.get_character() {
            // do nothing if not hangeul
            if hangeul::is_hangeul(ch as u32) {
                self.send_commands(decompose_character(ch));

            // handle punctuation.
            // punctuation that makes it here has been pre-filtered by the grid.
            // so we can just send it as is
            } else if ch.is_ascii_punctuation() {
                self.osc_sender
                    .send_punctuation(self.id.as_str(), ch as i32);
            }
        }
    }

    // increment is called on every beat
    fn increment(&mut self) {
        self.beat_count += 1;
        self.current_idx += 1;
        self.last_tick = Instant::now();
    }

    // get the next visible character
    fn get_character(&mut self) -> Option<char> {
        let visible_chars: Vec<_> = self.characters.iter().filter(|c| c.is_visible).collect();

        // return None if no chars are visible
        if visible_chars.is_empty() {
            return None;
        }

        // go back to the first char is index is greater than length
        if self.current_idx >= visible_chars.len() {
            self.current_idx = 0;
        }

        // return the character at the current index
        Some(visible_chars[self.current_idx].c)
    }

    // send the OSC messages
    fn send_commands(&self, chars: Vec<char>) {
        // Don't send if the self flag is false
        if !self.is_sending {
            return;
        }

        let length = chars.len();

        // if chars vec is invalid, return early
        if chars.is_empty() || length > 3 {
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

// decompose a hangeul character into a vector of component jamo
fn decompose_character(ch: char) -> Vec<char> {
    // if not a hangeul syllable,
    // return a vector with the original character
    if !hangeul::is_syllable(ch as u32) {
        return vec![ch];
    }

    // Use the hangeul crate to decompose the character into its component jamo,
    // transform the hangeul::Decomposed tuple (char, char, Option<char>) into a vector of chars
    if let Ok(result) = hangeul::decompose_char(&ch) {
        if result.2.is_none() {
            return vec![result.0, result.1];
        } else {
            return vec![result.0, result.1, result.2.unwrap()];
        }
    }
    // return an empty Vec if decomposition fails
    Vec::new()
}

// convert a hangeul character to an i32
// 12592 is the hangeul offset (we formerly subtracted the offset but no longer)
fn hangeul_to_i32(ch: char) -> i32 {
    let result = ch as u32;

    result as i32
}

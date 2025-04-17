// src/services/hangeul_parser.rs

use hangeul::*;
use rustkorean::*;

#[derive(Default, Debug)]
pub struct Karacter {
    first: Option<char>,     // current character in the stream
    second: Option<char>,    // next character in the stream
    third: Option<char>,     // the one after the next
    choseong: Option<char>,  // a possible choseong (initial)
    jungseong: Option<char>, // a possible jungseong (medial)
    jongseong: Option<char>, // a possible jongseong (final)
}

impl Karacter {
    pub fn new() -> Self {
        Self {
            first: None,
            second: None,
            third: None,
            choseong: None,
            jungseong: None,
            jongseong: None,
        }
    }

    pub fn process_char(&mut self, c: char) -> Option<char> {
        // First, fill this and next.
        if self.first.is_none() {
            if is_choseong(c as u32) {
                self.first = Some(c);
            }
            return Some(c);

        // start looking at the possibilities
        } else if self.first.is_some() && self.second.is_none() {
            return None;
        } else {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process() {
        let mut input = 'ㅏ';
        let new_string = hangeul::is_moeum(12623);
        println!("moeum: {:?}", new_string);
    }
}

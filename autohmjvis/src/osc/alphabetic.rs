//! Custom translation for alphabetic characters
//!
//! This sends alphabetic characters as if they were hangeul

pub fn to_hangeul_components(letter: char) -> Option<char> {
    let letter = letter.to_ascii_uppercase();

    match letter {
        'A' => Some('ㅏ'),
        'B' => Some('ㅂ'),
        'C' => Some('ㅆ'),
        'D' => Some('ㄷ'),
        'E' => Some('ㅔ'),
        'F' => Some('ㅎ'),
        'G' => Some('ㄱ'),
        'H' => Some('ㅎ'),
        'I' => Some('ㅣ'),
        'J' => Some('ㅈ'),
        'K' => Some('ㅋ'),
        'L' => Some('ㄹ'),
        'M' => Some('ㅁ'),
        'N' => Some('ㄴ'),
        'O' => Some('ㅗ'),
        'P' => Some('ㅍ'),
        'Q' => Some('ㅋ'),
        'R' => Some('ㄹ'),
        'S' => Some('ㅅ'),
        'T' => Some('ㅌ'),
        'U' => Some('ㅠ'),
        'V' => Some('ㅂ'),
        'W' => Some('ㅜ'),
        'X' => Some('ㅉ'),
        'Y' => Some('ㅣ'),
        'Z' => Some('ㅈ'),
        _ => None,
    }
}

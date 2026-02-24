//! Custom translation for alphabetic characters
//!
//! This sends alphabetic characters as if they were hangeul

pub fn to_hangeul_components(letter: char) -> Option<(i32, i32, i32)> {
    let letter = letter.to_ascii_uppercase();

    match letter {
        'A' => Some((11, 0, 0)),
        'B' => Some((0, 2, 0)),
        'C' => Some((14, 2, 1)),
        'D' => Some((0, 2, 0)),
        'E' => Some((11, 0, 0)),
        'F' => Some((1, 2, 1)),
        'G' => Some((0, 2, 0)),
        'H' => Some((18, 2, 0)),
        'I' => Some((11, 13, 0)),
        'J' => Some((0, 2, 0)),
        'K' => Some((14, 2, 1)),
        'L' => Some((5, 2, 4)),
        'M' => Some((2, 2, 4)),
        'N' => Some((2, 2, 4)),
        'O' => Some((11, 0, 0)),
        'P' => Some((14, 2, 1)),
        'Q' => Some((14, 2, 1)),
        'R' => Some((5, 2, 4)),
        'S' => Some((0, 2, 0)),
        'T' => Some((14, 2, 1)),
        'U' => Some((11, 13, 0)),
        'V' => Some((1, 2, 1)),
        'W' => Some((2, 2, 4)),
        'X' => Some((0, 2, 0)),
        'Y' => Some((11, 13, 0)),
        'Z' => Some((1, 2, 1)),
        _ => None,
    }
}

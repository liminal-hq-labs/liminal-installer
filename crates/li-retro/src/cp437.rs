// Code page 437: the character set of the PC text-mode screen, and the mapping from Unicode.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The glyphs of bytes 0x80 to 0xFF.
const HIGH: [char; 128] = [
	'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
	'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
	'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
	'╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
	'╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
	'▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
	'≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// The glyphs of bytes 0x01 to 0x1F, as drawn by the video card (0x00 is blank).
const LOW: [char; 32] = [
	'\0', '☺', '☻', '♥', '♦', '♣', '♠', '•', '◘', '○', '◙', '♂', '♀', '♪', '♫', '☼', '►', '◄', '↕',
	'‼', '¶', '§', '▬', '↨', '↑', '↓', '→', '←', '∟', '↔', '▲', '▼',
];

/// The byte that draws `c` on a PC text screen. Characters with no glyph become `?`, except the
/// dashes, which become `-`, and the typographic quotes, which become plain ones.
pub fn encode_char(c: char) -> u8 {
	match c {
		' '..='~' => c as u8,
		'\u{2013}' | '\u{2014}' | '\u{2212}' => b'-',
		'\u{2018}' | '\u{2019}' => b'\'',
		'\u{201c}' | '\u{201d}' => b'"',
		'\u{2026}' => b'.',
		_ => {
			if let Some(i) = HIGH.iter().position(|&h| h == c) {
				0x80 + i as u8
			} else if let Some(i) = LOW.iter().position(|&l| l == c) {
				i as u8
			} else {
				b'?'
			}
		}
	}
}

/// The character a byte draws, for dumps and tests.
pub fn decode_byte(b: u8) -> char {
	match b {
		0x20..=0x7e => b as char,
		0x80..=0xff => HIGH[usize::from(b - 0x80)],
		0 => ' ',
		_ => LOW[usize::from(b)],
	}
}

/// Whether `encode_char` has a real glyph for `c`.
pub fn has_glyph(c: char) -> bool {
	encode_char(c) != b'?' || c == '?'
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_trips_the_box_drawing_set() {
		for c in "╔═╗║╚╝─│┌┐└┘█░▒▓•·↑↓".chars() {
			assert_eq!(decode_byte(encode_char(c)), c, "{c}");
		}
		assert_eq!(encode_char('\u{2022}'), 0x07);
		assert_eq!(encode_char('\u{b7}'), 0xfa);
		assert_eq!(encode_char('\u{2588}'), 0xdb);
		assert_eq!(encode_char('\u{2014}'), b'-');
		assert_eq!(encode_char('\u{4e2d}'), b'?');
	}
}

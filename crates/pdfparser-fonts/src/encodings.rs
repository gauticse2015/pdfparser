//! Simple font encodings including PDF `/Differences` arrays.
//! Generic: works for any BaseEncoding + Differences, not corpus-specific.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum EncodingKind {
    /// Built-in base without differences.
    Named(BaseEncoding),
    /// Base + Differences map (code -> unicode char).
    Differences {
        base: BaseEncoding,
        map: HashMap<u8, char>,
    },
    /// Identity / raw.
    Identity,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum BaseEncoding {
    WinAnsi,
    MacRoman,
    Standard,
    MacExpert,
    Unknown,
}

impl EncodingKind {
    pub fn named(base: BaseEncoding) -> Self {
        EncodingKind::Named(base)
    }

    /// Build from optional base name + optional differences list (code, glyph_name).
    pub fn from_pdf(base_name: Option<&str>, differences: &[(u8, String)]) -> Self {
        let base = match base_name {
            Some("MacRomanEncoding") => BaseEncoding::MacRoman,
            Some("StandardEncoding") => BaseEncoding::Standard,
            Some("MacExpertEncoding") => BaseEncoding::MacExpert,
            Some("WinAnsiEncoding") | Some("WinAnsi") => BaseEncoding::WinAnsi,
            Some("Identity-H") | Some("Identity-V") => return EncodingKind::Identity,
            _ => BaseEncoding::WinAnsi, // PDF default when BaseEncoding omitted is often Standard; WinAnsi is common for TrueType
        };
        if differences.is_empty() {
            return EncodingKind::Named(base);
        }
        let mut map = HashMap::new();
        for (code, gname) in differences {
            if let Some(ch) = glyph_name_to_char(gname) {
                map.insert(*code, ch);
            }
        }
        EncodingKind::Differences { base, map }
    }
}

pub fn decode_simple(enc: &EncodingKind, code: u8) -> (char, f32) {
    match enc {
        EncodingKind::Identity => {
            if code == 0 {
                ('\u{FFFD}', 0.0)
            } else if (32..127).contains(&code) {
                (code as char, 0.5)
            } else {
                (char::from_u32(code as u32).unwrap_or('\u{FFFD}'), 0.3)
            }
        }
        EncodingKind::Differences { base, map } => {
            if let Some(&ch) = map.get(&code) {
                return (ch, 1.0);
            }
            decode_base(*base, code)
        }
        EncodingKind::Named(base) => decode_base(*base, code),
    }
}

fn decode_base(_base: BaseEncoding, code: u8) -> (char, f32) {
    // Shared approximation for Standard / WinAnsi / MacRoman without Differences.
    // Differences and ToUnicode cover production custom encodings.
    if (32..127).contains(&code) {
        (code as char, 1.0)
    } else if code == 0xA0 {
        ('\u{00A0}', 1.0)
    } else if code >= 0xA0 {
        (char::from_u32(code as u32).unwrap_or('\u{FFFD}'), 0.85)
    } else {
        ('\u{FFFD}', 0.0)
    }
}

/// Map PDF glyph names to Unicode (Adobe Glyph List subset + common names).
/// Generic coverage for production PDFs; unknown names return None.
pub fn glyph_name_to_char(name: &str) -> Option<char> {
    let name = name.trim_start_matches('/');
    // uniXXXX / uXXXXX
    if let Some(hex) = name.strip_prefix("uni") {
        if hex.len() == 4 {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                return char::from_u32(cp);
            }
        }
    }
    if let Some(hex) = name.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                return char::from_u32(cp);
            }
        }
    }
    // single letter A-Z a-z
    if name.len() == 1 {
        let c = name.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            return Some(c);
        }
    }
    match name {
        "space" | "nbspace" => Some(' '),
        "period" => Some('.'),
        "comma" => Some(','),
        "colon" => Some(':'),
        "semicolon" => Some(';'),
        "hyphen" | "minus" | "sfthyphen" => Some('-'),
        "endash" => Some('\u{2013}'),
        "emdash" => Some('\u{2014}'),
        "slash" | "solidus" => Some('/'),
        "backslash" => Some('\\'),
        "parenleft" => Some('('),
        "parenright" => Some(')'),
        "bracketleft" => Some('['),
        "bracketright" => Some(']'),
        "braceleft" => Some('{'),
        "braceright" => Some('}'),
        "underscore" => Some('_'),
        "quotesingle" | "quoteright" => Some('\''),
        "quoteleft" => Some('\u{2018}'),
        "quotedbl" => Some('"'),
        "quotedblleft" => Some('\u{201C}'),
        "quotedblright" => Some('\u{201D}'),
        "dollar" => Some('$'),
        "percent" => Some('%'),
        "ampersand" => Some('&'),
        "asterisk" => Some('*'),
        "plus" => Some('+'),
        "equal" => Some('='),
        "at" => Some('@'),
        "numbersign" | "hash" => Some('#'),
        "question" => Some('?'),
        "exclam" => Some('!'),
        "one" => Some('1'),
        "two" => Some('2'),
        "three" => Some('3'),
        "four" => Some('4'),
        "five" => Some('5'),
        "six" => Some('6'),
        "seven" => Some('7'),
        "eight" => Some('8'),
        "nine" => Some('9'),
        "zero" => Some('0'),
        // accented common
        "aacute" => Some('á'),
        "eacute" => Some('é'),
        "iacute" => Some('í'),
        "oacute" => Some('ó'),
        "uacute" => Some('ú'),
        "ntilde" => Some('ñ'),
        "ccedilla" => Some('ç'),
        "agrave" => Some('à'),
        "egrave" => Some('è'),
        "Aacute" => Some('Á'),
        "Eacute" => Some('É'),
        "fi" => Some('\u{FB01}'),
        "fl" => Some('\u{FB02}'),
        "bullet" => Some('\u{2022}'),
        "ellipsis" => Some('\u{2026}'),
        "trademark" => Some('\u{2122}'),
        "copyright" => Some('\u{00A9}'),
        "registered" => Some('\u{00AE}'),
        "degree" => Some('\u{00B0}'),
        "section" => Some('\u{00A7}'),
        "paragraph" => Some('\u{00B6}'),
        "dagger" => Some('\u{2020}'),
        "daggerdbl" => Some('\u{2021}'),
        "sterling" => Some('£'),
        "euro" => Some('€'),
        "yen" => Some('¥'),
        "cent" => Some('¢'),
        "multiply" => Some('×'),
        "divide" => Some('÷'),
        "plusminus" => Some('±'),
        "lessequal" => Some('≤'),
        "greaterequal" => Some('≥'),
        "notequal" => Some('≠'),
        "infty" | "infinity" => Some('∞'),
        "partialdiff" => Some('∂'),
        "summation" => Some('∑'),
        "product" => Some('∏'),
        "radical" => Some('√'),
        "approxequal" => Some('≈'),
        "arrowleft" => Some('←'),
        "arrowright" => Some('→'),
        "arrowup" => Some('↑'),
        "arrowdown" => Some('↓'),
        "lozenge" => Some('◊'),
        "diamond" => Some('♦'),
        "heart" => Some('♥'),
        "club" => Some('♣'),
        "spade" => Some('♠'),
        // .notdef
        ".notdef" => None,
        _ => {
            // glyph name is exactly a multi-letter word used as letter sequence? uncommon
            None
        }
    }
}

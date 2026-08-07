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
            None => BaseEncoding::WinAnsi,
            _ => BaseEncoding::Unknown,
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

/// Mac OS Roman 0x80–0xFF (Adobe MacRomanEncoding).
const MAC_ROMAN_HIGH: [char; 128] = [
    '\u{00C4}', '\u{00C5}', '\u{00C7}', '\u{00C9}', '\u{00D1}', '\u{00D6}', '\u{00DC}', '\u{00E1}',
    '\u{00E0}', '\u{00E2}', '\u{00E4}', '\u{00E3}', '\u{00E5}', '\u{00E7}', '\u{00E9}', '\u{00E8}',
    '\u{00EA}', '\u{00EB}', '\u{00ED}', '\u{00EC}', '\u{00EE}', '\u{00EF}', '\u{00F1}', '\u{00F3}',
    '\u{00F2}', '\u{00F4}', '\u{00F6}', '\u{00F5}', '\u{00FA}', '\u{00F9}', '\u{00FB}', '\u{00FC}',
    '\u{2020}', '\u{00B0}', '\u{00A2}', '\u{00A3}', '\u{00A7}', '\u{2022}', '\u{00B6}', '\u{00DF}',
    '\u{00AE}', '\u{00A9}', '\u{2122}', '\u{00B4}', '\u{00A8}', '\u{2260}', '\u{00C6}', '\u{00D8}',
    '\u{221E}', '\u{00B1}', '\u{2264}', '\u{2265}', '\u{00A5}', '\u{00B5}', '\u{2202}', '\u{2211}',
    '\u{220F}', '\u{03C0}', '\u{222B}', '\u{00AA}', '\u{00BA}', '\u{03A9}', '\u{00E6}', '\u{00F8}',
    '\u{00BF}', '\u{00A1}', '\u{00AC}', '\u{221A}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00AB}',
    '\u{00BB}', '\u{2026}', '\u{00A0}', '\u{00C0}', '\u{00C3}', '\u{00D5}', '\u{0152}', '\u{0153}',
    '\u{2013}', '\u{2014}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{00F7}', '\u{25CA}',
    '\u{00FF}', '\u{0178}', '\u{2044}', '\u{20AC}', '\u{2039}', '\u{203A}', '\u{FB01}', '\u{FB02}',
    '\u{2021}', '\u{00B7}', '\u{201A}', '\u{201E}', '\u{2030}', '\u{00C2}', '\u{00CA}', '\u{00C1}',
    '\u{00CB}', '\u{00C8}', '\u{00CD}', '\u{00CE}', '\u{00CF}', '\u{00CC}', '\u{00D3}', '\u{00D4}',
    '\u{F8FF}', '\u{00D2}', '\u{00DA}', '\u{00DB}', '\u{00D9}', '\u{0131}', '\u{02C6}', '\u{02DC}',
    '\u{00AF}', '\u{02D8}', '\u{02D9}', '\u{02DA}', '\u{00B8}', '\u{02DD}', '\u{02DB}', '\u{02C7}',
];

fn decode_base(base: BaseEncoding, code: u8) -> (char, f32) {
    if matches!(base, BaseEncoding::MacExpert) {
        let ch = crate::mac_expert::MAC_EXPERT[code as usize];
        let conf = if ch == '\u{FFFD}' { 0.0 } else { 1.0 };
        return (ch, conf);
    }
    if (32..127).contains(&code) {
        return (code as char, 1.0);
    }
    match base {
        BaseEncoding::MacRoman if code >= 0x80 => (MAC_ROMAN_HIGH[(code - 0x80) as usize], 1.0),
        BaseEncoding::MacExpert => unreachable!("handled above"),
        _ if code == 0xA0 => ('\u{00A0}', 1.0),
        _ if code >= 0xA0 => (char::from_u32(code as u32).unwrap_or('\u{FFFD}'), 0.85),
        _ => ('\u{FFFD}', 0.0),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_expert_distinct_from_mac_roman() {
        let ex = EncodingKind::Named(BaseEncoding::MacExpert);
        let mr = EncodingKind::Named(BaseEncoding::MacRoman);
        // 0x61: MacRoman 'a' vs MacExpert Asmall → 'A'
        let (ex_a, _) = decode_simple(&ex, 0x61);
        let (mr_a, _) = decode_simple(&mr, 0x61);
        assert_eq!(mr_a, 'a');
        assert_eq!(ex_a, 'A');
        let (ex80, _) = decode_simple(&ex, 0x80);
        let (mr80, _) = decode_simple(&mr, 0x80);
        assert_ne!(ex80, mr80);
    }

    #[test]
    fn unknown_base_encoding_for_mystery_names() {
        let enc = EncodingKind::from_pdf(Some("NotARealEncoding"), &[]);
        match enc {
            EncodingKind::Named(BaseEncoding::Unknown) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }
}

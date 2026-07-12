//! Font loading: encodings, widths, ToUnicode (simple + Type0 subset).
#![deny(missing_docs)]

mod encodings;
mod tounicode;

use encodings::{decode_simple, BaseEncoding, EncodingKind};
use thiserror::Error;
pub use tounicode::ToUnicodeMap;

/// Font errors.
#[derive(Debug, Error)]
pub enum FontError {
    /// Bad input.
    #[error("font error: {0}")]
    Message(String),
}

/// Loaded font for content interpretation.
#[derive(Debug, Clone)]
pub struct LoadedFont {
    /// Base font name.
    pub name: String,
    /// Simple vs composite.
    pub is_cid: bool,
    /// Encoding for simple fonts.
    encoding: EncodingKind,
    /// Widths by char code (simple) or default.
    widths: Vec<f32>,
    /// First char for widths array.
    #[allow(dead_code)]
    first_char: u8,
    /// Missing width.
    missing_width: f32,
    /// Default CID width.
    default_width: f32,
    /// CID widths sparse map as ranges is simplified to HashMap via vec index for Identity.
    cid_widths: Vec<(u32, u32, f32)>, // (start, end, width) inclusive
    /// ToUnicode.
    to_unicode: Option<ToUnicodeMap>,
    /// Ascent / descent design units.
    pub ascent: f32,
    /// Descent (typically negative).
    pub descent: f32,
    /// Horizontal scale space for Tw.
    pub space_code: Option<u32>,
}

impl LoadedFont {
    /// Construct a simple Latin font with WinAnsi and uniform widths.
    pub fn simple_latin(name: &str) -> Self {
        let mut widths = vec![500.0; 256];
        // approximate Helvetica widths for common glyphs — enough for layout
        for (c, w) in widths.iter_mut().enumerate().take(127).skip(32) {
            *w = match c as u8 {
                b'i' | b'l' | b'I' | b'j' | b't' | b'f' => 278.0,
                b'W' | b'M' => 833.0,
                b' ' => 278.0,
                _ => 556.0,
            };
        }
        Self {
            name: name.to_string(),
            is_cid: false,
            encoding: EncodingKind::named(BaseEncoding::WinAnsi),
            widths,
            first_char: 0,
            missing_width: 500.0,
            default_width: 1000.0,
            cid_widths: Vec::new(),
            to_unicode: None,
            ascent: 800.0,
            descent: -200.0,
            space_code: Some(0x20),
        }
    }

    /// Build from PDF font dictionary fields (bytes-in API).
    pub fn from_parts(parts: FontParts<'_>) -> Result<Self, FontError> {
        let mut font = if parts.subtype.contains("Type0") || parts.subtype.contains("CID") {
            Self {
                name: parts.base_font.unwrap_or("CIDFont").to_string(),
                is_cid: true,
                encoding: EncodingKind::Identity,
                widths: Vec::new(),
                first_char: 0,
                missing_width: 1000.0,
                default_width: parts.dw.unwrap_or(1000.0),
                cid_widths: parts.w_ranges.clone(),
                to_unicode: None,
                ascent: parts.ascent.unwrap_or(800.0),
                descent: parts.descent.unwrap_or(-200.0),
                space_code: None,
            }
        } else {
            let first = parts.first_char.unwrap_or(0);
            let mut widths = vec![parts.missing_width.unwrap_or(500.0); 256];
            if let Some(w) = &parts.widths {
                for (i, width) in w.iter().enumerate() {
                    let idx = first as usize + i;
                    if idx < 256 {
                        widths[idx] = *width;
                    }
                }
            }
            let enc = EncodingKind::from_pdf(parts.encoding_name.as_deref(), &parts.differences);
            Self {
                name: parts.base_font.unwrap_or("Font").to_string(),
                is_cid: false,
                encoding: enc,
                widths,
                first_char: first,
                missing_width: parts.missing_width.unwrap_or(500.0),
                default_width: 1000.0,
                cid_widths: Vec::new(),
                to_unicode: None,
                ascent: parts.ascent.unwrap_or(800.0),
                descent: parts.descent.unwrap_or(-200.0),
                space_code: Some(0x20),
            }
        };

        if let Some(bytes) = parts.to_unicode_bytes {
            if let Ok(map) = ToUnicodeMap::parse(bytes) {
                font.to_unicode = Some(map);
            }
        }
        Ok(font)
    }

    /// Width of a character code or CID in glyph space (1000 units).
    pub fn width(&self, code: u32) -> f32 {
        if self.is_cid {
            for (a, b, w) in &self.cid_widths {
                if code >= *a && code <= *b {
                    return *w;
                }
            }
            self.default_width
        } else if (code as usize) < self.widths.len() {
            let w = self.widths[code as usize];
            if w > 0.0 {
                w
            } else {
                self.missing_width
            }
        } else {
            self.missing_width
        }
    }

    /// Map code to unicode string.
    pub fn to_unicode(&self, code: u32) -> (String, f32) {
        if let Some(map) = &self.to_unicode {
            if let Some(s) = map.get(code) {
                return (s, 1.0);
            }
        }
        if self.is_cid {
            // Identity-H without ToUnicode: try as unicode codepoint if BMP
            if code > 0 && code < 0x110000 {
                if let Some(ch) = char::from_u32(code) {
                    if !ch.is_control() {
                        return (ch.to_string(), 0.3);
                    }
                }
            }
            return ("\u{FFFD}".into(), 0.0);
        }
        let (ch, conf) = decode_simple(&self.encoding, code as u8);
        (ch.to_string(), conf)
    }

    /// Decode a PDF string bytes into codes (simple 1-byte or CID 2-byte Identity).
    pub fn codes_from_bytes(&self, bytes: &[u8]) -> Vec<u32> {
        if self.is_cid {
            let mut out = Vec::new();
            let mut i = 0;
            while i + 1 < bytes.len() {
                let c = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
                out.push(c);
                i += 2;
            }
            if i < bytes.len() {
                out.push(bytes[i] as u32);
            }
            out
        } else {
            bytes.iter().map(|b| *b as u32).collect()
        }
    }

    /// Whether Tw applies.
    pub fn is_space_for_tw(&self, code: u32) -> bool {
        if self.space_code == Some(code) {
            return true;
        }
        let (s, _) = self.to_unicode(code);
        s == " "
    }
}

/// Input parts for font construction (push pure data).
#[derive(Debug, Default)]
pub struct FontParts<'a> {
    /// Subtype name.
    pub subtype: String,
    /// BaseFont.
    pub base_font: Option<&'a str>,
    /// Encoding name (BaseEncoding or standalone).
    pub encoding_name: Option<String>,
    /// Encoding Differences: (code, glyph name).
    pub differences: Vec<(u8, String)>,
    /// FirstChar.
    pub first_char: Option<u8>,
    /// Widths array.
    pub widths: Option<Vec<f32>>,
    /// MissingWidth.
    pub missing_width: Option<f32>,
    /// ToUnicode stream bytes.
    pub to_unicode_bytes: Option<&'a [u8]>,
    /// DW for CID.
    pub dw: Option<f32>,
    /// W ranges.
    pub w_ranges: Vec<(u32, u32, f32)>,
    /// Ascent.
    pub ascent: Option<f32>,
    /// Descent.
    pub descent: Option<f32>,
}

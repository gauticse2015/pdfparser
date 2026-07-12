//! Minimal ToUnicode CMap parser (bfchar / bfrange).
use std::collections::HashMap;
use std::fmt;

/// Error from [`ToUnicodeMap::parse`] (best-effort CMap; rarely fails today).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToUnicodeParseError;

impl fmt::Display for ToUnicodeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to parse ToUnicode CMap")
    }
}

impl std::error::Error for ToUnicodeParseError {}

/// Character-code → Unicode mapping from a PDF ToUnicode CMap stream.
#[derive(Debug, Clone, Default)]
pub struct ToUnicodeMap {
    map: HashMap<u32, String>,
}

impl ToUnicodeMap {
    /// Parse a (decoded) ToUnicode CMap byte stream.
    pub fn parse(data: &[u8]) -> Result<Self, ToUnicodeParseError> {
        let text = String::from_utf8_lossy(data);
        let mut map = HashMap::new();
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            let line = line.trim();
            if line.ends_with("beginbfchar") {
                // count may be on same line or previous — consume until endbfchar
                for l in lines.by_ref() {
                    let l = l.trim();
                    if l.contains("endbfchar") {
                        break;
                    }
                    if let Some((src, dst)) = parse_bfchar_line(l) {
                        map.insert(src, dst);
                    }
                }
            } else if line.ends_with("beginbfrange") {
                for l in lines.by_ref() {
                    let l = l.trim();
                    if l.contains("endbfrange") {
                        break;
                    }
                    parse_bfrange_line(l, &mut map);
                }
            }
        }
        Ok(Self { map })
    }

    /// Look up a character code (or CID for Type0).
    pub fn get(&self, code: u32) -> Option<String> {
        self.map.get(&code).cloned()
    }

    /// Number of mapped codes (for diagnostics/tests).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate a few entries for diagnostics.
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &String)> {
        self.map.iter()
    }
}

fn parse_hex_token(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let s = s.strip_prefix('<')?.strip_suffix('>')?;
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::new();
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}

fn bytes_to_u32(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |a, &x| (a << 8) | x as u32)
}

fn utf16be_to_string(b: &[u8]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i + 1 < b.len() {
        let u = ((b[i] as u16) << 8) | b[i + 1] as u16;
        i += 2;
        if (0xD800..=0xDBFF).contains(&u) && i + 1 < b.len() {
            let u2 = ((b[i] as u16) << 8) | b[i + 1] as u16;
            i += 2;
            if (0xDC00..=0xDFFF).contains(&u2) {
                let cp = 0x10000 + (((u as u32 - 0xD800) << 10) | (u2 as u32 - 0xDC00));
                if let Some(ch) = char::from_u32(cp) {
                    s.push(ch);
                }
                continue;
            }
        }
        if let Some(ch) = char::from_u32(u as u32) {
            s.push(ch);
        }
    }
    s
}

/// Extract `<hex>` tokens from a CMap line (handles glued forms like `<21><21><0041>`).
fn hex_tokens(l: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = l.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(j) = bytes[i + 1..].iter().position(|&b| b == b'>') {
                let end = i + 1 + j;
                out.push(l[i..=end].to_string());
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn parse_bfchar_line(l: &str) -> Option<(u32, String)> {
    let toks = hex_tokens(l);
    if toks.len() < 2 {
        // Fallback: whitespace-separated
        let mut parts = l.split_whitespace();
        let src = parse_hex_token(parts.next()?)?;
        let dst = parse_hex_token(parts.next()?)?;
        return Some((bytes_to_u32(&src), utf16be_to_string(&dst)));
    }
    let src = parse_hex_token(&toks[0])?;
    let dst = parse_hex_token(&toks[1])?;
    Some((bytes_to_u32(&src), utf16be_to_string(&dst)))
}

fn parse_bfrange_line(l: &str, map: &mut HashMap<u32, String>) {
    // Array form: <lo> <hi> [<dst>…] — skip for now if '[' present without simple triple.
    if l.contains('[') {
        return;
    }
    let toks = hex_tokens(l);
    if toks.len() < 3 {
        return;
    }
    let start = match parse_hex_token(&toks[0]) {
        Some(b) => bytes_to_u32(&b),
        None => return,
    };
    let end = match parse_hex_token(&toks[1]) {
        Some(b) => bytes_to_u32(&b),
        None => return,
    };
    if let Some(dst) = parse_hex_token(&toks[2]) {
        // Destination is UTF-16BE code unit(s). For single BMP char ranges the
        // first unit is the base codepoint (standard ToUnicode identity ranges).
        let base = if dst.len() >= 2 {
            ((dst[0] as u32) << 8) | (dst[1] as u32)
        } else {
            bytes_to_u32(&dst)
        };
        for c in start..=end {
            let cp = base + (c - start);
            if let Some(ch) = char::from_u32(cp) {
                map.insert(c, ch.to_string());
            }
        }
    }
}

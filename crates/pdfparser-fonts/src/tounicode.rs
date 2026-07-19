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
    ///
    /// Handles both line-oriented CMaps and **single-line** CMaps (common in
    /// BEA/NIPA-style Type0 fonts) where `beginbfchar` … `endbfchar` and all
    /// `<src> <dst>` pairs live on one continuous line with no newlines.
    pub fn parse(data: &[u8]) -> Result<Self, ToUnicodeParseError> {
        let text = String::from_utf8_lossy(data);
        let mut map = HashMap::new();

        // Region-based parse: independent of newlines.
        for region in cmap_regions(&text, "beginbfchar", "endbfchar") {
            parse_bfchar_region(region, &mut map);
        }
        for region in cmap_regions(&text, "beginbfrange", "endbfrange") {
            parse_bfrange_region(region, &mut map);
        }

        // Legacy line-oriented path as backup when region scan finds nothing
        // (odd formatting). Safe: only fills missing keys.
        if map.is_empty() {
            let mut lines = text.lines();
            while let Some(line) = lines.next() {
                let line = line.trim();
                if line.contains("beginbfchar") {
                    // Same-line pairs after the keyword.
                    if let Some(rest) = line.split("beginbfchar").nth(1) {
                        parse_bfchar_region(rest, &mut map);
                    }
                    for l in lines.by_ref() {
                        let l = l.trim();
                        if l.contains("endbfchar") {
                            if let Some(before) = l.split("endbfchar").next() {
                                parse_bfchar_region(before, &mut map);
                            }
                            break;
                        }
                        parse_bfchar_region(l, &mut map);
                    }
                } else if line.contains("beginbfrange") {
                    if let Some(rest) = line.split("beginbfrange").nth(1) {
                        parse_bfrange_region(rest, &mut map);
                    }
                    for l in lines.by_ref() {
                        let l = l.trim();
                        if l.contains("endbfrange") {
                            if let Some(before) = l.split("endbfrange").next() {
                                parse_bfrange_region(before, &mut map);
                            }
                            break;
                        }
                        parse_bfrange_region(l, &mut map);
                    }
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

/// Slices between `begin_kw` and `end_kw` (exclusive of keywords).
fn cmap_regions<'a>(text: &'a str, begin_kw: &str, end_kw: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(b) = rest.find(begin_kw) {
        let after = &rest[b + begin_kw.len()..];
        if let Some(e) = after.find(end_kw) {
            out.push(&after[..e]);
            rest = &after[e + end_kw.len()..];
        } else {
            out.push(after);
            break;
        }
    }
    out
}

/// Extract `<hex>` tokens (handles glued forms like `<21><21><0041>`).
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

/// Parse all `<src> <dst>` pairs in a bfchar region (any whitespace / one line).
fn parse_bfchar_region(region: &str, map: &mut HashMap<u32, String>) {
    let toks = hex_tokens(region);
    let mut i = 0;
    while i + 1 < toks.len() {
        if let (Some(src), Some(dst)) = (parse_hex_token(&toks[i]), parse_hex_token(&toks[i + 1])) {
            map.insert(bytes_to_u32(&src), utf16be_to_string(&dst));
        }
        i += 2;
    }
}

/// Parse simple `<lo> <hi> <dst>` bfrange triples (array form skipped).
fn parse_bfrange_region(region: &str, map: &mut HashMap<u32, String>) {
    // Array form: <lo> <hi> [<dst>…] — skip brackets for now.
    if region.contains('[') {
        // Still try simple triples outside brackets if any.
        for part in region.split('[') {
            if !part.contains(']') {
                parse_bfrange_simple_triples(part, map);
            }
        }
        return;
    }
    parse_bfrange_simple_triples(region, map);
}

fn parse_bfrange_simple_triples(region: &str, map: &mut HashMap<u32, String>) {
    let toks = hex_tokens(region);
    let mut i = 0;
    while i + 2 < toks.len() {
        let start = match parse_hex_token(&toks[i]) {
            Some(b) => bytes_to_u32(&b),
            None => {
                i += 1;
                continue;
            }
        };
        let end = match parse_hex_token(&toks[i + 1]) {
            Some(b) => bytes_to_u32(&b),
            None => {
                i += 1;
                continue;
            }
        };
        if let Some(dst) = parse_hex_token(&toks[i + 2]) {
            let base = if dst.len() >= 2 {
                ((dst[0] as u32) << 8) | (dst[1] as u32)
            } else {
                bytes_to_u32(&dst)
            };
            if end >= start && end - start < 10_000 {
                for c in start..=end {
                    let cp = base + (c - start);
                    if let Some(ch) = char::from_u32(cp) {
                        map.insert(c, ch.to_string());
                    }
                }
            }
            i += 3;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_line_bfchar_bea_style() {
        // Minimal single-line CMap like R010 NIPA Type0 fonts (no newlines).
        let data = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
            1 begincodespacerange <0000> <FFFF> endcodespacerange \
            4 beginbfchar <0003> <0020> <002A> <0047> <0037> <0054> <0055> <0072> endbfchar \
            endcmap CMapName currentdict /CMap defineresource pop end end";
        let map = ToUnicodeMap::parse(data).expect("parse");
        assert_eq!(map.get(0x03).as_deref(), Some(" "));
        assert_eq!(map.get(0x2A).as_deref(), Some("G")); // * CID -> G
        assert_eq!(map.get(0x37).as_deref(), Some("T")); // 7 CID -> T
        assert_eq!(map.get(0x55).as_deref(), Some("r"));
        assert!(map.len() >= 4);
    }

    #[test]
    fn parse_multiline_bfchar_still_works() {
        let data = b"2 beginbfchar\n<0041> <0041>\n<0042> <0042>\nendbfchar\n";
        let map = ToUnicodeMap::parse(data).expect("parse");
        assert_eq!(map.get(0x41).as_deref(), Some("A"));
        assert_eq!(map.get(0x42).as_deref(), Some("B"));
    }
}

//! Minimal ToUnicode CMap parser (bfchar / bfrange).
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ToUnicodeMap {
    map: HashMap<u32, String>,
}

impl ToUnicodeMap {
    pub fn parse(data: &[u8]) -> Result<Self, ()> {
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

    pub fn get(&self, code: u32) -> Option<String> {
        self.map.get(&code).cloned()
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

fn parse_bfchar_line(l: &str) -> Option<(u32, String)> {
    let mut parts = l.split_whitespace();
    let src = parse_hex_token(parts.next()?)?;
    let dst = parse_hex_token(parts.next()?)?;
    Some((bytes_to_u32(&src), utf16be_to_string(&dst)))
}

fn parse_bfrange_line(l: &str, map: &mut HashMap<u32, String>) {
    let tokens: Vec<&str> = l.split_whitespace().collect();
    if tokens.len() < 3 {
        return;
    }
    let start = match parse_hex_token(tokens[0]) {
        Some(b) => bytes_to_u32(&b),
        None => return,
    };
    let end = match parse_hex_token(tokens[1]) {
        Some(b) => bytes_to_u32(&b),
        None => return,
    };
    if tokens[2].starts_with('[') {
        // array form — skip complex for Phase T
        return;
    }
    if let Some(dst) = parse_hex_token(tokens[2]) {
        let base = bytes_to_u32(&dst);
        for c in start..=end {
            let cp = base + (c - start);
            if let Some(ch) = char::from_u32(cp) {
                map.insert(c, ch.to_string());
            }
        }
    }
}

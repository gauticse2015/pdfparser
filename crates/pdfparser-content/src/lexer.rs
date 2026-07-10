//! PDF content stream tokenizer.

#[derive(Debug, Clone)]
pub enum Token {
    Number(f32),
    Name(String),
    LiteralString(Vec<u8>),
    HexString(Vec<u8>),
    ArrayStart,
    ArrayEnd,
    DictStart,
    DictEnd,
    Operator(String),
    Boolean(bool),
    Null,
}

pub fn tokenize(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // comments
        if b == b'%' {
            while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        match b {
            b'[' => {
                tokens.push(Token::ArrayStart);
                i += 1;
            }
            b']' => {
                tokens.push(Token::ArrayEnd);
                i += 1;
            }
            b'<' if i + 1 < data.len() && data[i + 1] == b'<' => {
                tokens.push(Token::DictStart);
                i += 2;
            }
            b'>' if i + 1 < data.len() && data[i + 1] == b'>' => {
                tokens.push(Token::DictEnd);
                i += 2;
            }
            b'<' => {
                i += 1;
                let mut hex = Vec::new();
                while i < data.len() && data[i] != b'>' {
                    if data[i].is_ascii_hexdigit() {
                        hex.push(data[i]);
                    }
                    i += 1;
                }
                if i < data.len() {
                    i += 1;
                }
                tokens.push(Token::HexString(decode_hex(&hex)));
            }
            b'(' => {
                let (s, ni) = parse_literal_string(data, i);
                tokens.push(Token::LiteralString(s));
                i = ni;
            }
            b'/' => {
                i += 1;
                let start = i;
                while i < data.len() && is_name_char(data[i]) {
                    i += 1;
                }
                let name = String::from_utf8_lossy(&data[start..i]).into_owned();
                tokens.push(Token::Name(name));
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => {
                let start = i;
                if data[i] == b'+' || data[i] == b'-' {
                    i += 1;
                }
                while i < data.len() && (data[i].is_ascii_digit() || data[i] == b'.') {
                    i += 1;
                }
                // scientific
                if i < data.len() && (data[i] == b'e' || data[i] == b'E') {
                    i += 1;
                    if i < data.len() && (data[i] == b'+' || data[i] == b'-') {
                        i += 1;
                    }
                    while i < data.len() && data[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let s = String::from_utf8_lossy(&data[start..i]);
                if let Ok(n) = s.parse::<f32>() {
                    tokens.push(Token::Number(n));
                }
            }
            _ => {
                let start = i;
                while i < data.len() && !data[i].is_ascii_whitespace() && !is_delim(data[i]) {
                    i += 1;
                }
                if start == i {
                    i += 1;
                    continue;
                }
                let op = String::from_utf8_lossy(&data[start..i]).into_owned();
                match op.as_str() {
                    "true" => tokens.push(Token::Boolean(true)),
                    "false" => tokens.push(Token::Boolean(false)),
                    "null" => tokens.push(Token::Null),
                    _ => tokens.push(Token::Operator(op)),
                }
            }
        }
    }
    tokens
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_name_char(b: u8) -> bool {
    !b.is_ascii_whitespace() && !is_delim(b)
}

fn decode_hex(hex: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let h: Vec<u8> = hex
        .iter()
        .copied()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    while i + 1 < h.len() {
        let s = std::str::from_utf8(&h[i..i + 2]).unwrap_or("00");
        out.push(u8::from_str_radix(s, 16).unwrap_or(0));
        i += 2;
    }
    if i < h.len() {
        let s = format!("{}0", h[i] as char);
        out.push(u8::from_str_radix(&s, 16).unwrap_or(0));
    }
    out
}

fn parse_literal_string(data: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    // i points at '('
    i += 1;
    let mut out = Vec::new();
    let mut depth = 1;
    while i < data.len() {
        let b = data[i];
        i += 1;
        match b {
            b'(' => {
                depth += 1;
                out.push(b'(');
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                out.push(b')');
            }
            b'\\' => {
                if i >= data.len() {
                    break;
                }
                let e = data[i];
                i += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0c),
                    b'(' | b')' | b'\\' => out.push(e),
                    b'\n' | b'\r' => {
                        // line continuation
                        if e == b'\r' && i < data.len() && data[i] == b'\n' {
                            i += 1;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut v = (e - b'0') as u32;
                        for _ in 0..2 {
                            if i < data.len() && (b'0'..=b'7').contains(&data[i]) {
                                v = (v << 3) | (data[i] - b'0') as u32;
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        out.push(v as u8);
                    }
                    other => out.push(other),
                }
            }
            _ => out.push(b),
        }
    }
    (out, i)
}

//! Owned stream filter decode under governor.
use crate::error::{Error, Result};
use crate::limits::ResourceGovernor;
use flate2::read::ZlibDecoder;
use lopdf::{Dictionary, Object};
use std::io::Read;

/// Decode stream content given dict (/Filter, /DecodeParms).
pub fn decode_stream_data(
    dict: &Dictionary,
    content: &[u8],
    gov: &ResourceGovernor,
) -> Result<Vec<u8>> {
    let filters = filter_list(dict);
    let mut data = content.to_vec();
    let encoded_len = data.len() as u64;

    for filter in &filters {
        data = apply_filter(filter, &data, dict)?;
        gov.check_expand_ratio(encoded_len, data.len() as u64)?;
        gov.charge_expanded(data.len() as u64)?;
    }
    if filters.is_empty() {
        gov.charge_expanded(data.len() as u64)?;
    }
    Ok(data)
}

fn filter_list(dict: &Dictionary) -> Vec<String> {
    match dict.get(b"Filter") {
        Ok(Object::Name(n)) => vec![String::from_utf8_lossy(n).into_owned()],
        Ok(Object::Array(arr)) => arr
            .iter()
            .filter_map(|o| match o {
                Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn apply_filter(name: &str, data: &[u8], dict: &Dictionary) -> Result<Vec<u8>> {
    let n = name.trim_start_matches('/');
    match n {
        "FlateDecode" | "Fl" => decode_flate(data, dict),
        "ASCIIHexDecode" | "AHx" => decode_ascii_hex(data),
        "ASCII85Decode" | "A85" => decode_ascii85(data),
        "RunLengthDecode" | "RL" => decode_run_length(data),
        "LZWDecode" | "LZW" => decode_lzw(data),
        // DCT etc. returned as raw for non-content or later
        other => Err(Error::Unsupported(format!("filter {other}"))),
    }
}

fn decode_flate(data: &[u8], dict: &Dictionary) -> Result<Vec<u8>> {
    // Try zlib, then raw deflate
    let mut out = Vec::new();
    let mut z = ZlibDecoder::new(data);
    match z.read_to_end(&mut out) {
        Ok(_) => {}
        Err(_) => {
            out.clear();
            let mut z = flate2::read::DeflateDecoder::new(data);
            z.read_to_end(&mut out)
                .map_err(|e| Error::Syntax(format!("flate: {e}")))?;
        }
    }
    // Predictor
    if let Ok(Object::Dictionary(parms)) = dict.get(b"DecodeParms") {
        if let Ok(Object::Integer(p)) = parms.get(b"Predictor") {
            if *p >= 10 {
                out = apply_png_predictor(&out, parms)?;
            } else if *p == 2 {
                out = apply_tiff_predictor(&out, parms)?;
            }
        }
    } else if let Ok(Object::Array(arr)) = dict.get(b"DecodeParms") {
        // Use last decode parms if array
        if let Some(Object::Dictionary(parms)) = arr.last() {
            if let Ok(Object::Integer(p)) = parms.get(b"Predictor") {
                if *p >= 10 {
                    out = apply_png_predictor(&out, parms)?;
                }
            }
        }
    }
    Ok(out)
}

fn get_int(dict: &Dictionary, key: &[u8], default: i64) -> i64 {
    match dict.get(key) {
        Ok(Object::Integer(i)) => *i,
        _ => default,
    }
}

fn apply_png_predictor(data: &[u8], parms: &Dictionary) -> Result<Vec<u8>> {
    let columns = get_int(parms, b"Columns", 1) as usize;
    let colors = get_int(parms, b"Colors", 1) as usize;
    let bpc = get_int(parms, b"BitsPerComponent", 8) as usize;
    let row_bytes = (columns * colors * bpc).div_ceil(8);
    let stride = row_bytes + 1;
    if row_bytes == 0 || data.is_empty() {
        return Ok(data.to_vec());
    }
    let mut out = Vec::with_capacity(data.len());
    let mut prev = vec![0u8; row_bytes];
    let mut i = 0;
    while i + stride <= data.len() {
        let filter = data[i];
        let row = &data[i + 1..i + 1 + row_bytes];
        let mut cur = vec![0u8; row_bytes];
        match filter {
            0 => cur.copy_from_slice(row),
            1 => {
                for x in 0..row_bytes {
                    let left = if x >= colors { cur[x - colors] } else { 0 };
                    cur[x] = row[x].wrapping_add(left);
                }
            }
            2 => {
                for x in 0..row_bytes {
                    cur[x] = row[x].wrapping_add(prev[x]);
                }
            }
            3 => {
                for x in 0..row_bytes {
                    let left = if x >= colors { cur[x - colors] } else { 0 };
                    let up = prev[x];
                    cur[x] = row[x].wrapping_add(((left as u16 + up as u16) / 2) as u8);
                }
            }
            4 => {
                for x in 0..row_bytes {
                    let left = if x >= colors { cur[x - colors] } else { 0 };
                    let up = prev[x];
                    let up_left = if x >= colors { prev[x - colors] } else { 0 };
                    cur[x] = row[x].wrapping_add(paeth(left, up, up_left));
                }
            }
            _ => cur.copy_from_slice(row),
        }
        out.extend_from_slice(&cur);
        prev = cur;
        i += stride;
    }
    if i < data.len() && out.is_empty() {
        // fallback raw
        return Ok(data.to_vec());
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let pa = (p - a as i16).abs();
    let pb = (p - b as i16).abs();
    let pc = (p - c as i16).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn apply_tiff_predictor(data: &[u8], parms: &Dictionary) -> Result<Vec<u8>> {
    let columns = get_int(parms, b"Columns", 1) as usize;
    let colors = get_int(parms, b"Colors", 1) as usize;
    let row_bytes = columns * colors;
    if row_bytes == 0 {
        return Ok(data.to_vec());
    }
    let mut out = data.to_vec();
    let mut i = 0;
    while i + row_bytes <= out.len() {
        for x in colors..row_bytes {
            out[i + x] = out[i + x].wrapping_add(out[i + x - colors]);
        }
        i += row_bytes;
    }
    Ok(out)
}

fn decode_ascii_hex(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut nibble: Option<u8> = None;
    for &b in data {
        if b == b'>' {
            break;
        }
        if b.is_ascii_whitespace() {
            continue;
        }
        let v = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => continue,
        };
        if let Some(hi) = nibble {
            out.push((hi << 4) | v);
            nibble = None;
        } else {
            nibble = Some(v);
        }
    }
    if let Some(hi) = nibble {
        out.push(hi << 4);
    }
    Ok(out)
}

fn decode_ascii85(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut n = 0;
    let mut i = 0;
    let bytes = data;
    while i < bytes.len() {
        let b = bytes[i];
        i += 1;
        if b == b'~' {
            break;
        }
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(33..=117).contains(&b) {
            continue;
        }
        acc = acc * 85 + (b - 33) as u32;
        n += 1;
        if n == 5 {
            out.push((acc >> 24) as u8);
            out.push((acc >> 16) as u8);
            out.push((acc >> 8) as u8);
            out.push(acc as u8);
            acc = 0;
            n = 0;
        }
    }
    if n > 1 {
        for _ in n..5 {
            acc = acc * 85 + 84;
        }
        for k in 0..(n - 1) {
            out.push((acc >> (24 - 8 * k)) as u8);
        }
    }
    Ok(out)
}

fn decode_run_length(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = data[i] as i16;
        i += 1;
        if len == 128 {
            break;
        } else if len < 128 {
            let n = len as usize + 1;
            if i + n > data.len() {
                break;
            }
            out.extend_from_slice(&data[i..i + n]);
            i += n;
        } else {
            let n = 257 - len as usize;
            if i >= data.len() {
                break;
            }
            let b = data[i];
            i += 1;
            out.extend(std::iter::repeat(b).take(n));
        }
    }
    Ok(out)
}

fn decode_lzw(data: &[u8]) -> Result<Vec<u8>> {
    // Minimal early-change=1 LZW via weezl
    let mut decoder = weezl::decode::Decoder::new(weezl::BitOrder::Msb, 8);
    decoder
        .decode(data)
        .map_err(|e| Error::Syntax(format!("lzw: {e}")))
}

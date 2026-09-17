//! Kitty graphics protocol encoder: PNG bytes -> terminal escape sequence.
//! Works in kitty / WezTerm / foot / ghostty. The terminal renders actual pixels,
//! so the TUI shows the same visuals as the website — no character grid.

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

// a=T transmit+display, f=100 PNG, i=image id (same id = replace in place),
// c/r = display size in terminal cells, q=2 suppress responses.
// Payload is base64, split into <=4096-byte chunks (m=1 until the last).
pub fn kitty_png(png: &[u8], cols: u32, rows: u32) -> String {
    let b64 = base64_encode(png);
    let mut out = String::with_capacity(b64.len() + b64.len() / 4096 * 32);
    let mut i = 0;
    while i < b64.len() {
        let end = (i + 4096).min(b64.len());
        let m = if end == b64.len() { 0 } else { 1 };
        if i == 0 {
            out.push_str(&format!("\x1b_Ga=T,f=100,i=1,c={cols},r={rows},q=2,m={m};"));
        } else {
            out.push_str(&format!("\x1b_Gm={m};"));
        }
        out.push_str(&b64[i..end]);
        out.push_str("\x1b\\");
        i = end;
    }
    out
}

// delete all images (call on exit)
pub const KITTY_CLEAR: &str = "\x1b_Ga=d\x1b\\";

//! A PNG writer with its own small deflate (fixed Huffman codes and LZ77 matches), so the art tools need no
//! third-party crate. Output depends only on the pixels, so the same drawing always gives the same bytes.

/// Encode RGBA pixels, row by row, as a PNG file.
pub fn encode(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), w * h * 4);
    let mut raw = Vec::with_capacity(h * (w * 4 + 1));
    for row in rgba.chunks(w * 4) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bits per channel, RGBA, deflate, adaptive filters, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Bits written least significant first, as deflate wants.
struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, count: u32) {
        self.acc |= value << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Huffman codes go most significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        let rev = (0..len).fold(0, |r, i| r | (((code >> i) & 1) << (len - 1 - i)));
        self.put(rev, len);
    }

    fn literal(&mut self, v: u32) {
        match v {
            0..=143 => self.code(0x30 + v, 8),
            144..=255 => self.code(0x190 + v - 144, 9),
            256..=279 => self.code(v - 256, 7),
            _ => self.code(0xc0 + v - 280, 8),
        }
    }
}

const LEN_BASE: [u32; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits { out: vec![0x78, 0x01], acc: 0, n: 0 };
    bits.put(1, 1); // the only block
    bits.put(1, 2); // fixed Huffman codes
    const WINDOW: usize = 32768;
    const CHAIN: usize = 64;
    let mut head = vec![usize::MAX; 1 << 15];
    let mut prev = vec![usize::MAX; data.len()];
    let hash = |i: usize| -> usize {
        ((data[i] as usize) << 10 ^ (data[i + 1] as usize) << 5 ^ data[i + 2] as usize) & ((1 << 15) - 1)
    };
    let mut i = 0;
    let insert = |i: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if i + 2 < data.len() {
            let h = hash(i);
            prev[i] = head[h];
            head[h] = i;
        }
    };
    while i < data.len() {
        let (mut best_len, mut best_dist) = (0, 0);
        if i + 2 < data.len() {
            let mut cand = head[hash(i)];
            let mut tries = 0;
            while cand != usize::MAX && i - cand <= WINDOW && tries < CHAIN {
                let max = (data.len() - i).min(258);
                let len = (0..max).take_while(|&k| data[cand + k] == data[i + k]).count();
                if len > best_len {
                    (best_len, best_dist) = (len, i - cand);
                    if len == max {
                        break;
                    }
                }
                cand = prev[cand];
                tries += 1;
            }
        }
        if best_len >= 3 {
            let lc = LEN_BASE.iter().rposition(|&b| b as usize <= best_len).unwrap();
            bits.literal(257 + lc as u32);
            bits.put(best_len as u32 - LEN_BASE[lc], LEN_EXTRA[lc]);
            let dc = DIST_BASE.iter().rposition(|&b| b as usize <= best_dist).unwrap();
            bits.code(dc as u32, 5);
            bits.put(best_dist as u32 - DIST_BASE[dc], DIST_EXTRA[dc]);
            for k in i..i + best_len {
                insert(k, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            bits.literal(data[i] as u32);
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    bits.literal(256);
    if bits.n > 0 {
        bits.out.push(bits.acc as u8);
    }
    let mut out = bits.out;
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

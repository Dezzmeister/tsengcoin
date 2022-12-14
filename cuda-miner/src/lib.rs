#![cfg_attr(
    target_os = "cuda",
    no_std,
    feature(register_attr),
    register_attr(nvvm_internal)
)]

use cuda_std::prelude::*;
extern crate alloc;

type Schedule = [u32; 64];

const K: [u32; 64] = [
    0b01000010100010100010111110011000,
    0b01110001001101110100010010010001,
    0b10110101110000001111101111001111,
    0b11101001101101011101101110100101,
    0b00111001010101101100001001011011,
    0b01011001111100010001000111110001,
    0b10010010001111111000001010100100,
    0b10101011000111000101111011010101,
    0b11011000000001111010101010011000,
    0b00010010100000110101101100000001,
    0b00100100001100011000010110111110,
    0b01010101000011000111110111000011,
    0b01110010101111100101110101110100,
    0b10000000110111101011000111111110,
    0b10011011110111000000011010100111,
    0b11000001100110111111000101110100,
    0b11100100100110110110100111000001,
    0b11101111101111100100011110000110,
    0b00001111110000011001110111000110,
    0b00100100000011001010000111001100,
    0b00101101111010010010110001101111,
    0b01001010011101001000010010101010,
    0b01011100101100001010100111011100,
    0b01110110111110011000100011011010,
    0b10011000001111100101000101010010,
    0b10101000001100011100011001101101,
    0b10110000000000110010011111001000,
    0b10111111010110010111111111000111,
    0b11000110111000000000101111110011,
    0b11010101101001111001000101000111,
    0b00000110110010100110001101010001,
    0b00010100001010010010100101100111,
    0b00100111101101110000101010000101,
    0b00101110000110110010000100111000,
    0b01001101001011000110110111111100,
    0b01010011001110000000110100010011,
    0b01100101000010100111001101010100,
    0b01110110011010100000101010111011,
    0b10000001110000101100100100101110,
    0b10010010011100100010110010000101,
    0b10100010101111111110100010100001,
    0b10101000000110100110011001001011,
    0b11000010010010111000101101110000,
    0b11000111011011000101000110100011,
    0b11010001100100101110100000011001,
    0b11010110100110010000011000100100,
    0b11110100000011100011010110000101,
    0b00010000011010101010000001110000,
    0b00011001101001001100000100010110,
    0b00011110001101110110110000001000,
    0b00100111010010000111011101001100,
    0b00110100101100001011110010110101,
    0b00111001000111000000110010110011,
    0b01001110110110001010101001001010,
    0b01011011100111001100101001001111,
    0b01101000001011100110111111110011,
    0b01110100100011111000001011101110,
    0b01111000101001010110001101101111,
    0b10000100110010000111100000010100,
    0b10001100110001110000001000001000,
    0b10010000101111101111111111111010,
    0b10100100010100000110110011101011,
    0b10111110111110011010001111110111,
    0b11000110011100010111100011110010,
];

const fn u32_at(slice: &[u8], idx: usize) -> u32 {
    ((slice[idx] as u32) << 24) |
    ((slice[idx + 1] as u32) << 16) |
    ((slice[idx + 2] as u32) << 8) |
    (slice[idx + 3] as u32)
}

const fn u8s(n: u32) -> [u8; 4] {
    [
        ((n & 0xFF00_0000) >> 24) as u8,
        ((n & 0x00FF_0000) >> 16) as u8,
        ((n & 0x0000_FF00) >> 8) as u8,
        ((n & 0x0000_00FF) as u8)
    ]
}

#[kernel]
#[allow(improper_ctypes_definitions, clippy::missing_safety_doc)]
pub unsafe fn finish_hash(nonces: &[u8], prev: &[u32; 11], hash_vars: &[u32; 8], hashes: *mut u8) {
    let idx = thread::index_1d() as usize;
    let mut schedule: Schedule = [0 as u32; 64];
    let mut hash = hash_vars.clone();

    // Index into the nonce array to get the appropriate nonce
    let t = idx * 32;

    schedule[0..11].copy_from_slice(prev);
    schedule[11] = u32_at(nonces, t);
    schedule[12] = u32_at(nonces, t + 4);
    schedule[13] = u32_at(nonces, t + 8);
    schedule[14] = u32_at(nonces, t + 12);
    schedule[15] = u32_at(nonces, t + 16);

    // Initialize hash variables

    let mut a = hash[0];
    let mut b = hash[1];
    let mut c = hash[2];
    let mut d = hash[3];
    let mut e = hash[4];
    let mut f = hash[5];
    let mut g = hash[6];
    let mut h = hash[7];

    // Perform the first round of hashing

    let mut w0: u32;
    let mut w9: u32;
    let mut w1: u32;
    let mut s0: u32;
    let mut w14: u32;
    let mut s1: u32;

    let mut majority: u32;
    let mut choice: u32;
    let mut temp2: u32;
    let mut temp1: u32;

    for j in 0..48 {
        w0 = schedule[j];
        w9 = schedule[j + 9];
        w1 = schedule[j + 1];
        s0 = w1.rotate_right(7) ^ w1.rotate_right(18) ^ (w1 >> 3);
        w14 = schedule[j + 14];
        s1 = w14.rotate_right(17) ^ w14.rotate_right(19) ^ (w14 >> 10);

        schedule[j + 16] = w0.wrapping_add(s0).wrapping_add(w9).wrapping_add(s1);
    }

    for j in 0..64 {
        majority = (a & b) ^ (a & c) ^ (b & c);
        s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        choice = (e & f) ^ ((!e) & g);
        s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        temp2 = s0.wrapping_add(majority);
        temp1 = h.wrapping_add(s1).wrapping_add(choice).wrapping_add(K[j]).wrapping_add(schedule[j]);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    hash[0] = hash[0].wrapping_add(a);
    hash[1] = hash[1].wrapping_add(b);
    hash[2] = hash[2].wrapping_add(c);
    hash[3] = hash[3].wrapping_add(d);
    hash[4] = hash[4].wrapping_add(e);
    hash[5] = hash[5].wrapping_add(f);
    hash[6] = hash[6].wrapping_add(g);
    hash[7] = hash[7].wrapping_add(h);

    // Set up the schedule and hash vars for the second round of hashing

    schedule[0] = u32_at(nonces, t + 20);
    schedule[1] = u32_at(nonces, t + 24);
    schedule[2] = u32_at(nonces, t + 28);
    schedule[3] = 0x8000_0000;
    schedule[4..15].copy_from_slice(&[0; 11]);    
    schedule[15] = 0x0000_0460;

    a = hash[0];
    b = hash[1];
    c = hash[2];
    d = hash[3];
    e = hash[4];
    f = hash[5];
    g = hash[6];
    h = hash[7];

    // Perform the second round of hashing

    for j in 0..48 {
        w0 = schedule[j];
        w9 = schedule[j + 9];
        w1 = schedule[j + 1];
        s0 = w1.rotate_right(7) ^ w1.rotate_right(18) ^ (w1 >> 3);
        w14 = schedule[j + 14];
        s1 = w14.rotate_right(17) ^ w14.rotate_right(19) ^ (w14 >> 10);

        schedule[j + 16] = w0.wrapping_add(s0).wrapping_add(w9).wrapping_add(s1);
    }

    for j in 0..64 {
        majority = (a & b) ^ (a & c) ^ (b & c);
        s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        choice = (e & f) ^ ((!e) & g);
        s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        temp2 = s0.wrapping_add(majority);
        temp1 = h.wrapping_add(s1).wrapping_add(choice).wrapping_add(K[j]).wrapping_add(schedule[j]);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    hash[0] = hash[0].wrapping_add(a);
    hash[1] = hash[1].wrapping_add(b);
    hash[2] = hash[2].wrapping_add(c);
    hash[3] = hash[3].wrapping_add(d);
    hash[4] = hash[4].wrapping_add(e);
    hash[5] = hash[5].wrapping_add(f);
    hash[6] = hash[6].wrapping_add(g);
    hash[7] = hash[7].wrapping_add(h);

    // Get the hash variables out and "return" them
    
    hashes.add(t).copy_from(u8s(hash[0]).as_ptr(), 4);
    hashes.add(t + 4).copy_from(u8s(hash[1]).as_ptr(), 4);
    hashes.add(t + 8).copy_from(u8s(hash[2]).as_ptr(), 4);
    hashes.add(t + 12).copy_from(u8s(hash[3]).as_ptr(), 4);
    hashes.add(t + 16).copy_from(u8s(hash[4]).as_ptr(), 4);
    hashes.add(t + 20).copy_from(u8s(hash[5]).as_ptr(), 4);
    hashes.add(t + 24).copy_from(u8s(hash[6]).as_ptr(), 4);
    hashes.add(t + 28).copy_from(u8s(hash[7]).as_ptr(), 4);
}


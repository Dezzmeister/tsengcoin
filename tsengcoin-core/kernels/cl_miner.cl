
__constant uint K[64] = {
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
};

#define memcpy(src, dest, n) for (size_t i = 0; i < (n); i++) { dest[i] = src[i]; }

#define rotr(x, n) (((x) >> (n)) | ((x) << (32 - (n))))

uint uint_at(__constant const uchar * restrict chars, const size_t idx) {
    return 
        ((uint)(chars[idx]) << 24) |
        ((uint)(chars[idx + 1]) << 16) |
        ((uint)(chars[idx + 2]) << 8) |
        (uint)(chars[idx + 3]);
}

void copy_hash_out(__private uint * restrict hash, __global uchar * restrict hashes, size_t offset) {
    uint n;
    uchar u0, u1, u2, u3;

    for (size_t i = 0; i < 8; i++) {
        n = hash[i];
        u0 = (uchar)((n & 0xFF000000) >> 24);
        u1 = (uchar)((n & 0x00FF0000) >> 16);
        u2 = (uchar)((n & 0x0000FF00) >> 8);
        u3 = (uchar)(n & 0x000000FF);

        hashes[(i * 4) + offset] = u0;
        hashes[(i * 4) + 1 + offset] = u1;
        hashes[(i * 4) + 2 + offset] = u2;
        hashes[(i * 4) + 3 + offset] = u3;
    }
}

__kernel void finish_hash(
    __constant const uchar * restrict nonces,
    __constant const uint * restrict prev,
    __constant const uint * restrict hash_vars,
    __global uchar * restrict hashes
) {
    const size_t idx = get_global_id(0);
    uint schedule[64] = { 0 };
    uint hash[8];

    memcpy(hash_vars, hash, 8 * sizeof(uint));

    const size_t t = idx * 32;
    memcpy(prev, schedule, 11 * sizeof(uint));
    schedule[11] = uint_at(nonces, t);
    schedule[12] = uint_at(nonces, t + 4);
    schedule[13] = uint_at(nonces, t + 8);
    schedule[14] = uint_at(nonces, t + 12);
    schedule[15] = uint_at(nonces, t + 16);

    uint a = hash[0];
    uint b = hash[1];
    uint c = hash[2];
    uint d = hash[3];
    uint e = hash[4];
    uint f = hash[5];
    uint g = hash[6];
    uint h = hash[7];
    
    uint w0;
    uint w9;
    uint w1;
    uint s0;
    uint w14;
    uint s1;

    uint majority;
    uint choice;
    uint temp2;
    uint temp1;

    for (size_t j = 0; j < 48; j++) {
        w0 = schedule[j];
        w9 = schedule[j + 9];
        w1 = schedule[j + 1];
        s0 = rotr(w1, 7) ^ rotr(w1, 18) ^ (w1 >> 3);
        w14 = schedule[j + 14];
        s1 = rotr(w14, 17) ^ rotr(w14, 19) ^ (w14 >> 10);

        // Implementation defined
        schedule[j + 16] = w0 + s0 + w9 + s1;
    }

    for (size_t j = 0; j < 64; j++) {
        majority = (a & b) ^ (a & c) ^ (b & c);
        s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        choice = (e & f) ^ ((~e) & g);
        s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        temp2 = s0 + majority;
        temp1 = h + s1 + choice + K[j] + schedule[j];

        h = g;
        g = f;
        f = e;
        e = d + temp1;
        d = c;
        c = b;
        b = a;
        a = temp1 + temp2;
    }

    hash[0] += a;
    hash[1] += b;
    hash[2] += c;
    hash[3] += d;
    hash[4] += e;
    hash[5] += f;
    hash[6] += g;
    hash[7] += h;

    // Hash next block
    schedule[0] = uint_at(nonces, t + 20);
    schedule[1] = uint_at(nonces, t + 24);
    schedule[2] = uint_at(nonces, t + 28);
    schedule[3] = 0x80000000;
    schedule[4] = 0;
    schedule[5] = 0;
    schedule[6] = 0;
    schedule[7] = 0;
    schedule[8] = 0;
    schedule[9] = 0;
    schedule[10] = 0;
    schedule[11] = 0;
    schedule[12] = 0;
    schedule[13] = 0;
    schedule[14] = 0;
    schedule[15] = 0x00000460;

    a = hash[0];
    b = hash[1];
    c = hash[2];
    d = hash[3];
    e = hash[4];
    f = hash[5];
    g = hash[6];
    h = hash[7];

    for (size_t j = 0; j < 48; j++) {
        w0 = schedule[j];
        w9 = schedule[j + 9];
        w1 = schedule[j + 1];
        s0 = rotr(w1, 7) ^ rotr(w1, 18) ^ (w1 >> 3);
        w14 = schedule[j + 14];
        s1 = rotr(w14, 17) ^ rotr(w14, 19) ^ (w14 >> 10);

        schedule[j + 16] = w0 + s0 + w9 + s1;
    }

    for (size_t j = 0; j < 64; j++) {
        majority = (a & b) ^ (a & c) ^ (b & c);
        s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        choice = (e & f) ^ ((~e) & g);
        s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        temp2 = s0 + majority;
        temp1 = h + s1 + choice + K[j] + schedule[j];

        h = g;
        g = f;
        f = e;
        e = d + temp1;
        d = c;
        c = b;
        b = a;
        a = temp1 + temp2;
    }

    hash[0] += a;
    hash[1] += b;
    hash[2] += c;
    hash[3] += d;
    hash[4] += e;
    hash[5] += f;
    hash[6] += g;
    hash[7] += h;

    copy_hash_out(hash, hashes, t);
}
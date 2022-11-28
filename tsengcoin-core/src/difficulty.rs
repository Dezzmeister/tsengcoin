use num_bigint::BigUint;
use num_traits::One;

use crate::wallet::Hash256;

type Minute = usize;

/// TODO: Difficulty retargeting
/// How often a block should be found
pub const TARGET_BLOCK_INTERVAL: Minute = 4;
/// After how many blocks should the difficulty be recalculated
pub const NUM_BLOCKS_RETARGET: usize = 100;

pub fn get_difficulty_target(difficulty_bits: u32) -> Hash256 {
    let raw_exp = (difficulty_bits >> 24) as u8;
    let coeff = difficulty_bits & 0x00FF_FFFF;
    let exp = 8 * (raw_exp - 3);

    let target_int = (BigUint::one() << exp) * coeff;
    let bytes = target_int.to_bytes_be();
    let mut out = [0 as u8; 32];

    out[(32 - bytes.len())..].copy_from_slice(&bytes);

    out
}

use num_bigint::BigUint;

use crate::{wallet::Hash256, v1::block::Block};

type Second = usize;

/// TODO: Difficulty retargeting
/// How often a block should be found (5 minutes)
pub const TARGET_BLOCK_INTERVAL: Second = 300;
/// After how many blocks should the difficulty be recalculated
pub const NUM_BLOCKS_RETARGET: usize = 100;

pub const RETARGET_INTERVAL: usize = NUM_BLOCKS_RETARGET * TARGET_BLOCK_INTERVAL;

pub fn retarget_difficulty(old: Hash256, last_block: &Block, first_block: &Block) -> Hash256 {
    let time_interval = last_block.header.timestamp - first_block.header.timestamp;
    let secs = time_interval.num_seconds() as usize;
    let mut adjustment = secs / RETARGET_INTERVAL;

    // Clamp the adjustment if it is too big or too small, like Bitcoin does. We do this
    // to prevent massive fluctuations in the difficulty of the network.
    if adjustment < (RETARGET_INTERVAL / 4) {
        adjustment = RETARGET_INTERVAL / 4;
    } else if adjustment > (RETARGET_INTERVAL * 4) {
        adjustment = RETARGET_INTERVAL * 4;
    }

    let new_hash_uint = BigUint::from_bytes_be(&old) * adjustment;
    let bytes = new_hash_uint.to_bytes_be();
    let mut out = [0 as u8; 32];

    out[(32 - bytes.len())..].copy_from_slice(&bytes);

    out
}

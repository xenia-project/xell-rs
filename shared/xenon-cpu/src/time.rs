const TIMEBASE_FREQ: u64 = 3192000000 / 64;

use crate::intrin::mftb;
use core::time::Duration;

fn tdelay(time: u128) {
    let tgt = time.saturating_add(mftb() as u128);
    while mftb() < tgt {}
}

pub fn delay(length: Duration) {
    tdelay((length.as_nanos() * TIMEBASE_FREQ as u128) / 1_000_000_000);
}

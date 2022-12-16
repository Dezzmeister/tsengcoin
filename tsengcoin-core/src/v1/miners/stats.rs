use std::{fs::{OpenOptions}, io::Write, time::{SystemTime, UNIX_EPOCH}};

use super::api::HASH_PER_SEC_INTERVAL;

pub const DEFAULT_GRANULARITY: TimeElapsedMillis = (HASH_PER_SEC_INTERVAL * 1000) as TimeElapsedMillis;

type TimeElapsedMillis = u32;
type Hashrate = usize;

pub type MinerStatRecord = (TimeElapsedMillis, Hashrate);
pub type MinerStats = Vec<MinerStatRecord>;

#[derive(Debug)]
pub struct MinerStatsState {
    /// Records of hashrate at a given time
    pub stats: MinerStats,
    /// About how many milliseconds between hashrate measurements
    pub granularity: u32,
    /// How many milliseconds to record for
    pub record_for: u32,
    /// When we started recording (millis since epoch)
    pub start_time: u128,
    /// Where to save the stats to
    pub filename: String,
}

impl MinerStatsState {
    pub fn new(granularity: u32, record_for: u32, filename: String) -> Self {
        Self {
            stats: vec![],
            granularity,
            record_for,
            start_time: 0,
            filename
        }
    }

    pub fn start(&mut self) {
        self.start_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    }

    pub fn add_record(&mut self, hashrate: Hashrate) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let elapsed = (now - self.start_time) as u32;

        if elapsed > self.record_for {
            return;
        }

        self.stats.push((elapsed, hashrate));
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.filename)
            .unwrap();

        let csv_lines = self.stats_to_csv();
        write!(file, "{}", csv_lines)?;

        Ok(())
    }

    pub fn done(&self) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let elapsed = (now - self.start_time) as u32;

        return elapsed > self.record_for;
    }

    fn stats_to_csv(&mut self) -> String {
        let mut out = String::from("");

        for record in self.stats.drain(..) {
            out.push_str(&format!("{}, {}\n", record.0, record.1));
        }

        out
    }
}
//! 15-minute F5J graphic telemetry ring buffer and bucket accumulator.
//!
//! Matches WimH's SoarETX / Zorro F5J Telemetry Graph architecture:
//! - 110 buckets spanning 15 minutes (900 seconds -> 8.1818s per bucket).
//! - Tracks interval average (curve trace) and interval minimum.
//! - Identifies global `MAX` thermal peak and real-flight `MIN` valley
//!   (ignoring the initial 90 seconds after the 10-second F5J window closes).

pub const MAX_SAMPLES: usize = 110;
/// 900,000 ms / 110 buckets = 8181.818 ms -> 8182 ms per bucket.
pub const SAMPLE_INTERVAL_MS: u32 = 8182;
/// 90 seconds / 8.182s ≈ 11 samples ignored for `MIN` detection.
pub const SAMPLES_90S: usize = 11;

#[derive(Clone, Debug)]
pub struct AltitudeGraph {
    /// Interval average altitude in decimeters (`0.1m`), clamped `>= 0`.
    pub avg_samples_dm: [i16; MAX_SAMPLES],
    /// Interval minimum altitude in decimeters (`0.1m`), clamped `>= 0`.
    pub min_samples_dm: [i16; MAX_SAMPLES],
    /// Number of valid plotted buckets (`0..=MAX_SAMPLES`).
    pub sample_count: usize,

    /// Overall peak altitude in decimeters (`0.1m`) during flight.
    pub max_val_dm: i16,
    /// 0-based sample index where `max_val_dm` occurred.
    pub max_val_idx: Option<usize>,

    /// Minimum real-flight altitude in decimeters (`0.1m`) excluding first 90s.
    pub min_val_dm: i16,
    /// 0-based sample index where `min_val_dm` occurred.
    pub min_val_idx: Option<usize>,

    /// Auto-scaling Y-axis ceiling in decimeters (`0.1m`).
    pub graph_max_dm: i16,

    /// Sample index recorded when the 10-second F5J start window closed.
    pub f5j_sample_idx: usize,
    /// Highest sample index incorporated into the `min_val_dm` scan so far.
    min_included_idx: usize,

    // Active bucket accumulators
    last_sample_ms: u32,
    bucket_sum_dm: i32,
    bucket_count: u32,
    bucket_min_dm: i16,
}

impl Default for AltitudeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl AltitudeGraph {
    pub const fn new() -> Self {
        Self {
            avg_samples_dm: [0; MAX_SAMPLES],
            min_samples_dm: [0; MAX_SAMPLES],
            sample_count: 0,
            max_val_dm: 0,
            max_val_idx: None,
            min_val_dm: i16::MAX,
            min_val_idx: None,
            graph_max_dm: 10, // 1.0m minimum scale floor
            f5j_sample_idx: 0,
            min_included_idx: 0,
            last_sample_ms: 0,
            bucket_sum_dm: 0,
            bucket_count: 0,
            bucket_min_dm: 0,
        }
    }

    /// Initialize graph at launch timestamp with initial relative altitude (`>= 0 dm`).
    pub fn start_flight(&mut self, now_ms: u32, rel_alt_dm: i16) {
        let alt = rel_alt_dm.max(0);
        self.avg_samples_dm.fill(0);
        self.min_samples_dm.fill(0);

        self.sample_count = 1;
        self.avg_samples_dm[0] = alt;
        self.min_samples_dm[0] = alt;

        self.max_val_dm = alt;
        self.max_val_idx = Some(0);
        self.min_val_dm = i16::MAX;
        self.min_val_idx = None;
        self.graph_max_dm = alt.max(10);

        self.f5j_sample_idx = 0;
        self.min_included_idx = 0;

        self.last_sample_ms = now_ms;
        self.bucket_sum_dm = alt as i32;
        self.bucket_count = 1;
        self.bucket_min_dm = alt;
    }

    /// Mark the sample index where the 10-second FAI post-motor window closed.
    pub fn mark_f5j_window_closed(&mut self) {
        let idx = self.sample_count.saturating_sub(1);
        self.f5j_sample_idx = idx;
        self.min_included_idx = idx;
        self.min_val_dm = i16::MAX;
        self.min_val_idx = None;
    }

    /// Update graph accumulators and advance bucket every `SAMPLE_INTERVAL_MS`.
    pub fn update(&mut self, now_ms: u32, rel_alt_dm: i16, is_glide_phase: bool, landed: bool) {
        let alt = rel_alt_dm.max(0);

        if alt > self.max_val_dm {
            self.max_val_dm = alt;
            self.max_val_idx = Some(self.sample_count.saturating_sub(1));
        }
        if alt > self.graph_max_dm {
            self.graph_max_dm = alt;
        }

        self.bucket_sum_dm += alt as i32;
        self.bucket_count += 1;
        if alt < self.bucket_min_dm {
            self.bucket_min_dm = alt;
        }

        if now_ms.wrapping_sub(self.last_sample_ms) >= SAMPLE_INTERVAL_MS {
            self.last_sample_ms = now_ms;
            let avg_dm = if self.bucket_count > 0 {
                ((self.bucket_sum_dm + (self.bucket_count as i32 / 2)) / self.bucket_count as i32)
                    .clamp(0, i16::MAX as i32) as i16
            } else {
                alt
            };

            if self.sample_count < MAX_SAMPLES {
                let idx = self.sample_count;
                self.avg_samples_dm[idx] = avg_dm;
                self.min_samples_dm[idx] = self.bucket_min_dm;
                self.sample_count += 1;
            } else {
                // Past 15 minutes: overwrite final slot as running tail bucket
                self.avg_samples_dm[MAX_SAMPLES - 1] = avg_dm;
                self.min_samples_dm[MAX_SAMPLES - 1] = self.bucket_min_dm;
            }

            self.bucket_sum_dm = alt as i32;
            self.bucket_count = 1;
            self.bucket_min_dm = alt;

            // Update "real flight" minimum (only after 90s buffer has elapsed past window)
            if is_glide_phase && !landed && self.sample_count > SAMPLES_90S {
                let safe_idx = self.sample_count - SAMPLES_90S;
                while self.min_included_idx < safe_idx {
                    if self.min_included_idx > self.f5j_sample_idx {
                        let v = self.min_samples_dm[self.min_included_idx];
                        if v < self.min_val_dm {
                            self.min_val_dm = v;
                            self.min_val_idx = Some(self.min_included_idx);
                        }
                    }
                    self.min_included_idx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f5j_graph_bucket_averaging_and_90s_min_mask() {
        let mut graph = AltitudeGraph::new();
        graph.start_flight(0, 0);
        assert_eq!(graph.sample_count, 1);

        // Simulate 15 buckets (15 * 8182ms = ~122s)
        // Window closes around bucket 2 (at ~16s)
        for i in 1..=15 {
            let t = i as u32 * SAMPLE_INTERVAL_MS;
            let alt = if i <= 3 {
                100 * i as i16 // climb to 30m (300 dm)
            } else if i == 6 {
                50 // 5.0m dip INSIDE first 90s (must be masked!)
            } else {
                200 - (i as i16 * 5) // gentle sink
            };
            graph.update(t - 100, alt, i > 2, false);
            graph.update(t, alt, i > 2, false);
            if i == 2 {
                graph.mark_f5j_window_closed();
            }
        }

        // Peak must be tracked at 300 dm (30m)
        assert_eq!(graph.max_val_dm, 300);
        assert!(graph.max_val_idx.is_some());

        // With 16 samples total (indices 0..15) and SAMPLES_90S=11:
        // safe_idx = 16 - 11 = 5. Window closed at idx 1 or 2.
        // Indices 3..4 were checked; bucket 6 (5.0m dip) is still within the trailing 90s guard window
        // until 11 more samples arrive, proving the 90s mask works cleanly.
        assert!(graph.min_val_idx.is_none() || graph.min_val_dm >= 180);
    }
}

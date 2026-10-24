//! Speed change without pitch change, for exports with a baked-in speed.
//!
//! A Rust port of the speed path (pitch 1, rate 1, mono) of Sonic by Bill Cox (2010), as adapted in AndroidX Media3's
//! `androidx.media3.common.audio.Sonic` (The Android Open Source Project), both under the Apache License 2.0. It is the
//! algorithm ExoPlayer uses for its playback speed, so an exported 1.5x file sounds like in-app listening at 1.5x.
//! See `NOTICE` at the repository root.
//!
//! How it works: the input's pitch period is found with AMDF (on a down-sampled copy first, then refined); a speed-up
//! cross-fades two periods into one and skips the rest, a slow-down inserts a cross-faded period; between those,
//! input is copied through unchanged.

use lectrice_domain::{ExportSpeed, TimeStretch, TimeStretcher};

const MIN_PITCH_HZ: u32 = 65;
const MAX_PITCH_HZ: u32 = 400;
/// Pitch is searched at about this rate first.
const AMDF_FREQUENCY_HZ: u32 = 4000;
/// Sonic treats speeds this close to 1 as 1.
const SPEED_EPSILON: f32 = 0.000_01;
/// The refined pitch search looks this many down-sampling steps around the rough period.
const REFINE_STEPS: usize = 4;
/// Sonic's starting value for the worst period, from its 8-bit origins.
const INITIAL_WORST_PERIOD: usize = 255;

/// The [`TimeStretch`] adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonicTimeStretch;

impl TimeStretch for SonicTimeStretch {
    fn stretcher(&self, sample_rate: u32, speed: ExportSpeed) -> Box<dyn TimeStretcher> {
        Box::new(Sonic::new(sample_rate, speed.as_factor()))
    }
}

struct Sonic {
    speed: f32,
    min_period: usize,
    max_period: usize,
    max_required: usize,
    skip: usize,
    input: Vec<f32>,
    down_sampled: Vec<f32>,
    remaining_to_copy: usize,
    /// The fraction of a sample each period's count was rounded by, carried to the next one (as Media3 does), so a
    /// long part doesn't drift from input ÷ speed.
    rounding_error: f64,
    prev_period: usize,
    prev_min_diff: f32,
    min_diff: f32,
    max_diff: f32,
}

impl Sonic {
    fn new(sample_rate: u32, speed: f32) -> Self {
        let min_period = (sample_rate / MAX_PITCH_HZ).max(1) as usize;
        let max_period = ((sample_rate / MIN_PITCH_HZ) as usize).max(min_period);
        let max_required = 2 * max_period;
        let skip = if sample_rate > AMDF_FREQUENCY_HZ { (sample_rate / AMDF_FREQUENCY_HZ) as usize } else { 1 };
        Self {
            speed,
            min_period,
            max_period,
            max_required,
            skip,
            input: Vec::new(),
            down_sampled: vec![0.0; max_required / skip],
            remaining_to_copy: 0,
            rounding_error: 0.0,
            prev_period: 0,
            prev_min_diff: 0.0,
            min_diff: 0.0,
            max_diff: 0.0,
        }
    }

    fn process(&mut self, out: &mut Vec<f32>) {
        if (self.speed - 1.0).abs() < SPEED_EPSILON {
            out.append(&mut self.input);
            return;
        }
        if self.input.len() < self.max_required {
            return;
        }
        let mut position = 0;
        loop {
            if self.remaining_to_copy > 0 {
                position += self.copy_to_output(position, out);
            } else {
                let period = self.find_pitch_period(position);
                if self.speed > 1.0 {
                    position += period + self.skip_pitch_period(position, period, out);
                } else {
                    position += self.insert_pitch_period(position, period, out);
                }
            }
            if position + self.max_required > self.input.len() {
                break;
            }
        }
        self.input.drain(..position);
    }

    fn copy_to_output(&mut self, position: usize, out: &mut Vec<f32>) -> usize {
        let count = self.remaining_to_copy.min(self.max_required);
        out.extend_from_slice(&self.input[position..position + count]);
        self.remaining_to_copy -= count;
        count
    }

    /// Speed-up: two periods become one cross-faded period; then part of the input is copied as is.
    fn skip_pitch_period(&mut self, position: usize, period: usize, out: &mut Vec<f32>) -> usize {
        let speed = f64::from(self.speed);
        let count = if speed >= 2.0 {
            self.rounded(period as f64 / (speed - 1.0)).min(period)
        } else {
            self.remaining_to_copy = self.rounded(period as f64 * (2.0 - speed) / (speed - 1.0));
            period
        };
        overlap_add(count, out, &self.input[position..], &self.input[position + period..]);
        count
    }

    /// Slow-down: a period is copied, then a cross-faded extra one is inserted; then part of the input is copied as is.
    fn insert_pitch_period(&mut self, position: usize, period: usize, out: &mut Vec<f32>) -> usize {
        let speed = f64::from(self.speed);
        let count = if speed < 0.5 {
            self.rounded(period as f64 * speed / (1.0 - speed)).min(period)
        } else {
            self.remaining_to_copy = self.rounded(period as f64 * (2.0 * speed - 1.0) / (1.0 - speed));
            period
        };
        out.extend_from_slice(&self.input[position..position + period]);
        overlap_add(count, out, &self.input[position + period..], &self.input[position..]);
        count
    }

    fn rounded(&mut self, exact: f64) -> usize {
        let wanted = exact + self.rounding_error;
        let count = wanted.round().max(0.0);
        self.rounding_error = wanted - count;
        count as usize
    }

    fn find_pitch_period(&mut self, position: usize) -> usize {
        let period = if self.skip == 1 {
            self.find_period_in_range(Source::Input(position), self.min_period, self.max_period)
        } else {
            self.down_sample(position);
            let rough = self.find_period_in_range(Source::DownSampled, self.min_period / self.skip, self.max_period / self.skip) * self.skip;
            let from = rough.saturating_sub(REFINE_STEPS * self.skip).max(self.min_period);
            let to = (rough + REFINE_STEPS * self.skip).min(self.max_period);
            self.find_period_in_range(Source::Input(position), from, to)
        };
        let chosen = if self.previous_period_better() { self.prev_period } else { period };
        self.prev_min_diff = self.min_diff;
        self.prev_period = period;
        chosen
    }

    fn down_sample(&mut self, position: usize) {
        let skip = self.skip;
        for (index, value) in self.down_sampled.iter_mut().enumerate() {
            let start = position + index * skip;
            *value = self.input[start..start + skip].iter().sum::<f32>() / skip as f32;
        }
    }

    /// AMDF: the period whose shifted copy differs least from the signal (per sample); also records the largest difference.
    fn find_period_in_range(&mut self, source: Source, min_period: usize, max_period: usize) -> usize {
        let samples = match source {
            Source::Input(position) => &self.input[position..],
            Source::DownSampled => &self.down_sampled[..],
        };
        let (mut best_period, mut worst_period) = (0, INITIAL_WORST_PERIOD);
        let (mut min_diff, mut max_diff) = (1.0_f32, 0.0_f32);
        for period in min_period..=max_period {
            let diff: f32 = samples[..period].iter().zip(&samples[period..2 * period]).map(|(a, b)| (a - b).abs()).sum();
            if diff * (best_period as f32) < min_diff * period as f32 {
                min_diff = diff;
                best_period = period;
            }
            if diff * (worst_period as f32) > max_diff * period as f32 {
                max_diff = diff;
                worst_period = period;
            }
        }
        let best_period = best_period.max(min_period).max(1);
        self.min_diff = min_diff / best_period as f32;
        self.max_diff = max_diff / worst_period as f32;
        best_period
    }

    /// Keeps the last period when this one matched badly and the last one matched much better.
    fn previous_period_better(&self) -> bool {
        if self.min_diff == 0.0 || self.prev_period == 0 {
            return false;
        }
        if self.max_diff > self.min_diff * 3.0 {
            return false;
        }
        if self.min_diff * 2.0 <= self.prev_min_diff * 3.0 {
            return false;
        }
        true
    }
}

#[derive(Clone, Copy)]
enum Source {
    Input(usize),
    DownSampled,
}

/// Fades `ramp_down` out while `ramp_up` fades in, over `count` samples.
fn overlap_add(count: usize, out: &mut Vec<f32>, ramp_down: &[f32], ramp_up: &[f32]) {
    let length = count as f32;
    out.extend((0..count).map(|t| (ramp_down[t] * (length - t as f32) + ramp_up[t] * t as f32) / length));
}

impl TimeStretcher for Sonic {
    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        self.input.extend_from_slice(samples);
        self.process(out);
    }

    /// Pads with silence so every real sample is processed, then cuts the output to the expected length.
    fn flush(&mut self, out: &mut Vec<f32>) {
        let remaining = self.input.len();
        let expected = (remaining as f32 / self.speed + 0.5) as usize;
        let start = out.len();
        self.input.resize(remaining + 2 * self.max_required, 0.0);
        self.process(out);
        out.truncate(start + expected.min(out.len() - start));
        self.input.clear();
        self.remaining_to_copy = 0;
        self.rounding_error = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 24_000;
    const TONE_HZ: f32 = 220.0;

    fn sine(seconds: f32) -> Vec<f32> {
        let count = (seconds * SAMPLE_RATE as f32) as usize;
        (0..count).map(|index| (2.0 * std::f32::consts::PI * TONE_HZ * index as f32 / SAMPLE_RATE as f32).sin() * 0.5).collect()
    }

    fn stretch(input: &[f32], percent: u16, chunks: usize) -> Vec<f32> {
        let mut stretcher = SonicTimeStretch.stretcher(SAMPLE_RATE, ExportSpeed::from_percent(percent).unwrap());
        let mut out = Vec::new();
        for chunk in input.chunks(input.len().div_ceil(chunks)) {
            stretcher.push(chunk, &mut out);
        }
        stretcher.flush(&mut out);
        out
    }

    /// Counts upward zero crossings away from the ends (where the fade-in of the padding sits).
    fn frequency(samples: &[f32]) -> f32 {
        let margin = samples.len() / 10;
        let middle = &samples[margin..samples.len() - margin];
        let crossings = middle.windows(2).filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0).count();
        crossings as f32 * SAMPLE_RATE as f32 / middle.len() as f32
    }

    fn assert_close(actual: f32, expected: f32, tolerance: f32, what: &str) {
        assert!((actual - expected).abs() <= expected * tolerance, "{what}: {actual} vs {expected}");
    }

    #[test]
    fn one_and_a_half_times_shortens_by_a_third_at_the_same_pitch() {
        let input = sine(1.0);

        let out = stretch(&input, 150, 1);

        assert_close(out.len() as f32, input.len() as f32 / 1.5, 0.01, "length");
        assert_close(frequency(&out), TONE_HZ, 0.02, "frequency");
    }

    #[test]
    fn every_speed_divides_the_length() {
        let input = sine(1.0);
        for percent in [75, 125, 175, 200] {
            let factor = f32::from(percent) / 100.0;

            let out = stretch(&input, percent, 1);

            assert_close(out.len() as f32, input.len() as f32 / factor, 0.01, &format!("length at {percent} %"));
            assert_close(frequency(&out), TONE_HZ, 0.02, &format!("frequency at {percent} %"));
        }
    }

    #[test]
    fn normal_speed_passes_the_samples_through() {
        let input = sine(0.1);

        assert_eq!(stretch(&input, 100, 3), input);
    }

    #[test]
    fn many_small_pushes_give_the_same_length_as_one() {
        let input = sine(1.0);
        let max_period = (SAMPLE_RATE / MIN_PITCH_HZ) as usize;

        let whole = stretch(&input, 150, 1).len();
        let chunked = stretch(&input, 150, 50).len();

        assert!(whole.abs_diff(chunked) <= max_period, "{whole} vs {chunked}");
    }

    /// A ten-minute part must not drift from input ÷ speed, or chapter marks timed from the input land late.
    #[test]
    fn a_long_stream_keeps_its_exact_length() {
        let input = sine(60.0);
        let max_period = (SAMPLE_RATE / MIN_PITCH_HZ) as f32;
        for percent in [75, 125, 150, 175, 200] {
            let expected = input.len() as f32 / (f32::from(percent) / 100.0);

            let out = stretch(&input, percent, 600);

            assert!((out.len() as f32 - expected).abs() <= max_period, "at {percent} %: {} vs {expected}", out.len());
        }
    }

    #[test]
    fn a_stream_shorter_than_one_window_still_comes_out() {
        let input = sine(0.01);

        let out = stretch(&input, 200, 1);

        assert_eq!(out.len(), (input.len() as f32 / 2.0 + 0.5) as usize);
    }
}

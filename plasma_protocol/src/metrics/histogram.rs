// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::MetricAccumulator;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::ops::Add;

const BUCKET_SIZE: usize = 1;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistogramMetricAccumulator<const BUCKET_COUNT: usize> {
    /// How many samples have value 0.0-9.99, 10.0-19.99, ... ?
    #[serde(rename = "b", with = "BigArray")]
    buckets: [u32; BUCKET_COUNT],
    /// How many samples have value below the min bucket?
    #[serde(rename = "o")]
    overflow: u32,
    /// How many samples have value above the max bucket?
    #[serde(rename = "u")]
    underflow: u32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct HistogramMetricSummary<const BUCKET_COUNT: usize> {
    /// What percent samples have value 0.0-9.99, 10.0-19.99, ... ?
    #[serde(with = "BigArray")]
    buckets: [f32; BUCKET_COUNT],
    /// What percent samples have value below the min bucket?
    overflow: f32,
    /// What percent samples have value above the max bucket?
    underflow: f32,
    median: f32,
}

impl<const BUCKET_COUNT: usize> Default for HistogramMetricAccumulator<BUCKET_COUNT> {
    fn default() -> Self {
        Self {
            buckets: [0; BUCKET_COUNT],
            overflow: 0,
            underflow: 0,
        }
    }
}

impl<const BUCKET_COUNT: usize> HistogramMetricAccumulator<BUCKET_COUNT> {
    pub fn push(&mut self, sample: f32) {
        if sample < 0.0 {
            self.underflow = self.underflow.saturating_add(1);
        } else if sample > (BUCKET_COUNT * BUCKET_SIZE) as f32 {
            self.overflow = self.overflow.saturating_add(1);
        } else {
            let bucket = ((sample / BUCKET_SIZE as f32) as usize).min(BUCKET_COUNT - 1);
            self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        }
    }

    pub fn median(&self) -> f32 {
        let sum = self.buckets.iter().map(|b| *b as u64).sum::<u64>();
        let median_partial_sum = sum / 2;
        if median_partial_sum == 0 {
            return 0.0;
        }
        let mut partial_sum = 0u64;
        for (i, b) in self.buckets.iter().enumerate() {
            partial_sum += *b as u64;
            if partial_sum >= median_partial_sum {
                return i as f32
                    + (median_partial_sum as f32 - (partial_sum - *b as u64) as f32) / *b as f32;
            }
        }
        debug_assert!(false);
        self.buckets.len() as f32
    }
}

impl<const BUCKET_COUNT: usize> MetricAccumulator for HistogramMetricAccumulator<BUCKET_COUNT> {
    /// Median.
    type DataPoint = (f32,);
    type Summary = HistogramMetricSummary<BUCKET_COUNT>;

    fn summarize(&self) -> Self::Summary {
        let total = self.buckets.iter().sum::<u32>() + self.overflow + self.underflow;
        let to_percent = if total == 0 {
            0f32
        } else {
            100f32 / total as f32
        };
        HistogramMetricSummary {
            buckets: self.buckets.map(|a| a as f32 * to_percent),
            overflow: self.overflow as f32 * to_percent,
            underflow: self.underflow as f32 * to_percent,
            median: self.median(),
        }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.median(),)
    }
}

impl<const BUCKET_COUNT: usize> Add for HistogramMetricAccumulator<BUCKET_COUNT> {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        for (s, r) in self.buckets.iter_mut().zip(rhs.buckets.iter()) {
            *s = s.saturating_add(*r);
        }
        self.overflow = self.overflow.saturating_add(rhs.overflow);
        self.underflow = self.underflow.saturating_add(rhs.underflow);
        self
    }
}

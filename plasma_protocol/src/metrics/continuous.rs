// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::MetricAccumulator;
use serde::{Deserialize, Serialize};
use std::ops::Add;

/// A metric tracking a continuous value.
/// Can be aggregated by adding all fields.
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub struct ContinuousMetricAccumulator {
    #[serde(rename = "c")]
    pub count: u32,
    // These values get large, so use f64 instead of f32.
    #[serde(rename = "t")]
    pub total: f64,
    #[serde(rename = "s")]
    pub squared_total: f64,
}

impl ContinuousMetricAccumulator {
    /// Returns count as a f64, changing a 0 count to 1 to avoid dividing by zero.
    fn non_zero_count(count: u32) -> f64 {
        count.max(1) as f64
    }

    pub fn push(&mut self, sample: f32) {
        if self.count < u32::MAX {
            self.count += 1;
            self.total += sample as f64;
            self.squared_total += (sample as f64).powi(2);
        }
    }

    fn compute_average(count: u32, total: f64) -> f32 {
        (total / Self::non_zero_count(count)) as f32
    }

    pub fn average(&self) -> f32 {
        Self::compute_average(self.count, self.total)
    }

    fn compute_standard_deviation(count: u32, total: f64, squared_total: f64) -> f32 {
        let non_zero_count = Self::non_zero_count(count);
        ((squared_total / non_zero_count) - (total / non_zero_count).powi(2)).sqrt() as f32
    }

    fn standard_deviation(&self) -> f32 {
        Self::compute_standard_deviation(self.count, self.total, self.squared_total)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ContinuousMetricSummary {
    average: f32,
    standard_deviation: f32,
}

impl MetricAccumulator for ContinuousMetricAccumulator {
    type DataPoint = (f32, f32);
    type Summary = ContinuousMetricSummary;

    fn summarize(&self) -> Self::Summary {
        ContinuousMetricSummary {
            average: self.average(),
            standard_deviation: self.standard_deviation(),
        }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.average(), self.standard_deviation())
    }
}

impl Add for ContinuousMetricAccumulator {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            count: self.count.saturating_add(rhs.count),
            total: self.total + rhs.total,
            squared_total: self.squared_total + rhs.squared_total,
        }
    }
}

/// A metric combining `ContinuousMetric` and `ExtremaMetric`.
#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContinuousExtremaMetricAccumulator {
    #[serde(rename = "c")]
    pub count: u32,
    #[serde(rename = "l")]
    pub min: f32,
    #[serde(rename = "h")]
    pub max: f32,
    #[serde(rename = "t")]
    pub total: f64,
    #[serde(rename = "s")]
    pub squared_total: f64,
}

impl ContinuousExtremaMetricAccumulator {
    pub fn new(sample: f32) -> Self {
        Self {
            count: 1,
            min: sample,
            max: sample,
            squared_total: (sample as f64).powi(2),
            total: sample as f64,
        }
    }

    pub fn push(&mut self, sample: f32) {
        if self.count < u32::MAX {
            if self.count == 0 {
                self.min = sample;
                self.max = sample;
            } else {
                self.min = self.min.min(sample);
                self.max = self.max.max(sample);
            }
            self.total += sample as f64;
            self.squared_total += (sample as f64).powi(2);
            self.count += 1;
        }
    }

    /// Automatically converts to float.
    pub fn push_count(&mut self, sample: usize) {
        self.push(sample as f32);
    }

    pub fn average(&self) -> f32 {
        ContinuousMetricAccumulator::compute_average(self.count, self.total)
    }

    pub fn standard_deviation(&self) -> f32 {
        ContinuousMetricAccumulator::compute_standard_deviation(
            self.count,
            self.total,
            self.squared_total,
        )
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ContinuousExtremaMetricSummary {
    average: f32,
    standard_deviation: f32,
    min: f32,
    max: f32,
}

impl MetricAccumulator for ContinuousExtremaMetricAccumulator {
    type DataPoint = (f32, f32, f32);
    type Summary = ContinuousExtremaMetricSummary;

    fn summarize(&self) -> Self::Summary {
        ContinuousExtremaMetricSummary {
            average: self.average(),
            standard_deviation: self.standard_deviation(),
            min: self.min,
            max: self.max,
        }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.average(), self.min, self.max)
    }
}

impl Add for ContinuousExtremaMetricAccumulator {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.count == 0 {
            rhs
        } else if rhs.count == 0 {
            self
        } else {
            Self {
                count: self.count.saturating_add(rhs.count),
                min: self.min.min(rhs.min),
                max: self.max.max(rhs.max),
                total: self.total + rhs.total,
                squared_total: self.squared_total + rhs.squared_total,
            }
        }
    }
}

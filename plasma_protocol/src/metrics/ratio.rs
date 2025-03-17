// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::MetricAccumulator;
use serde::{Deserialize, Serialize};
use std::ops::Add;

/// A metric tracking the ratio of data satisfying a condition to all data.
#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioMetricAccumulator {
    /// Total population size.
    #[serde(rename = "t")]
    pub total: u32,
    /// Number meeting the condition
    #[serde(rename = "c")]
    pub count: u32,
}

impl RatioMetricAccumulator {
    pub fn new(condition: bool) -> Self {
        Self {
            count: if condition { 1 } else { 0 },
            total: 1,
        }
    }

    pub fn push(&mut self, condition: bool) {
        debug_assert!(self.count <= self.total);
        if self.total < u32::MAX {
            self.total += 1;
            if condition {
                self.count += 1;
            }
        }
    }

    /// Returns 0 if there are no data.
    fn ratio(&self) -> f32 {
        (self.count as f64 / self.total.max(1) as f64) as f32
    }

    fn percent(&self) -> f32 {
        self.ratio() * 100.0
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RatioMetricSummary {
    percent: f32,
    total: u32,
}

impl MetricAccumulator for RatioMetricAccumulator {
    type DataPoint = (f32,);
    type Summary = RatioMetricSummary;

    fn summarize(&self) -> Self::Summary {
        RatioMetricSummary {
            percent: self.percent(),
            total: self.total,
        }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.percent(),)
    }
}

impl Add for RatioMetricAccumulator {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let max = u32::MAX - rhs.total;
        Self {
            total: self.total + rhs.total.min(max),
            count: self.count + rhs.count.min(max),
        }
    }
}

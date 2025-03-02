// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::MetricAccumulator;
use hyperloglog::{HyperLogLog, Registers};
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::ops::Add;

/// A metric representing something precisely countable.
#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscreteMetricAccumulator {
    #[serde(rename = "t")]
    pub total: u32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct DiscreteMetricSummary {
    pub total: u32,
}

impl DiscreteMetricAccumulator {
    pub fn new(sample: u32) -> Self {
        Self { total: sample }
    }

    pub fn increment(&mut self) {
        self.add_multiple(1);
    }

    pub fn add_multiple(&mut self, amount: u32) {
        self.total = self.total.saturating_add(amount)
    }

    /// Automatically converts to u32.
    pub fn add_length(&mut self, amount: usize) {
        self.add_multiple(amount.min(u32::MAX as usize) as u32)
    }
}

impl MetricAccumulator for DiscreteMetricAccumulator {
    type DataPoint = (u32,);
    type Summary = DiscreteMetricSummary;

    fn summarize(&self) -> Self::Summary {
        DiscreteMetricSummary { total: self.total }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.total,)
    }
}

impl Add for DiscreteMetricAccumulator {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            total: self.total.saturating_add(rhs.total),
        }
    }
}

/// A metric tracking the maximum and minimum of something discrete.
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub struct DiscreteExtremaMetricAccumulator {
    #[serde(rename = "c")]
    pub count: u32,
    #[serde(rename = "l")]
    pub min: u32,
    #[serde(rename = "h")]
    pub max: u32,
}

impl DiscreteExtremaMetricAccumulator {
    pub fn push(&mut self, sample: u32) {
        if self.count == 0 {
            self.min = sample;
            self.max = sample;
        } else if self.count < u32::MAX {
            self.min = self.min.min(sample);
            self.max = self.max.max(sample);
            self.count += 1;
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct DiscreteExtremaMetricSummary {
    pub min: f32,
    pub max: f32,
}

impl MetricAccumulator for DiscreteExtremaMetricAccumulator {
    type DataPoint = (u32, u32);
    type Summary = Self;

    fn summarize(&self) -> Self::Summary {
        *self
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.min, self.max)
    }
}

impl Add for DiscreteExtremaMetricAccumulator {
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
            }
        }
    }
}

/// A metric tracking the maximum and minimum of something.
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub struct ExtremaMetricAccumulator {
    #[serde(rename = "c")]
    pub count: u32,
    #[serde(rename = "l")]
    pub min: f32,
    #[serde(rename = "h")]
    pub max: f32,
}

impl ExtremaMetricAccumulator {
    pub fn push(&mut self, sample: f32) {
        if self.count == 0 {
            self.min = sample;
            self.max = sample;
        } else if self.count < u32::MAX {
            self.min = self.min.min(sample);
            self.max = self.max.max(sample);
            self.count += 1;
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ExtremaMetricSummary {
    pub min: f32,
    pub max: f32,
}

impl MetricAccumulator for ExtremaMetricAccumulator {
    type DataPoint = (f32, f32);
    type Summary = Self;

    fn summarize(&self) -> Self::Summary {
        *self
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.min, self.max)
    }
}

impl Add for ExtremaMetricAccumulator {
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
            }
        }
    }
}

/// A metric representing something imprecisely countable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistinctCountMetricAccumulator<R>(
    #[serde(bound(serialize = "R: Registers", deserialize = "R: Registers"))] HyperLogLog<R>,
);

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct DistinctCountMetricSummary {
    /// The approximate distinct count.
    total: u32,
}

impl<R: Registers> Default for DistinctCountMetricAccumulator<R> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<R: Registers> DistinctCountMetricAccumulator<R> {
    pub fn insert<T: Hash>(&mut self, sample: &T) {
        self.0.insert(sample);
    }
}

impl<R: Registers> MetricAccumulator for DistinctCountMetricAccumulator<R> {
    type DataPoint = (u32,);
    type Summary = DistinctCountMetricSummary;

    fn summarize(&self) -> Self::Summary {
        DistinctCountMetricSummary {
            total: self.0.cardinality().min(u32::MAX as u64) as u32,
        }
    }

    fn data_point(&self) -> Self::DataPoint {
        (self.0.cardinality().min(u32::MAX as u64) as u32,)
    }
}

impl<R: Registers> Add for DistinctCountMetricAccumulator<R> {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.0.merge(&rhs.0);
        self
    }
}

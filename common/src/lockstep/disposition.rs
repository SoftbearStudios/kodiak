use plasma_protocol::PlayerId;

/// Alters the game logic based on whether predicting/interpolating.
// TODO: convert parts of this to an enum.
pub struct LockstepDisposition {
    pub(crate) inner: LockstepDispositionInner,
}

pub(crate) enum LockstepDispositionInner {
    GroundTruth,
    Predicting {
        perspective: Option<PlayerId>,
        additional_interpolation_prediction: bool,
    },
    LerpingCurrentPredictionToNextPrediction {
        perspective: Option<PlayerId>,
        smoothed_normalized_ticks_since_real: f32,
    },
    // TODO: this feature was a mistake...
    LerpingOldCurrentPredictionToNewCurrentPrediction,
}

impl LockstepDisposition {
    /// Is currently making an uncertain predicition.
    pub fn is_predicting(&self) -> bool {
        matches!(self.inner, LockstepDispositionInner::Predicting { .. })
    }

    /// Currently making an uncertain prediction from a player's persepctive.
    pub fn predicting(&self) -> Option<PlayerId> {
        if let LockstepDispositionInner::Predicting { perspective, .. }
        | LockstepDispositionInner::LerpingCurrentPredictionToNextPrediction {
            perspective,
            ..
        } = &self.inner
        {
            *perspective
        } else {
            None
        }
    }

    pub fn interpolation_prediction(&self) -> bool {
        matches!(
            self.inner,
            LockstepDispositionInner::Predicting {
                additional_interpolation_prediction: true,
                ..
            } | LockstepDispositionInner::LerpingCurrentPredictionToNextPrediction { .. }
        )
    }

    /// Fractional ticks since last certain/real state from server, smoothed and normalized to be
    /// centered on zero. If other players' movements are not predicted, this can help smooth them.
    /// Only present during lerping between current prediction and next prediction.
    pub fn smoothed_normalized_ticks_since_real(&self) -> Option<f32> {
        if let LockstepDispositionInner::LerpingCurrentPredictionToNextPrediction {
            smoothed_normalized_ticks_since_real,
            ..
        } = &self.inner
        {
            Some(*smoothed_normalized_ticks_since_real)
        } else {
            None
        }
    }
}

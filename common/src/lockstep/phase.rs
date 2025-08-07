use plasma_protocol::PlayerId;

/// Alters the game logic based on whether predicting/interpolating.
pub struct LockstepPhase {
    pub(crate) inner: LockstepPhaseInner,
}

pub(crate) enum LockstepPhaseInner {
    /// Applying authoritative tick from server.
    GroundTruth,
    /// Client making prediction based on local inputs but without server ticks.
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

impl LockstepPhase {
    /// Is currently making an uncertain predicition.
    pub fn is_predicting(&self) -> bool {
        matches!(self.inner, LockstepPhaseInner::Predicting { .. })
    }

    /// Currently making an uncertain prediction from a player's persepctive.
    pub fn predicting(&self) -> Option<PlayerId> {
        if let LockstepPhaseInner::Predicting { perspective, .. }
        | LockstepPhaseInner::LerpingCurrentPredictionToNextPrediction {
            perspective, ..
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
            LockstepPhaseInner::Predicting {
                additional_interpolation_prediction: true,
                ..
            } | LockstepPhaseInner::LerpingCurrentPredictionToNextPrediction { .. }
        )
    }

    /// Fractional ticks since last certain/real state from server, smoothed and normalized to be
    /// centered on zero. If other players' movements are not predicted, this can help smooth them.
    /// Only present during lerping between current prediction and next prediction.
    pub fn smoothed_normalized_ticks_since_real(&self) -> Option<f32> {
        if let LockstepPhaseInner::LerpingCurrentPredictionToNextPrediction {
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

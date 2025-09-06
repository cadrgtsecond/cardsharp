//! Implements the [Free Spaced Repetition Scheduling][fsrs] algorithm.
//! You are not expected to understand this.
//!
//! [fsrs]: https://github.com/open-spaced-repetition/free-spaced-repetition-scheduler
// This makes the code easier to read if you understand the algorithm
#![allow(clippy::many_single_char_names)]

use serde::{Deserialize, Serialize};

const WEIGHTS: [f32; 21] = [
    0.212, 1.2931, 2.3065, 8.2956, 6.4133, 0.8334, 3.0194, 0.001, 1.8722, 0.1666, 0.796, 1.4835,
    0.0614, 0.2629, 1.6483, 0.6014, 1.8729, 0.5425, 0.0912, 0.0658, 0.1542,
];

#[derive(Debug, Copy, Clone)]
pub enum Grade {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct FSRSParams {
    stability: f32,
    difficulty: f32,
}

impl FSRSParams {
    pub fn new(stability: f32, difficulty: f32) -> Self {
        Self {
            stability,
            difficulty: difficulty.clamp(1.0, 10.0),
        }
    }
    pub fn from_initial_grade(grade: Grade) -> Self {
        let w = WEIGHTS;
        let g = f32::from(grade as u8);
        // We need to use `new` here because the value of difficulty should be clamped
        // Otherwise, the difficulty for `Grade::Easy` will end up negative
        Self::new(
            w[grade as usize - 1],
            w[4] - f32::exp(w[5] * (g - 1.0)) + 1.0,
        )
    }

    pub fn update_successful(self, grade: Grade) -> Self {
        let w = WEIGHTS;
        let g = f32::from(grade as u8);
        let s = self.stability;
        let d = self.difficulty;
        let r = self.recall_probability(0.0);

        let increase_d = 11.0 - d;
        let increase_s = s.powf(-w[9]);
        let increase_r = f32::exp(w[10] * (1.0 - r) - 1.0);
        let increase = 1.0 + increase_d * increase_s * increase_r;

        let delta_d = -w[6] * (g - 3.0);

        // Linear damping
        let d1 = d + delta_d * (10.0 - d) / 9.0;
        // Mean reversion
        let d2 = w[7] * Self::from_initial_grade(Grade::Easy).difficulty + (1.0 - w[7]) * d1;

        Self::new(s * increase, d2)
    }

    pub fn update_same_day(self, grade: Grade) -> Self {
        let w = WEIGHTS;
        let g = f32::from(grade as u8);
        let s = self.stability;

        let sinc = f32::exp(w[17] * (g - 3.0 + w[18])) * s.powf(-w[19]);
        let mut s2 = s * sinc;

        if g >= 3.0 {
            s2 = s2.max(s);
        }
        Self::new(s2, self.difficulty)
    }

    /// Recall probability after `time` days
    pub fn recall_probability(self, time: f32) -> f32 {
        let w = WEIGHTS;
        let s = self.stability;

        let factor = 0.9_f32.powf(-1.0 / w[20]) - 1.0;
        (1.0 + factor * time / s).powf(-w[20])
    }
}

// Most of these are simple sanity checks, or tests against hardcoded data
// This should be fine since the algorithm shouldn't change often,
// and even if it does, it should be reimplemented rather than modified
//
// In the future, we should implement more stronger testing,
// by checking against a reference implementation,
// or against real Anki user data
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn initial_state() {
        let w = WEIGHTS;
        let grades = [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy];
        let stabilities = &w[0..4];
        let del = f32::exp(w[5]);
        let difficulties = [
            w[4],
            w[4] - del + 1.0,
            w[4] - del * del + 1.0,
            w[4] - del * del * del + 1.0,
        ];

        for i in 0..4 {
            assert_eq!(
                FSRSParams::from_initial_grade(grades[i]),
                FSRSParams::new(stabilities[i], difficulties[i])
            );
        }
    }

    #[test]
    pub fn stability() {
        let grades = [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy];
        for g in grades {
            let card = FSRSParams::from_initial_grade(g);
            // A card's stability is the number of days it takes for its recall to become 90%
            assert!((card.recall_probability(card.stability) - 0.9).abs() < 0.01);
        }
    }
}

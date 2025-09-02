//! Implements the [Free Spaced Repetition Scheduling][fsrs] algorithm.
//! You are not expected to understand this.
//!
//! [fsrs]: https://github.com/open-spaced-repetition/free-spaced-repetition-scheduler

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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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
        let g = grade;
        Self {
            stability: w[g as usize - 1],
            difficulty: w[4] - f32::exp(w[5] * (g as u8 as f32 - 1.0)) + 1.0,
        }
    }

    pub fn update_successful(self, grade: Grade) -> Self {
        let w = WEIGHTS;
        let g = grade;
        let s = self.stability;
        let d = self.difficulty;
        let r = self.recall_probability();

        let increase_d = 11.0 - d;
        let increase_s = s.powf(-w[9]);
        let increase_r = f32::exp(w[10] * (1.0 - r) - 1.0);
        let increase = 1.0 + increase_d * increase_s * increase_r;

        let delta_d = -w[6] * (g as u8 as f32 - 3.0);
        let d1 = d + delta_d * (10.0 - d) / 9.0;
        let d2 = w[7] * Self::from_initial_grade(Grade::Easy).difficulty + (1.0 - w[7]) * d1;
        Self {
            stability: s * increase,
            difficulty: d2,
        }
    }

    pub fn update_same_day(self, grade: Grade) -> Self {
        let w = WEIGHTS;
        let g = grade;
        let s = self.stability;

        let mut s2 = s * (w[17] * (g as u8 as f32 - 3.0 + w[18])).exp() * s.powf(-w[19]);
        if let Grade::Easy | Grade::Good = g
            && s2 < s
        {
            s2 = s;
        }
        Self {
            stability: s2,
            difficulty: 0.0,
        }
    }

    pub fn recall_probability(self) -> f32 {
        let w = WEIGHTS;
        1.0 + (0.9_f32.powf(-1.0 / w[20]) - 1.0).powf(-w[20])
    }
}

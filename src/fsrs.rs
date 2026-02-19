// FSRS (Free Spaced Repetition Scheduler) implementation.
// Transcribed from Borretti's 100-line implementation.

use chrono::NaiveDate;

type R = f64;
type S = f64;
type D = f64;
type T = f64;

const F: f64 = 19.0 / 81.0;
const C: f64 = -0.5;
const DESIRED_RETENTION: f64 = 0.9;

const W: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192, 1.01925,
    1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898, 0.51655, 0.6621,
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grade {
    Forgot,
    Hard,
    Good,
    Easy,
}

impl Grade {
    pub fn from_u8(n: u8) -> Option<Grade> {
        match n {
            1 => Some(Grade::Forgot),
            2 => Some(Grade::Hard),
            3 => Some(Grade::Good),
            4 => Some(Grade::Easy),
            _ => None,
        }
    }
}

impl From<Grade> for f64 {
    fn from(g: Grade) -> f64 {
        match g {
            Grade::Forgot => 1.0,
            Grade::Hard => 2.0,
            Grade::Good => 3.0,
            Grade::Easy => 4.0,
        }
    }
}

pub struct ReviewOutcome {
    pub stability: f64,
    pub difficulty: f64,
    pub due: NaiveDate,
}

fn retrievability(t: T, s: S) -> R {
    (1.0 + F * (t / s)).powf(C)
}

fn interval(s: S) -> T {
    (s / F) * (DESIRED_RETENTION.powf(1.0 / C) - 1.0)
}

fn s_0(g: Grade) -> S {
    match g {
        Grade::Forgot => W[0],
        Grade::Hard => W[1],
        Grade::Good => W[2],
        Grade::Easy => W[3],
    }
}

fn d_0(g: Grade) -> D {
    let g: f64 = g.into();
    clamp_d(W[4] - f64::exp(W[5] * (g - 1.0)) + 1.0)
}

fn clamp_d(d: D) -> D {
    d.clamp(1.0, 10.0)
}

fn s_success(d: D, s: S, r: R, g: Grade) -> S {
    let t_d = 11.0 - d;
    let t_s = s.powf(-W[9]);
    let t_r = f64::exp(W[10] * (1.0 - r)) - 1.0;
    let h = if g == Grade::Hard { W[15] } else { 1.0 };
    let b = if g == Grade::Easy { W[16] } else { 1.0 };
    let c = f64::exp(W[8]);
    let alpha = 1.0 + t_d * t_s * t_r * h * b * c;
    s * alpha
}

fn s_fail(d: D, s: S, r: R) -> S {
    let d_f = d.powf(-W[12]);
    let s_f = (s + 1.0).powf(W[13]) - 1.0;
    let r_f = f64::exp(W[14] * (1.0 - r));
    let c_f = W[11];
    let s_f = d_f * s_f * r_f * c_f;
    f64::min(s_f, s)
}

fn stability(d: D, s: S, r: R, g: Grade) -> S {
    if g == Grade::Forgot {
        s_fail(d, s, r)
    } else {
        s_success(d, s, r, g)
    }
}

fn delta_d(g: Grade) -> f64 {
    let g: f64 = g.into();
    -W[6] * (g - 3.0)
}

fn dp(d: D, g: Grade) -> f64 {
    d + delta_d(g) * ((10.0 - d) / 9.0)
}

fn difficulty(d: D, g: Grade) -> D {
    clamp_d(W[7] * d_0(Grade::Easy) + (1.0 - W[7]) * dp(d, g))
}

pub fn review_new(grade: Grade, today: NaiveDate) -> ReviewOutcome {
    let s = s_0(grade);
    let d = d_0(grade);
    let i = f64::max(interval(s).round(), 1.0);
    let due = today + chrono::Days::new(i as u64);
    ReviewOutcome {
        stability: s,
        difficulty: d,
        due,
    }
}

pub fn review_existing(
    d: f64,
    s: f64,
    days_elapsed: f64,
    grade: Grade,
    today: NaiveDate,
) -> ReviewOutcome {
    let r = retrievability(days_elapsed, s);
    let new_s = stability(d, s, r, grade);
    let new_d = difficulty(d, grade);
    let i = f64::max(interval(new_s).round(), 1.0);
    let due = today + chrono::Days::new(i as u64);
    ReviewOutcome {
        stability: new_s,
        difficulty: new_d,
        due,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrievability_at_zero() {
        let r = retrievability(0.0, 1.0);
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn interval_roundtrip() {
        // For desired retention 0.9, interval(s) should equal s
        let s = 5.0;
        let i = interval(s);
        assert!((i - s).abs() < 1e-10);
    }

    #[test]
    fn stability_increases_on_good() {
        let d = 5.0;
        let s = 3.0;
        let r = retrievability(s, s); // r = 0.9 at t = s
        let new_s = s_success(d, s, r, Grade::Good);
        assert!(new_s > s);
    }

    #[test]
    fn stability_decreases_on_forgot() {
        let d = 5.0;
        let s = 3.0;
        let r = retrievability(s, s);
        let new_s = s_fail(d, s, r);
        assert!(new_s < s);
    }

    #[test]
    fn difficulty_clamped() {
        // Repeated forgot should not push difficulty above 10
        let mut d = d_0(Grade::Forgot);
        for _ in 0..100 {
            d = difficulty(d, Grade::Forgot);
        }
        assert!(d <= 10.0);
        assert!(d >= 1.0);

        // Repeated easy should not push difficulty below 1
        let mut d = d_0(Grade::Easy);
        for _ in 0..100 {
            d = difficulty(d, Grade::Easy);
        }
        assert!(d >= 1.0);
        assert!(d <= 10.0);
    }

    #[test]
    fn review_new_produces_future_due() {
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let outcome = review_new(Grade::Good, today);
        assert!(outcome.due > today);
        assert!(outcome.stability > 0.0);
        assert!(outcome.difficulty >= 1.0);
        assert!(outcome.difficulty <= 10.0);
    }

    #[test]
    fn review_existing_good_extends_interval() {
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let first = review_new(Grade::Good, today);
        let days = (first.due - today).num_days() as f64;
        let second = review_existing(
            first.difficulty,
            first.stability,
            days,
            Grade::Good,
            first.due,
        );
        assert!(second.due > first.due);
        assert!(second.stability > first.stability);
    }
}

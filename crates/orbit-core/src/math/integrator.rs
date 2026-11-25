//! Fixed-step Runge-Kutta 4 and adaptive Dormand-Prince 5(4).
//!
//! Both integrators are deterministic: the same inputs produce bit-identical
//! trajectories on the same platform. Inputs are validated; non-finite values
//! abort with an error rather than poisoning the caller's state.

use std::fmt;

/// A first-order ODE `dy/dt = f(t, y)`.
pub trait OdeSystem {
    fn dimension(&self) -> usize;
    fn derivative(&self, t: f64, y: &[f64], out: &mut [f64]);
}

impl<F: ?Sized> OdeSystem for F
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    fn dimension(&self) -> usize {
        // The closure cannot report a dimension; callers pass slices and we
        // trust the length. This implementation is only used through helpers
        // that size buffers from the supplied state.
        0
    }

    fn derivative(&self, t: f64, y: &[f64], out: &mut [f64]) {
        self(t, y, out)
    }
}

/// Errors that stop an integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegratorError {
    /// A state, time or derivative contained NaN or infinity.
    NonFinite,
    /// The step size fell below the configured floor.
    StepTooSmall,
    /// The step budget was exhausted.
    StepLimit,
    /// A slice had the wrong length.
    Dimension,
    /// A non-positive or non-finite step was requested.
    BadStep,
}

impl fmt::Display for IntegratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegratorError::NonFinite => write!(f, "non-finite value in integration"),
            IntegratorError::StepTooSmall => write!(f, "step size below floor"),
            IntegratorError::StepLimit => write!(f, "step budget exhausted"),
            IntegratorError::Dimension => write!(f, "dimension mismatch"),
            IntegratorError::BadStep => write!(f, "invalid step size"),
        }
    }
}

impl std::error::Error for IntegratorError {}

/// Summary returned by a completed propagation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    pub steps: u32,
    pub rejected: u32,
    pub t: f64,
    pub h_last: f64,
}

impl Default for StepReport {
    fn default() -> Self {
        Self {
            steps: 0,
            rejected: 0,
            t: 0.0,
            h_last: 0.0,
        }
    }
}

/// One classical fourth-order Runge-Kutta step.
///
/// `y` is advanced in place. `scratch` must have the same length as `y`.
pub fn rk4_step<F: OdeSystem + ?Sized>(
    sys: &F,
    t: f64,
    y: &mut [f64],
    h: f64,
    scratch: &mut [f64],
) -> Result<(), IntegratorError> {
    let n = y.len();
    if scratch.len() < n || !h.is_finite() || h <= 0.0 || !t.is_finite() {
        return Err(IntegratorError::BadStep);
    }
    let mut k1 = vec![0.0; n];
    let mut k2 = vec![0.0; n];
    let mut k3 = vec![0.0; n];
    let mut k4 = vec![0.0; n];
    let mut tmp = vec![0.0; n];

    sys.derivative(t, y, &mut k1);
    for i in 0..n {
        tmp[i] = y[i] + 0.5 * h * k1[i];
    }
    sys.derivative(t + 0.5 * h, &tmp, &mut k2);
    for i in 0..n {
        tmp[i] = y[i] + 0.5 * h * k2[i];
    }
    sys.derivative(t + 0.5 * h, &tmp, &mut k3);
    for i in 0..n {
        tmp[i] = y[i] + h * k3[i];
    }
    sys.derivative(t + h, &tmp, &mut k4);

    for i in 0..n {
        let delta = h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        let next = y[i] + delta;
        if !next.is_finite() {
            return Err(IntegratorError::NonFinite);
        }
        y[i] = next;
    }
    for (slot, value) in scratch.iter_mut().zip(y.iter()) {
        *slot = *value;
    }
    Ok(())
}

/// Integrate from `t0` to `t1` with fixed RK4 steps.
pub fn integrate_rk4<F: OdeSystem + ?Sized>(
    sys: &F,
    y: &mut [f64],
    t0: f64,
    t1: f64,
    h: f64,
    on_step: &mut dyn FnMut(f64, &[f64]),
) -> Result<StepReport, IntegratorError> {
    if !t0.is_finite() || !t1.is_finite() || t1 < t0 {
        return Err(IntegratorError::BadStep);
    }
    if !h.is_finite() || h <= 0.0 {
        return Err(IntegratorError::BadStep);
    }
    let mut scratch = vec![0.0; y.len()];
    let mut t = t0;
    let mut report = StepReport::default();
    while t < t1 - 1.0e-12 {
        let step = h.min(t1 - t);
        rk4_step(sys, t, y, step, &mut scratch)?;
        t += step;
        report.steps += 1;
        report.t = t;
        report.h_last = step;
        on_step(t, y);
        if report.steps > 50_000_000 {
            return Err(IntegratorError::StepLimit);
        }
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy)]
struct DpCoefficients {
    c: [f64; 7],
    a: [[f64; 7]; 7],
    b5: [f64; 7],
    b4: [f64; 7],
}

const DP: DpCoefficients = DpCoefficients {
    c: [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0],
    a: [
        [0.0; 7],
        [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0, 0.0],
        [
            19372.0 / 6561.0,
            -25360.0 / 2187.0,
            64448.0 / 6561.0,
            -212.0 / 729.0,
            0.0,
            0.0,
            0.0,
        ],
        [
            9017.0 / 3168.0,
            -355.0 / 33.0,
            46732.0 / 5247.0,
            49.0 / 176.0,
            -5103.0 / 18656.0,
            0.0,
            0.0,
        ],
        [
            35.0 / 384.0,
            0.0,
            500.0 / 1113.0,
            125.0 / 192.0,
            -2187.0 / 6784.0,
            11.0 / 84.0,
            0.0,
        ],
    ],
    b5: [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
        0.0,
    ],
    b4: [
        5179.0 / 57600.0,
        0.0,
        7571.0 / 16695.0,
        393.0 / 640.0,
        -92097.0 / 339200.0,
        187.0 / 2100.0,
        1.0 / 40.0,
    ],
};

/// Configuration for the adaptive integrator.
#[derive(Debug, Clone, Copy)]
pub struct DormandPrince {
    pub rtol: f64,
    pub atol: f64,
    pub h_min: f64,
    pub h_max: f64,
    pub max_steps: u32,
}

impl Default for DormandPrince {
    fn default() -> Self {
        Self {
            rtol: 1.0e-10,
            atol: 1.0e-9,
            h_min: 1.0e-4,
            h_max: 600.0,
            max_steps: 500_000,
        }
    }
}

impl DormandPrince {
    /// A controller suitable for ascent: looser tolerances, short steps.
    pub fn ascent() -> Self {
        Self {
            rtol: 1.0e-9,
            atol: 1.0e-7,
            h_min: 1.0e-4,
            h_max: 2.0,
            max_steps: 200_000,
        }
    }

    /// Validate the controller configuration.
    pub fn validate(&self) -> Result<(), IntegratorError> {
        let ok = self.rtol.is_finite()
            && self.atol.is_finite()
            && self.h_min.is_finite()
            && self.h_max.is_finite()
            && self.rtol >= 0.0
            && self.atol >= 0.0
            && self.rtol + self.atol > 0.0
            && self.h_min > 0.0
            && self.h_max >= self.h_min
            && self.max_steps > 0;
        if ok {
            Ok(())
        } else {
            Err(IntegratorError::BadStep)
        }
    }

    fn error_norm(&self, y: &[f64], y5: &[f64], y4: &[f64]) -> f64 {
        let mut sum = 0.0;
        for i in 0..y.len() {
            let scale = self.atol + self.rtol * y[i].abs().max(y5[i].abs());
            let ratio = (y5[i] - y4[i]) / scale;
            sum += ratio * ratio;
        }
        (sum / y.len() as f64).sqrt()
    }

    /// Attempt one adaptive step. On success `t` and `y` are advanced.
    ///
    /// Returns the accepted step size. Rejected attempts reduce `h` and retry
    /// until the floor is reached.
    pub fn step<F: OdeSystem + ?Sized>(
        &self,
        sys: &F,
        t: &mut f64,
        y: &mut [f64],
        h: &mut f64,
    ) -> Result<f64, IntegratorError> {
        self.validate()?;
        let n = y.len();
        for value in y.iter() {
            if !value.is_finite() {
                return Err(IntegratorError::NonFinite);
            }
        }
        let mut k = vec![vec![0.0f64; n]; 7];
        let mut accepted = false;
        let mut used = 0.0;
        for _attempt in 0..64 {
            if !h.is_finite() || *h < self.h_min {
                return Err(IntegratorError::StepTooSmall);
            }
            let hh = h.min(self.h_max);
            let mut stage = vec![0.0; n];
            let mut y5 = vec![0.0; n];
            let mut y4 = vec![0.0; n];
            // k[0] is the derivative at (t, y).
            sys.derivative(*t, y, &mut k[0]);
            let mut bad = false;
            for s in 1..7 {
                for i in 0..n {
                    let mut acc = 0.0;
                    for j in 0..s {
                        acc += DP.a[s][j] * k[j][i];
                    }
                    stage[i] = y[i] + hh * acc;
                    if !stage[i].is_finite() {
                        bad = true;
                    }
                }
                if bad {
                    break;
                }
                sys.derivative(*t + DP.c[s] * hh, &stage, &mut k[s]);
            }
            if bad {
                *h *= 0.25;
                continue;
            }
            for i in 0..n {
                let mut inc5 = 0.0;
                let mut inc4 = 0.0;
                for j in 0..7 {
                    inc5 += DP.b5[j] * k[j][i];
                    inc4 += DP.b4[j] * k[j][i];
                }
                y5[i] = y[i] + hh * inc5;
                y4[i] = y[i] + hh * inc4;
                if !y5[i].is_finite() || !y4[i].is_finite() {
                    bad = true;
                }
            }
            if bad {
                *h *= 0.25;
                continue;
            }
            let err = self.error_norm(y, &y5, &y4);
            let factor = if err <= 1.0e-16 {
                5.0
            } else {
                (0.9 * err.powf(-0.2)).clamp(0.2, 5.0)
            };
            if err <= 1.0 {
                *t += hh;
                y.copy_from_slice(&y5);
                used = hh;
                *h = (hh * factor).clamp(self.h_min, self.h_max);
                accepted = true;
                break;
            }
            *h = hh * (0.9 * err.powf(-0.2)).clamp(0.1, 0.5);
        }
        if accepted {
            for value in y.iter() {
                if !value.is_finite() {
                    return Err(IntegratorError::NonFinite);
                }
            }
            Ok(used)
        } else {
            Err(IntegratorError::StepTooSmall)
        }
    }

    /// Propagate to `t_end`, calling `on_step` after every accepted step.
    ///
    /// The callback may request an early stop.
    pub fn propagate<F: OdeSystem + ?Sized>(
        &self,
        sys: &F,
        y: &mut [f64],
        t0: f64,
        t_end: f64,
        h0: f64,
        on_step: &mut dyn FnMut(f64, &[f64]) -> Flow,
    ) -> Result<StepReport, IntegratorError> {
        if t_end < t0 || !t0.is_finite() || !t_end.is_finite() {
            return Err(IntegratorError::BadStep);
        }
        let mut t = t0;
        let mut h = h0.clamp(self.h_min, self.h_max);
        let mut report = StepReport {
            t: t0,
            h_last: h,
            ..StepReport::default()
        };
        while t < t_end {
            if report.steps >= self.max_steps {
                return Err(IntegratorError::StepLimit);
            }
            // Do not overshoot the requested end time by more than one step.
            if t + h > t_end {
                h = (t_end - t).max(self.h_min);
            }
            match self.step(sys, &mut t, y, &mut h) {
                Ok(used) => {
                    report.steps += 1;
                    report.h_last = used;
                    report.t = t;
                    if on_step(t, y) == Flow::Stop {
                        return Ok(report);
                    }
                }
                Err(IntegratorError::StepTooSmall) => {
                    return Err(IntegratorError::StepTooSmall);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(report)
    }
}

/// Whether propagation should continue after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decay(_t: f64, y: &[f64], out: &mut [f64]) {
        out[0] = -y[0];
    }

    fn oscillator(_t: f64, y: &[f64], out: &mut [f64]) {
        out[0] = y[1];
        out[1] = -y[0];
    }

    #[test]
    fn rk4_decay_is_accurate() {
        let mut y = [1.0];
        let mut scratch = [0.0];
        rk4_step(
            &decay as &dyn Fn(f64, &[f64], &mut [f64]),
            0.0,
            &mut y,
            0.01,
            &mut scratch,
        )
        .unwrap();
        assert!((y[0] - (-0.01f64).exp()).abs() < 1e-10);
    }

    #[test]
    fn rk4_integration_reaches_end_time() {
        let mut y = [1.0];
        let report = integrate_rk4(
            &(decay as fn(f64, &[f64], &mut [f64])),
            &mut y,
            0.0,
            1.0,
            0.01,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(report.steps, 100);
        assert!((y[0] - (-1.0f64).exp()).abs() < 1e-8);
    }

    #[test]
    fn rk4_rejects_bad_arguments() {
        let mut y = [1.0];
        let mut scratch = [0.0];
        assert_eq!(
            rk4_step(
                &(decay as fn(f64, &[f64], &mut [f64])),
                0.0,
                &mut y,
                0.0,
                &mut scratch
            ),
            Err(IntegratorError::BadStep)
        );
        assert_eq!(
            integrate_rk4(
                &(decay as fn(f64, &[f64], &mut [f64])),
                &mut y,
                1.0,
                0.0,
                0.1,
                &mut |_, _| {}
            ),
            Err(IntegratorError::BadStep)
        );
    }

    #[test]
    fn dormand_prince_beats_tolerance_on_decay() {
        let dp = DormandPrince {
            rtol: 1e-11,
            atol: 1e-12,
            h_max: 0.5,
            ..DormandPrince::default()
        };
        let mut y = [1.0];
        let report = dp
            .propagate(
                &(decay as fn(f64, &[f64], &mut [f64])),
                &mut y,
                0.0,
                10.0,
                0.5,
                &mut |_, _| Flow::Continue,
            )
            .unwrap();
        assert!(report.steps > 5);
        assert!((y[0] - (-10.0f64).exp()).abs() < 1e-10);
    }

    #[test]
    fn oscillator_energy_is_bounded() {
        let dp = DormandPrince {
            rtol: 1e-12,
            atol: 1e-12,
            h_max: 10.0,
            ..DormandPrince::default()
        };
        let mut y = [1.0, 0.0];
        dp.propagate(
            &(oscillator as fn(f64, &[f64], &mut [f64])),
            &mut y,
            0.0,
            100.0 * std::f64::consts::TAU,
            0.1,
            &mut |_, _| Flow::Continue,
        )
        .unwrap();
        let energy = 0.5 * (y[0] * y[0] + y[1] * y[1]);
        assert!((energy - 0.5).abs() < 1e-8);
    }

    #[test]
    fn early_stop_works() {
        let dp = DormandPrince::default();
        let mut y = [1.0];
        let mut count = 0;
        let report = dp
            .propagate(
                &(decay as fn(f64, &[f64], &mut [f64])),
                &mut y,
                0.0,
                100.0,
                0.1,
                &mut |_, _| {
                    count += 1;
                    if count == 3 {
                        Flow::Stop
                    } else {
                        Flow::Continue
                    }
                },
            )
            .unwrap();
        assert_eq!(report.steps, 3);
    }

    #[test]
    fn nan_state_is_rejected() {
        let dp = DormandPrince::default();
        let mut y = [f64::NAN];
        let mut t = 0.0;
        let mut h = 0.1;
        assert_eq!(
            dp.step(
                &(decay as fn(f64, &[f64], &mut [f64])),
                &mut t,
                &mut y,
                &mut h
            ),
            Err(IntegratorError::NonFinite)
        );
    }
}

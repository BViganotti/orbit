//! One-dimensional root finding and minimization used by mission targeting.
//!
//! All routines are bounded by an iteration limit and return `None` rather than
//! looping forever. Non-finite evaluations are rejected immediately.

/// Result of a root-finding attempt, including the residual actually obtained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Root {
    pub x: f64,
    pub residual: f64,
    pub iterations: u32,
}

/// Bisection on a bracket `[lo, hi]` where the function changes sign.
pub fn bisect<F: FnMut(f64) -> f64>(
    mut f: F,
    lo: f64,
    hi: f64,
    xtol: f64,
    max_iter: u32,
) -> Option<Root> {
    if !lo.is_finite() || !hi.is_finite() || lo >= hi || xtol <= 0.0 {
        return None;
    }
    let flo = f(lo);
    let fhi = f(hi);
    if !flo.is_finite() || !fhi.is_finite() {
        return None;
    }
    if flo == 0.0 {
        return Some(Root {
            x: lo,
            residual: 0.0,
            iterations: 0,
        });
    }
    if fhi == 0.0 {
        return Some(Root {
            x: hi,
            residual: 0.0,
            iterations: 0,
        });
    }
    if flo.signum() == fhi.signum() {
        return None;
    }
    let mut a = lo;
    let mut b = hi;
    let mut fa = flo;
    for iteration in 1..=max_iter {
        let mid = 0.5 * (a + b);
        if !mid.is_finite() {
            return None;
        }
        let fm = f(mid);
        if !fm.is_finite() {
            return None;
        }
        if fm == 0.0 || (b - a) * 0.5 <= xtol {
            return Some(Root {
                x: mid,
                residual: fm,
                iterations: iteration,
            });
        }
        if fa.signum() == fm.signum() {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
    }
    let x = 0.5 * (a + b);
    Some(Root {
        x,
        residual: f(x),
        iterations: max_iter,
    })
}

/// Secant iteration. Falls back to bisection-style damping when a step is
/// non-finite or leaves the trust interval.
pub fn secant<F: FnMut(f64) -> f64>(
    mut f: F,
    x0: f64,
    x1: f64,
    xtol: f64,
    max_iter: u32,
) -> Option<Root> {
    if !x0.is_finite() || !x1.is_finite() || x0 == x1 || xtol <= 0.0 {
        return None;
    }
    let mut a = x0;
    let mut b = x1;
    let mut fa = f(a);
    let mut fb = f(b);
    if !fa.is_finite() || !fb.is_finite() {
        return None;
    }
    for iteration in 1..=max_iter {
        if fb.abs() <= xtol {
            return Some(Root {
                x: b,
                residual: fb,
                iterations: iteration,
            });
        }
        let denom = fb - fa;
        if denom.abs() < 1.0e-300 {
            return None;
        }
        let mut next = b - fb * (b - a) / denom;
        if !next.is_finite() || next == b {
            return None;
        }
        // Damp wild excursions by halving toward the better endpoint.
        if (next - b).abs() > 0.5 * (b - a).abs().max(1.0e-12) * 4.0 {
            next = 0.5 * (b + next);
        }
        let fnext = f(next);
        if !fnext.is_finite() {
            return None;
        }
        a = b;
        fa = fb;
        b = next;
        fb = fnext;
        if (b - a).abs() <= xtol {
            return Some(Root {
                x: b,
                residual: fb,
                iterations: iteration,
            });
        }
    }
    Some(Root {
        x: b,
        residual: fb,
        iterations: max_iter,
    })
}

/// Newton iteration with a caller-supplied derivative.
pub fn newton<F, D>(mut f: F, mut df: D, mut x: f64, xtol: f64, max_iter: u32) -> Option<Root>
where
    F: FnMut(f64) -> f64,
    D: FnMut(f64) -> f64,
{
    if !x.is_finite() || xtol <= 0.0 {
        return None;
    }
    for iteration in 1..=max_iter {
        let fx = f(x);
        let dx = df(x);
        if !fx.is_finite() || !dx.is_finite() || dx.abs() < 1.0e-300 {
            return None;
        }
        let step = fx / dx;
        if !step.is_finite() {
            return None;
        }
        x -= step;
        if !x.is_finite() {
            return None;
        }
        if step.abs() <= xtol {
            return Some(Root {
                x,
                residual: f(x),
                iterations: iteration,
            });
        }
    }
    Some(Root {
        x,
        residual: f(x),
        iterations: max_iter,
    })
}

/// Golden-section minimization on `[lo, hi]`.
pub fn golden_minimize<F: FnMut(f64) -> f64>(
    mut f: F,
    lo: f64,
    hi: f64,
    xtol: f64,
    max_iter: u32,
) -> Option<(f64, f64)> {
    if !lo.is_finite() || !hi.is_finite() || lo > hi || xtol <= 0.0 {
        return None;
    }
    if lo == hi {
        let v = f(lo);
        return v.is_finite().then_some((lo, v));
    }
    let inv_phi = (5.0f64.sqrt() - 1.0) * 0.5;
    let mut a = lo;
    let mut b = hi;
    let mut c = b - inv_phi * (b - a);
    let mut d = a + inv_phi * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    if !fc.is_finite() || !fd.is_finite() {
        return None;
    }
    for _ in 0..max_iter {
        if (b - a).abs() <= xtol {
            break;
        }
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - inv_phi * (b - a);
            fc = f(c);
            if !fc.is_finite() {
                return None;
            }
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + inv_phi * (b - a);
            fd = f(d);
            if !fd.is_finite() {
                return None;
            }
        }
    }
    if fc < fd {
        Some((c, fc))
    } else {
        Some((d, fd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bisection_finds_cubic_root() {
        let r = bisect(|x| x * x * x - 2.0, 1.0, 2.0, 1e-12, 200).unwrap();
        assert!((r.x - 2.0f64.powf(1.0 / 3.0)).abs() < 1e-11);
        assert!(r.iterations < 100);
    }

    #[test]
    fn bisection_rejects_invalid_bracket() {
        assert!(bisect(|x| x * x + 1.0, -1.0, 1.0, 1e-9, 50).is_none());
        assert!(bisect(|x| x, 1.0, 1.0, 1e-9, 50).is_none());
        assert!(bisect(|_| f64::NAN, 0.0, 1.0, 1e-9, 50).is_none());
    }

    #[test]
    fn secant_converges() {
        let r = secant(|x| x.cos() - x, 0.0, 1.0, 1e-12, 50).unwrap();
        assert!((r.x - 0.739_085_133_215_160_6).abs() < 1e-10);
    }

    #[test]
    fn newton_converges_and_rejects_flat() {
        let r = newton(|x| x * x - 4.0, |x| 2.0 * x, 1.0, 1e-12, 20).unwrap();
        assert!((r.x - 2.0).abs() < 1e-12);
        assert!(newton(|_| 1.0, |_| 0.0, 1.0, 1e-12, 20).is_none());
    }

    #[test]
    fn golden_minimum_of_parabola() {
        let (x, y) =
            golden_minimize(|x| (x - 2.5) * (x - 2.5) + 1.0, -10.0, 10.0, 1e-10, 200).unwrap();
        assert!((x - 2.5).abs() < 1e-7);
        assert!((y - 1.0).abs() < 1e-10);
    }
}

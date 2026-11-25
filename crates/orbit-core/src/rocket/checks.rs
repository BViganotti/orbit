//! Pre-flight checks that turn a validated rocket into actionable engineering
//! advice. Checks never mutate the vehicle and never panic on odd-but-valid
//! input.

use crate::rocket::rocket::{Rocket, RocketSummary};
use crate::G0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

impl Issue {
    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
        }
    }

    fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
        }
    }

    fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckReport {
    pub issues: Vec<Issue>,
    pub summary: Option<RocketSummary>,
}

impl CheckReport {
    pub fn ok(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|i| i.severity == Severity::Error)
    }

    pub fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }
}

/// Run engineering checks. Structural validation is assumed to have passed;
/// this function additionally reports marginal designs.
pub fn inspect(rocket: &Rocket) -> CheckReport {
    let mut issues = Vec::new();
    let mut summary = None;
    if let Err(e) = rocket.validate() {
        issues.push(Issue::error("structure", e));
        return CheckReport { issues, summary };
    }
    let s = rocket.summary();
    summary = Some(s);

    if s.liftoff_twr < 1.0 {
        issues.push(Issue::error(
            "twr",
            format!(
                "liftoff thrust-to-weight is {:.2}; the vehicle cannot leave the pad",
                s.liftoff_twr
            ),
        ));
    } else if s.liftoff_twr < 1.15 {
        issues.push(Issue::warning(
            "twr",
            format!(
                "liftoff thrust-to-weight is {:.2}; ascent will be sluggish",
                s.liftoff_twr
            ),
        ));
    } else if s.liftoff_twr > 5.0 {
        issues.push(Issue::warning(
            "twr",
            "liftoff thrust-to-weight above 5 causes excessive aerodynamic load",
        ));
    }

    if s.total_delta_v < 9_400.0 {
        issues.push(Issue::warning(
            "delta-v",
            format!(
                "ideal delta-v {:.0} m/s is below a typical low Earth orbit budget",
                s.total_delta_v
            ),
        ));
    }
    if s.total_delta_v < 13_000.0 {
        issues.push(Issue::info(
            "lunar-budget",
            "delta-v may be insufficient for transfer and lunar capture",
        ));
    }

    for (i, stage) in rocket.stages.iter().enumerate() {
        let wet = stage.wet_mass();
        let dry_fraction = if wet > 0.0 {
            stage.dry_mass() / wet
        } else {
            1.0
        };
        if dry_fraction > 0.35 {
            issues.push(Issue::warning(
                "dry-fraction",
                format!(
                    "stage {} dry mass is {:.0}% of wet mass; performance is poor",
                    i + 1,
                    dry_fraction * 100.0
                ),
            ));
        }
        let twr = stage.thrust_vacuum() / (wet * G0);
        if twr > 8.0 {
            issues.push(Issue::warning(
                "stage-twr",
                format!(
                    "stage {} vacuum thrust-to-weight is {:.1}; throttle down",
                    i + 1,
                    twr
                ),
            ));
        }
        if stage.burn_time() > 1200.0 {
            issues.push(Issue::warning(
                "burn-time",
                format!(
                    "stage {} burns for {:.0} s at full throttle",
                    i + 1,
                    stage.burn_time()
                ),
            ));
        }
    }

    if s.payload_fraction < 0.005 {
        issues.push(Issue::warning(
            "payload",
            "payload is less than 0.5% of liftoff mass",
        ));
    }
    if s.total_burn_time > 3000.0 {
        issues.push(Issue::warning(
            "duration",
            "total burn time exceeds 50 minutes",
        ));
    }
    if issues.is_empty() {
        issues.push(Issue::info(
            "nominal",
            "all structural and performance checks passed",
        ));
    }
    CheckReport { issues, summary }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocket::presets;

    #[test]
    fn presets_are_mission_capable() {
        for rocket in presets::all() {
            let report = inspect(&rocket);
            assert!(
                report.ok(),
                "{}: {:?}",
                rocket.name,
                report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Error)
                    .collect::<Vec<_>>()
            );
            let s = report.summary.unwrap();
            assert!(s.total_delta_v > 12_000.0, "{}", rocket.name);
        }
    }

    #[test]
    fn weak_rocket_reports_error() {
        let mut r = presets::kestrel();
        r.stages[0].engine_count = 1;
        r.stages[0].engine.thrust_sea_level = 50_000.0;
        r.stages[0].engine.thrust_vacuum = 60_000.0;
        let report = inspect(&r);
        assert!(!report.ok());
        assert!(report.errors() >= 1);
    }

    #[test]
    fn malformed_rocket_returns_single_error() {
        let mut r = presets::kestrel();
        r.payload_mass = -5.0;
        let report = inspect(&r);
        assert!(!report.ok());
        assert_eq!(report.errors(), 1);
        assert!(report.summary.is_none());
    }
}

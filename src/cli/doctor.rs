use crate::core::doctor::{self, Severity};
use anyhow::Result;
use std::path::Path;
use std::process::exit;

/// Print a library health report. Exits with status 1 when any error is found so
/// the command can gate CI; warnings alone leave the exit status at 0.
pub fn run(dir: &Path) -> Result<()> {
    let report = doctor::check(dir);

    if report.findings.is_empty() {
        println!("no problems found");
        return Ok(());
    }

    for finding in &report.findings {
        let label = match finding.severity {
            Severity::Error => "error",
            Severity::Warning => "warn",
        };
        println!("{label} [{}] {}", finding.category, finding.message);
    }

    let (errors, warnings) = (report.errors(), report.warnings());
    println!("\n{errors} error(s), {warnings} warning(s)");

    if errors > 0 {
        exit(1);
    }
    Ok(())
}

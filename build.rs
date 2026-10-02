//! M31A build script: compile-time deployment metadata.
//!
//! The Cargo package version remains the ONLY semantic version authority
//! (`CARGO_PKG_VERSION`). This script only records additional build facts
//! (git commit/branch, timestamp, target, dirty flag, channel) as
//! `cargo:rustc-env` values consumed by `src/deployment/context.rs`.
//! All values degrade to explicit `"unknown"` sentinels (fail-obvious, never
//! fake-clean) when git facts are unavailable.

use std::process::Command;

fn capture(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=M31A_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=M31A_BUILD_BRANCH");

    // Channel is compile-time artifact identity. `development` and
    // `production` are mutually exclusive (hard error in
    // src/deployment/channel.rs); the build script backstops with an
    // explicit failure so no silent choice is ever stamped.
    let dev = std::env::var("CARGO_FEATURE_DEVELOPMENT").is_ok();
    let prod = std::env::var("CARGO_FEATURE_PRODUCTION").is_ok();
    if dev && prod {
        panic!(
            "M31A deployment channels are mutually exclusive: features `development` and `production` \
             must never be enabled together. Build each channel separately."
        );
    }
    let channel = if dev { "development" } else { "production" };
    println!("cargo:rustc-env=M31A_CHANNEL={channel}");

    // Git facts: explicit env overrides win (CI determinism), else live git.
    let commit = std::env::var("M31A_BUILD_COMMIT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| capture("git", &["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=M31A_GIT_COMMIT={commit}");

    let branch = std::env::var("M31A_BUILD_BRANCH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| capture("git", &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=M31A_GIT_BRANCH={branch}");

    let dirty = capture("git", &["status", "--porcelain"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    println!(
        "cargo:rustc-env=M31A_GIT_DIRTY={}",
        if dirty { "true" } else { "false" }
    );

    // Build timestamp: SOURCE_DATE_EPOCH honored when valid, else wall clock.
    let timestamp = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|e| e.trim().parse::<i64>().ok())
        .filter(|s| *s >= 0)
        .and_then(chrono_lite_rfc3339)
        .unwrap_or_else(|| {
            // Fallback without chrono dependency in build script: use `date`.
            capture("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]).unwrap_or_else(|| "unknown".to_string())
        });
    println!("cargo:rustc-env=M31A_BUILD_TIMESTAMP={timestamp}");

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=M31A_TARGET={target}");
}

/// Minimal epoch-secs → RFC3339 UTC formatter (no chrono in build script).
fn chrono_lite_rfc3339(secs: i64) -> Option<String> {
    // Days-based civil conversion (Howard Hinnant algorithm).
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    y += i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"))
}

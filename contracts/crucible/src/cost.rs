//! Helpers for measuring and reporting contract execution costs.
//! SDK 26: soroban_env_host is not a public dependency; FeeEstimate is defined locally.

/// Fee breakdown returned by the Soroban host (SDK 26 compatible).
/// Mirrors the fields previously exposed by `soroban_env_host::FeeEstimate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeEstimate {
    pub total: i64,
    pub instructions: i64,
    pub disk_read_entries: i64,
    pub write_entries: i64,
    pub disk_read_bytes: i64,
    pub write_bytes: i64,
    pub contract_events: i64,
    pub persistent_entry_rent: i64,
    pub temporary_entry_rent: i64,
}

/// A report of the compute costs for a contract invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostReport {
    instructions: u64,
    memory: u64,
    fee_estimate: Option<FeeEstimate>,
}

impl CostReport {
    /// Creates a new cost report.
    pub fn new(instructions: u64, memory: u64) -> Self {
        Self {
            instructions,
            memory,
            fee_estimate: None,
        }
    }

    /// Creates a new cost report with an SDK-derived fee estimate.
    pub fn new_with_fee_estimate(
        instructions: u64,
        memory: u64,
        fee_estimate: FeeEstimate,
    ) -> Self {
        Self {
            instructions,
            memory,
            fee_estimate: Some(fee_estimate),
        }
    }

    /// Returns the number of CPU instructions consumed.
    pub fn instructions(&self) -> u64 {
        self.instructions
    }

    /// Returns the peak memory usage in bytes.
    pub fn memory_bytes(&self) -> u64 {
        self.memory
    }

    /// Returns the estimated network fee in stroops.
    pub fn fee_stroops(&self) -> i64 {
        self.fee_estimate
            .as_ref()
            .map(|fee| fee.total)
            .unwrap_or((self.instructions / 100) as i64)
    }

    /// Returns whether the fee estimate comes from the Soroban SDK.
    pub fn uses_sdk_fee_estimate(&self) -> bool {
        self.fee_estimate.is_some()
    }

    /// Returns a human-readable formatted table report of the costs.
    pub fn report(&self) -> String {
        let instructions_str = format_with_commas(self.instructions);
        let memory_str = format_with_commas(self.memory);
        let fee_str = format!("{} str", self.fee_stroops());
        let mut output = String::new();
        output.push_str("+---------------------+-----------+\n");
        output.push_str("| Metric              | Value     |\n");
        output.push_str("+---------------------+-----------+\n");
        output.push_str(&format!(
            "| Instructions        | {:>9} |\n",
            instructions_str
        ));
        output.push_str(&format!("| Memory (bytes)      | {:>9} |\n", memory_str));
        output.push_str(&format!("| Estimated fee       | {:>9} |\n", fee_str));
        output.push_str("+---------------------+-----------+");
        output
    }

    /// Returns a CI-safe ASCII report of the costs.
    ///
    /// This keeps the same core metrics as [`report`](Self::report) while avoiding
    /// box-drawing characters for terminals, logs, and markdown renderers that do
    /// not handle Unicode table borders consistently.
    pub fn report_plain(&self) -> String {
        let instructions_str = format_with_commas(self.instructions);
        let memory_str = format_with_commas(self.memory);
        let fee_str = format!("{} str", self.fee_stroops());

        format!(
            "Metric | Value\n\
             --- | ---\n\
             Instructions | {}\n\
             Memory (bytes) | {}\n\
             Estimated fee | {}",
            instructions_str, memory_str, fee_str
        )
    }
}

/// Format a number with comma separators for readability.
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, &c) in chars.iter().enumerate() {
        result.push(c);
        let remaining = len - i - 1;
        if remaining > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
    }
    result
}

#[cfg(feature = "snapshots")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "snapshots")]
#[derive(Serialize, Deserialize)]
struct CostSnapshot {
    name: String,
    instructions: u64,
    memory_bytes: u64,
    fee_stroops: i64,
}

#[cfg(feature = "snapshots")]
impl CostReport {
    pub fn assert_snapshot(&self, name: &str) {
        self.assert_snapshot_with_tolerance(name, 0.05);
    }

    pub fn assert_snapshot_with_tolerance(&self, name: &str, tolerance: f64) {
        use std::fs;
        use std::path::PathBuf;

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let snap_dir = PathBuf::from(&manifest_dir)
            .join("test_snapshots")
            .join("cost");
        let snap_path = snap_dir.join(format!("{}.json", name));

        let update = std::env::var("CRUCIBLE_UPDATE_SNAPSHOTS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !snap_path.exists() || update {
            fs::create_dir_all(&snap_dir)
                .unwrap_or_else(|e| panic!("failed to create snapshot dir: {}", e));

            let snapshot = CostSnapshot {
                name: name.to_string(),
                instructions: self.instructions,
                memory_bytes: self.memory,
                fee_stroops: self.fee_stroops(),
            };
            let json = serde_json::to_string_pretty(&snapshot)
                .unwrap_or_else(|e| panic!("failed to serialize snapshot: {}", e));
            fs::write(&snap_path, json)
                .unwrap_or_else(|e| panic!("failed to write snapshot: {}", e));

            if update {
                eprintln!("[crucible] updated snapshot '{}'", name);
            } else {
                eprintln!("[crucible] wrote new snapshot '{}'", name);
            }
            return;
        }

        let contents = fs::read_to_string(&snap_path)
            .unwrap_or_else(|e| panic!("failed to read snapshot '{}': {}", name, e));

        let saved: CostSnapshot = serde_json::from_str(&contents)
            .unwrap_or_else(|e| panic!("failed to parse snapshot '{}': {}", name, e));

        check_within_tolerance(
            "instructions",
            saved.instructions,
            self.instructions,
            tolerance,
            name,
        );
        check_within_tolerance(
            "memory_bytes",
            saved.memory_bytes,
            self.memory,
            tolerance,
            name,
        );
    }
}

#[cfg(feature = "snapshots")]
fn check_within_tolerance(metric: &str, saved: u64, current: u64, tolerance: f64, name: &str) {
    if saved == 0 {
        return;
    }
    let ratio = current as f64 / saved as f64;
    if ratio > 1.0 + tolerance {
        panic!(
            "cost regression in snapshot '{}': {} increased from {} to {} ({:.1}% > {:.1}% tolerance)",
            name, metric, saved, current, (ratio - 1.0) * 100.0, tolerance * 100.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_report_creation() {
        let report = CostReport::new(1_000_000, 50_000);
        assert_eq!(report.instructions(), 1_000_000);
        assert_eq!(report.memory_bytes(), 50_000);
    }

    #[test]
    fn test_fee_stroops_calculation() {
        let report = CostReport::new(10_000, 0);
        assert_eq!(report.fee_stroops(), 100);
    }

    #[test]
    fn test_fee_stroops_uses_sdk_fee_estimate_when_available() {
        let sdk_fee = FeeEstimate {
            total: 42,
            instructions: 10,
            disk_read_entries: 0,
            write_entries: 0,
            disk_read_bytes: 0,
            write_bytes: 0,
            contract_events: 0,
            persistent_entry_rent: 0,
            temporary_entry_rent: 0,
        };
        let report = CostReport::new_with_fee_estimate(10_000, 0, sdk_fee.clone());
        assert!(report.uses_sdk_fee_estimate());
        assert_eq!(report.fee_stroops(), 42);
        assert_eq!(report.report().contains("Estimated fee"), true);
    }

    #[test]
    fn test_fee_stroops_falls_back_to_instruction_heuristic() {
        let report = CostReport::new(50_000, 0);
        assert_eq!(report.uses_sdk_fee_estimate(), false);
        assert_eq!(report.fee_stroops(), 500); // 50_000 / 100 = 500
    }

    #[test]
    fn test_report_returns_non_empty_string() {
        let report = CostReport::new(1_234_567, 45_678);
        let report_str = report.report();
        assert!(!report_str.is_empty());
        assert!(report_str.contains("Instructions"));
        assert!(report_str.contains("Memory"));
        assert!(report_str.contains("Estimated fee"));
    }

    #[test]
    fn test_format_with_commas() {
        assert_eq!(format_with_commas(0), "0");
        assert_eq!(format_with_commas(123), "123");
        assert_eq!(format_with_commas(1_234), "1,234");
        assert_eq!(format_with_commas(1_234_567), "1,234,567");
        assert_eq!(format_with_commas(1_000_000_000), "1,000,000,000");
    }

    #[test]
    fn test_snapshot_serialization_roundtrip() {
        #[cfg(feature = "snapshots")]
        {
            let snap = super::CostSnapshot {
                name: "test".to_string(),
                instructions: 1000,
                memory_bytes: 2000,
                fee_stroops: 10,
            };
            let json = serde_json::to_string(&snap).unwrap();
            let parsed: super::CostSnapshot = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.instructions, 1000);
            assert_eq!(parsed.memory_bytes, 2000);
        }
    }
}

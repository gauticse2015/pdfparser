//! Resource governor and limits.
use std::sync::atomic::{AtomicU64, Ordering};

/// Limit kinds (structured).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    /// File size.
    FileSize,
    /// Expanded stream bytes.
    ExpandedBytes,
    /// Single stream expansion.
    StreamExpansion,
    /// Operations.
    Operations,
    /// Nesting depth.
    NestingDepth,
    /// Other.
    Other,
}

/// Compile-time hard maxima.
pub mod hard_max {
    /// Max file bytes (512 MiB).
    pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
    /// Max total expanded.
    pub const MAX_TOTAL_EXPANDED: u64 = 1024 * 1024 * 1024;
    /// Max single stream expansion ratio.
    pub const MAX_EXPAND_RATIO: u64 = 1000;
    /// Max ops per page.
    pub const MAX_PAGE_OPS: u64 = 10_000_000;
}

/// Configurable limits (clamped to hard_max).
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Max input file size.
    pub max_file_bytes: u64,
    /// Max total expanded stream bytes (process budget for this document).
    pub max_total_expanded_bytes: u64,
    /// Max expansion ratio per stream.
    pub max_expand_ratio: u64,
    /// Max content operators per page.
    pub max_page_ops: u64,
    /// Max nesting depth.
    pub max_nesting_depth: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 128 * 1024 * 1024,
            max_total_expanded_bytes: 256 * 1024 * 1024,
            max_expand_ratio: 200,
            max_page_ops: 2_000_000,
            max_nesting_depth: 64,
        }
    }
}

impl ResourceLimits {
    /// Clamp to hard maxima.
    pub fn clamped(mut self) -> Self {
        self.max_file_bytes = self.max_file_bytes.min(hard_max::MAX_FILE_BYTES);
        self.max_total_expanded_bytes = self
            .max_total_expanded_bytes
            .min(hard_max::MAX_TOTAL_EXPANDED);
        self.max_expand_ratio = self.max_expand_ratio.min(hard_max::MAX_EXPAND_RATIO);
        self.max_page_ops = self.max_page_ops.min(hard_max::MAX_PAGE_OPS);
        self
    }
}

/// Process-wide-ish budget for one document open.
#[derive(Debug)]
pub struct ResourceGovernor {
    /// Limits.
    pub limits: ResourceLimits,
    expanded: AtomicU64,
}

impl ResourceGovernor {
    /// Create with clamped limits.
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits: limits.clamped(),
            expanded: AtomicU64::new(0),
        }
    }

    /// Charge expanded bytes; error if over budget.
    pub fn charge_expanded(&self, n: u64) -> Result<(), crate::Error> {
        let prev = self.expanded.fetch_add(n, Ordering::SeqCst);
        if prev.saturating_add(n) > self.limits.max_total_expanded_bytes {
            return Err(crate::Error::LimitExceeded {
                kind: LimitKind::ExpandedBytes,
            });
        }
        Ok(())
    }

    /// Check single-stream expansion ratio.
    pub fn check_expand_ratio(&self, encoded: u64, decoded: u64) -> Result<(), crate::Error> {
        if encoded == 0 {
            return Ok(());
        }
        if decoded / encoded.max(1) > self.limits.max_expand_ratio {
            return Err(crate::Error::LimitExceeded {
                kind: LimitKind::StreamExpansion,
            });
        }
        Ok(())
    }
}

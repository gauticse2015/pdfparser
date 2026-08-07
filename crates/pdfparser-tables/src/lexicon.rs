//! Shared form / notice / tax-worksheet lexicons.
//!
//! Geometry detectors must not each maintain a private IRS/NIST string list.

/// Phrases that mark IRS / tax-form field grids (Schedule C/D, 1099, OMB).
pub const TAX_FORM_PHRASES: &[&str] = &[
    "social security",
    "employer id",
    "employer identification",
    "enter code from instructions",
    "(ssn)",
    "(ein)",
    "accounting method",
    "business address",
    "principal business or profession",
    "profit or loss from business",
    "schedule c",
    "schedule d",
    "omb no.",
    "form 1099",
    "form 1099-b",
    "short-term transactions",
    "long-term transactions",
    "proceeds (sales price)",
    "cost (or other basis)",
    "totals for all short-term",
    "totals for all long-term",
    "adjustments to gain or loss",
    "capital gain or (loss)",
    "department of the treasury",
    "irs use only",
    "internal revenue",
];

/// NIST / withdrawn-standard / warning-notice metadata grids.
pub const NOTICE_METADATA_PHRASES: &[&str] = &[
    "name of standard",
    "withdrawn",
    "warning notice",
    "series/number",
    "fips",
];

/// Join the first `limit` cells into a lowercase blob for keyword scans.
pub fn cell_blob(texts: impl IntoIterator<Item = impl AsRef<str>>, limit: usize) -> String {
    texts
        .into_iter()
        .take(limit)
        .map(|s| s.as_ref().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Count how many `phrases` occur as substrings of `blob`.
pub fn phrase_hits(blob: &str, phrases: &[&str]) -> u32 {
    phrases.iter().filter(|p| blob.contains(*p)).count() as u32
}

/// True when the blob looks like an IRS header strip (OMB / Treasury).
pub fn is_irs_header_blob(blob: &str) -> bool {
    blob.contains("department of the treasury")
        || blob.contains("omb no.")
        || blob.contains("irs use only")
        || blob.contains("internal revenue")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tax_hits_schedule_c() {
        let blob = cell_blob(["Schedule C", "OMB No. 1545"], 8);
        assert!(phrase_hits(&blob, TAX_FORM_PHRASES) >= 2);
        assert!(is_irs_header_blob(&blob));
    }
}

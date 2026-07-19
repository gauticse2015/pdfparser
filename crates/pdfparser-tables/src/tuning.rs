//! User-overridable geometry / densify thresholds.
//!
//! System defaults match production policy for mixed PDFs. Callers with known
//! document classes (statistical yearbooks, multi-line prose grids, invoices)
//! can override individual keys at extract time without forking detectors.
//!
//! # Example
//! ```
//! use pdfparser_tables::{TableOptions, TablePreset, TableTuning};
//!
//! let mut opts = TableOptions::from_preset(TablePreset::Auto);
//! opts.tuning
//!     .apply_overrides([
//!         ("densify_y_skip_numeric_frac", 0.10),
//!         ("densify_y_explode_growth_hi", 3.0),
//!     ])
//!     .unwrap();
//! assert!((opts.tuning.densify_y_skip_numeric_frac - 0.10).abs() < 1e-6);
//! ```
//!
//! Keys form a flat settings dict (`key → f64`). Booleans/u32 fields use 0/1
//! and whole numbers respectively when set via the map API.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Geometry / densify / lattice thresholds that often need document-type tuning.
///
/// Nested under [`crate::TableAdvancedOptions::tuning`] (reachable as
/// `opts.tuning` via Deref). Not part of the ≤12 product surface.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct TableTuning {
    // ── densify Y growth policy ──────────────────────────────────────────
    /// Max extra rows counted as "small recovery" (default **2**).
    pub densify_y_small_delta_max: u32,
    /// Max growth ratio for small recovery (default **1.4**).
    pub densify_y_small_growth_max: f32,
    /// Min pre-densify body rows before mid-range wrap-explode reject (default **6**).
    pub densify_y_explode_min_before: u32,
    /// Growth ratio lower bound for wrap-explode reject (default **1.4**).
    pub densify_y_explode_growth_lo: f32,
    /// Growth ratio upper bound for wrap-explode reject (default **2.6**).
    /// Growth above this is kept (sparse-H statistical tables that grow 3×+).
    pub densify_y_explode_growth_hi: f32,

    // ── densify Y prose skip ─────────────────────────────────────────────
    /// Skip Y densify when V-cols ≥ this, H sparse, and numeric density low (default **6**).
    pub densify_y_skip_min_v_cols: u32,
    /// Skip Y densify only when H-rows ≤ this (default **5**).
    pub densify_y_skip_max_h_rows: u32,
    /// Skip Y densify only when H-rows ≥ this (default **2**).
    pub densify_y_skip_min_h_rows: u32,
    /// Numeric text-run fraction below which prose-Y skip may fire (default **0.22**).
    pub densify_y_skip_numeric_frac: f32,

    // ── densify X guards ─────────────────────────────────────────────────
    /// Reject X densify when densified cols > this absolute (default **14**).
    pub densify_x_explode_abs_cols: u32,
    /// Reject X densify when dens_cols > base×factor + add (default **3.5**).
    pub densify_x_explode_growth_factor: f32,
    /// Additive term for X explode check (default **2.0**).
    pub densify_x_explode_growth_add: f32,
    /// Narrow-grid max extra cols from densify when base ≤ 3 (default **1**).
    pub densify_x_narrow_max_extra: u32,
    /// Exterior frame expand pad as fraction of frame width (default **0.55**).
    pub densify_x_exterior_pad_frac: f32,
    /// Max character count for "short" densify-X token candidates (default **14**).
    pub densify_x_short_token_chars: u32,

    // ── densify pitch / regularity ───────────────────────────────────────
    /// Max coefficient of variation of text-band pitch for regular densify (default **0.45**).
    pub densify_pitch_cv_max: f32,

    // ── lattice joint span filters ───────────────────────────────────────
    /// Min joint span fraction of extent for vertical (column) lines (default **0.40**).
    pub lattice_v_span_frac: f32,
    /// Min joint span fraction for horizontal (row) lines (default **0.22**).
    pub lattice_h_span_frac: f32,
    /// Raster-line V span floor (default **0.15**).
    pub lattice_raster_v_span_frac: f32,
    /// Raster-line H span floor (default **0.10**).
    pub lattice_raster_h_span_frac: f32,
    /// Recover long H rules when long-clustered count ≥ ys×ratio (default **1.5**).
    pub lattice_long_h_recover_ratio: f32,
    /// Long-H recovery requires segment length ≥ width×this (default **0.55**).
    pub lattice_long_h_width_frac: f32,

    // ── solid-lattice stream ownership exception ─────────────────────────
    /// Tall densified few-col lattices must not kill multi-col stream (default **3**).
    pub solid_lattice_stream_safe_max_cols: u32,
    /// Min rows for the stream-safe densified lattice exception (default **20**).
    pub solid_lattice_stream_safe_min_rows: u32,
}

impl Default for TableTuning {
    fn default() -> Self {
        Self {
            densify_y_small_delta_max: 2,
            densify_y_small_growth_max: 1.4,
            densify_y_explode_min_before: 6,
            densify_y_explode_growth_lo: 1.4,
            densify_y_explode_growth_hi: 2.6,
            densify_y_skip_min_v_cols: 6,
            densify_y_skip_max_h_rows: 5,
            densify_y_skip_min_h_rows: 2,
            densify_y_skip_numeric_frac: 0.22,
            densify_x_explode_abs_cols: 14,
            densify_x_explode_growth_factor: 3.5,
            densify_x_explode_growth_add: 2.0,
            densify_x_narrow_max_extra: 1,
            densify_x_exterior_pad_frac: 0.55,
            densify_x_short_token_chars: 14,
            densify_pitch_cv_max: 0.45,
            lattice_v_span_frac: 0.40,
            lattice_h_span_frac: 0.22,
            lattice_raster_v_span_frac: 0.15,
            lattice_raster_h_span_frac: 0.10,
            lattice_long_h_recover_ratio: 1.5,
            lattice_long_h_width_frac: 0.55,
            solid_lattice_stream_safe_max_cols: 3,
            solid_lattice_stream_safe_min_rows: 20,
        }
    }
}

/// Known setting keys (stable API surface for CLI / JSON / language bindings).
pub const TABLE_TUNING_KEYS: &[&str] = &[
    "densify_y_small_delta_max",
    "densify_y_small_growth_max",
    "densify_y_explode_min_before",
    "densify_y_explode_growth_lo",
    "densify_y_explode_growth_hi",
    "densify_y_skip_min_v_cols",
    "densify_y_skip_max_h_rows",
    "densify_y_skip_min_h_rows",
    "densify_y_skip_numeric_frac",
    "densify_x_explode_abs_cols",
    "densify_x_explode_growth_factor",
    "densify_x_explode_growth_add",
    "densify_x_narrow_max_extra",
    "densify_x_exterior_pad_frac",
    "densify_x_short_token_chars",
    "densify_pitch_cv_max",
    "lattice_v_span_frac",
    "lattice_h_span_frac",
    "lattice_raster_v_span_frac",
    "lattice_raster_h_span_frac",
    "lattice_long_h_recover_ratio",
    "lattice_long_h_width_frac",
    "solid_lattice_stream_safe_max_cols",
    "solid_lattice_stream_safe_min_rows",
];

impl TableTuning {
    /// Production defaults (alias of [`Default::default`]).
    pub fn defaults() -> Self {
        Self::default()
    }

    /// All supported settings keys.
    pub fn keys() -> &'static [&'static str] {
        TABLE_TUNING_KEYS
    }

    /// Read one setting as `f64` (u32 fields cast).
    pub fn get(&self, key: &str) -> Option<f64> {
        Some(match key {
            "densify_y_small_delta_max" => self.densify_y_small_delta_max as f64,
            "densify_y_small_growth_max" => self.densify_y_small_growth_max as f64,
            "densify_y_explode_min_before" => self.densify_y_explode_min_before as f64,
            "densify_y_explode_growth_lo" => self.densify_y_explode_growth_lo as f64,
            "densify_y_explode_growth_hi" => self.densify_y_explode_growth_hi as f64,
            "densify_y_skip_min_v_cols" => self.densify_y_skip_min_v_cols as f64,
            "densify_y_skip_max_h_rows" => self.densify_y_skip_max_h_rows as f64,
            "densify_y_skip_min_h_rows" => self.densify_y_skip_min_h_rows as f64,
            "densify_y_skip_numeric_frac" => self.densify_y_skip_numeric_frac as f64,
            "densify_x_explode_abs_cols" => self.densify_x_explode_abs_cols as f64,
            "densify_x_explode_growth_factor" => self.densify_x_explode_growth_factor as f64,
            "densify_x_explode_growth_add" => self.densify_x_explode_growth_add as f64,
            "densify_x_narrow_max_extra" => self.densify_x_narrow_max_extra as f64,
            "densify_x_exterior_pad_frac" => self.densify_x_exterior_pad_frac as f64,
            "densify_x_short_token_chars" => self.densify_x_short_token_chars as f64,
            "densify_pitch_cv_max" => self.densify_pitch_cv_max as f64,
            "lattice_v_span_frac" => self.lattice_v_span_frac as f64,
            "lattice_h_span_frac" => self.lattice_h_span_frac as f64,
            "lattice_raster_v_span_frac" => self.lattice_raster_v_span_frac as f64,
            "lattice_raster_h_span_frac" => self.lattice_raster_h_span_frac as f64,
            "lattice_long_h_recover_ratio" => self.lattice_long_h_recover_ratio as f64,
            "lattice_long_h_width_frac" => self.lattice_long_h_width_frac as f64,
            "solid_lattice_stream_safe_max_cols" => self.solid_lattice_stream_safe_max_cols as f64,
            "solid_lattice_stream_safe_min_rows" => self.solid_lattice_stream_safe_min_rows as f64,
            _ => return None,
        })
    }

    /// Set one setting from a numeric value. Unknown keys return `Err`.
    pub fn set(&mut self, key: &str, value: f64) -> Result<(), String> {
        if !value.is_finite() {
            return Err(format!("table tuning '{key}': value must be finite"));
        }
        match key {
            "densify_y_small_delta_max" => self.densify_y_small_delta_max = as_u32(value, key)?,
            "densify_y_small_growth_max" => self.densify_y_small_growth_max = value as f32,
            "densify_y_explode_min_before" => {
                self.densify_y_explode_min_before = as_u32(value, key)?
            }
            "densify_y_explode_growth_lo" => self.densify_y_explode_growth_lo = value as f32,
            "densify_y_explode_growth_hi" => self.densify_y_explode_growth_hi = value as f32,
            "densify_y_skip_min_v_cols" => self.densify_y_skip_min_v_cols = as_u32(value, key)?,
            "densify_y_skip_max_h_rows" => self.densify_y_skip_max_h_rows = as_u32(value, key)?,
            "densify_y_skip_min_h_rows" => self.densify_y_skip_min_h_rows = as_u32(value, key)?,
            "densify_y_skip_numeric_frac" => self.densify_y_skip_numeric_frac = value as f32,
            "densify_x_explode_abs_cols" => self.densify_x_explode_abs_cols = as_u32(value, key)?,
            "densify_x_explode_growth_factor" => {
                self.densify_x_explode_growth_factor = value as f32
            }
            "densify_x_explode_growth_add" => self.densify_x_explode_growth_add = value as f32,
            "densify_x_narrow_max_extra" => self.densify_x_narrow_max_extra = as_u32(value, key)?,
            "densify_x_exterior_pad_frac" => self.densify_x_exterior_pad_frac = value as f32,
            "densify_x_short_token_chars" => self.densify_x_short_token_chars = as_u32(value, key)?,
            "densify_pitch_cv_max" => self.densify_pitch_cv_max = value as f32,
            "lattice_v_span_frac" => self.lattice_v_span_frac = value as f32,
            "lattice_h_span_frac" => self.lattice_h_span_frac = value as f32,
            "lattice_raster_v_span_frac" => self.lattice_raster_v_span_frac = value as f32,
            "lattice_raster_h_span_frac" => self.lattice_raster_h_span_frac = value as f32,
            "lattice_long_h_recover_ratio" => self.lattice_long_h_recover_ratio = value as f32,
            "lattice_long_h_width_frac" => self.lattice_long_h_width_frac = value as f32,
            "solid_lattice_stream_safe_max_cols" => {
                self.solid_lattice_stream_safe_max_cols = as_u32(value, key)?
            }
            "solid_lattice_stream_safe_min_rows" => {
                self.solid_lattice_stream_safe_min_rows = as_u32(value, key)?
            }
            _ => {
                return Err(format!(
                    "unknown table tuning key '{key}' (see TableTuning::keys())"
                ))
            }
        }
        Ok(())
    }

    /// Apply a settings dict (key → value). Stops on first unknown / invalid key.
    pub fn apply_overrides<I, K>(&mut self, pairs: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (K, f64)>,
        K: AsRef<str>,
    {
        for (k, v) in pairs {
            self.set(k.as_ref(), v)?;
        }
        Ok(())
    }

    /// Export all keys as a flat settings dict (stable for logging / round-trip).
    pub fn as_map(&self) -> Vec<(&'static str, f64)> {
        TABLE_TUNING_KEYS
            .iter()
            .filter_map(|&k| self.get(k).map(|v| (k, v)))
            .collect()
    }

    /// Parse CLI / config fragments `key=value` or `key:value` (comma-separated OK).
    ///
    /// Example: `"densify_y_skip_numeric_frac=0.10,densify_y_explode_growth_hi=3.0"`.
    pub fn apply_kv_string(&mut self, s: &str) -> Result<(), String> {
        for part in s.split(|c| c == ',' || c == ';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (k, v) = part
                .split_once('=')
                .or_else(|| part.split_once(':'))
                .ok_or_else(|| {
                    format!("table tuning entry '{part}' must be key=value (or key:value)")
                })?;
            let k = k.trim();
            let v = v
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("table tuning '{k}': expected number, got '{}'", v.trim()))?;
            self.set(k, v)?;
        }
        Ok(())
    }
}

fn as_u32(value: f64, key: &str) -> Result<u32, String> {
    if value < 0.0 || value > u32::MAX as f64 {
        return Err(format!("table tuning '{key}': out of u32 range"));
    }
    if (value - value.round()).abs() > 1e-6 {
        return Err(format!("table tuning '{key}': expected whole number"));
    }
    Ok(value.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip_map() {
        let t = TableTuning::defaults();
        let map = t.as_map();
        assert_eq!(map.len(), TABLE_TUNING_KEYS.len());
        let mut t2 = TableTuning::defaults();
        t2.apply_overrides(map.iter().map(|(k, v)| (*k, *v)))
            .unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn apply_kv_string_overrides() {
        let mut t = TableTuning::defaults();
        t.apply_kv_string("densify_y_skip_numeric_frac=0.10; densify_y_explode_min_before=8")
            .unwrap();
        assert!((t.densify_y_skip_numeric_frac - 0.10).abs() < 1e-6);
        assert_eq!(t.densify_y_explode_min_before, 8);
    }

    #[test]
    fn unknown_key_errors() {
        let mut t = TableTuning::defaults();
        assert!(t.set("not_a_real_key", 1.0).is_err());
    }
}

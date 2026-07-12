//! Optional page render / line providers (Tier 1 capability ladder).
//!
//! Full-page render backends plug in behind [`PageRenderer`]:
//! - [`NullPageRenderer`] — default fail-soft
//! - [`ExternalCliPageRenderer`] — pdftoppm / mutool / gs process (PR3 spike)
//!
//! Default product path does **not** require a renderer (pure Rust Core).
//! See `docs/design-table-engine-v2.md` K4/K25 and PR3.

mod external_cli;
mod render;

pub use external_cli::ExternalCliPageRenderer;
pub use render::{NullPageRenderer, PageRenderer, ProviderError, RenderSafety};

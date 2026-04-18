//! Three-stage rendering pipeline types (chartml 5.0 phase 2).
//!
//! The pipeline splits chart rendering into three explicit stages so each
//! has a single typed responsibility:
//!
//! ```text
//!   YAML ──► FETCH ──► FetchedChart ──► TRANSFORM ──► PreparedChart ──► render ──► SVG
//!            (async)                    (async)                          (sync)
//! ```
//!
//! Phase 2 introduces the type shapes and exposes the explicit
//! `ChartML::fetch` / `transform` / `render_prepared_to_svg` /
//! `render_to_svg_async` methods. No provider trait yet — `fetch` reads
//! from pre-registered sources only. Phase 3 adds the provider trait,
//! resolver, cache, and parallel fetch.
//!
//! All types derive `Clone` because:
//! - `DataTable` is `Arc`-backed internally, so cloning sources is cheap;
//! - `Clone` enables the "fetch + transform once, resize-render N times"
//!   use case (e.g. responsive layouts) without re-running upstream stages.

use std::collections::HashMap;
use std::time::SystemTime;

use indexmap::IndexMap;

use crate::data::DataTable;
use crate::spec::ChartSpec;

mod render_options;

pub use render_options::RenderOptions;

/// Stage 1 output: the spec plus every named source resolved to a `DataTable`.
///
/// `sources` always has at least one entry. The key is the user-chosen name
/// from the YAML `data:` map; for unnamed flat data the canonical key is
/// `"source"`. Insertion order matches the YAML — preserved via `IndexMap`
/// so transform middleware can rely on stable ordering.
#[derive(Debug, Clone)]
pub struct FetchedChart {
    pub spec: ChartSpec,
    pub sources: IndexMap<String, DataTable>,
    pub metadata: FetchMetadata,
}

/// Stage 2 output: spec plus the single transformed table the renderer will consume.
///
/// `data` is either the lone source (passthrough when no transform is declared)
/// or the result of running registered `TransformMiddleware` over the
/// fetched sources.
#[derive(Debug, Clone)]
pub struct PreparedChart {
    pub spec: ChartSpec,
    pub data: DataTable,
    pub metadata: PreparedMetadata,
}

/// Per-fetch telemetry. `per_source` stays empty in phase 2 (no provider
/// dispatch yet); phase 3 populates it from `FetchResult.metadata` per
/// declared source.
#[derive(Debug, Clone)]
pub struct FetchMetadata {
    pub refreshed_at: SystemTime,
    /// Source names served from cache. Empty in phase 2 (no resolver yet).
    pub cache_hits: Vec<String>,
    /// Source names that produced a fresh fetch. Empty in phase 2 (no resolver yet).
    pub cache_misses: Vec<String>,
    /// Per-source provider metadata (e.g. `bytes_billed`, `rows_returned`).
    /// Empty in phase 2 — populated once providers exist.
    pub per_source: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl FetchMetadata {
    /// Construct empty metadata stamped at the current wall clock. Used by
    /// phase 2's `fetch` (which only reads from pre-registered sources).
    pub fn empty_now() -> Self {
        Self {
            refreshed_at: SystemTime::now(),
            cache_hits: Vec::new(),
            cache_misses: Vec::new(),
            per_source: HashMap::new(),
        }
    }
}

/// Per-transform telemetry. Captures whether the middleware actually ran
/// (vs. passthrough) plus the names of the sources the transform consumed.
#[derive(Debug, Clone)]
pub struct PreparedMetadata {
    pub refreshed_at: SystemTime,
    /// `true` when transform middleware was invoked; `false` for the
    /// single-source passthrough path.
    pub transform_applied: bool,
    /// Names of every source that fed the transform stage. Insertion order
    /// matches the YAML — preserved by carrying the `IndexMap` keys
    /// directly out of stage 1.
    pub sources_used: Vec<String>,
}

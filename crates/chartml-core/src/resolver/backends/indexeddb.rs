//! `IndexedDbBackend` — persistent tier-2 [`CacheBackend`] for browser
//! consumers (chartml 5.0 phase 3b).
//!
//! ## Why IndexedDB?
//!
//! Page refreshes, tab navigations, and SPA route changes all blow away the
//! tier-1 in-process [`MemoryBackend`](crate::resolver::cache::MemoryBackend).
//! IndexedDB is origin-scoped persistent storage available in every modern
//! browser, so persisting cached `DataTable`s there means a refresh within the
//! source's TTL skips the upstream entirely — important for expensive
//! analytical queries that take seconds to return.
//!
//! ## Storage layout
//!
//! Every backend instance opens **one IndexedDB database** (`database_name`)
//! and reads/writes through **one object store** (`ENTRIES_STORE = "entries"`).
//! Entries are keyed by a UTF-8 string of the form `"{namespace}:{key:016x}"`
//! so a single database can host multiple namespaces without collision and
//! namespace-scoped operations (`clear`, `invalidate_by_tag`) iterate via an
//! `IDBKeyRange` bound to the namespace prefix.
//!
//! ## Stored blob format (versioned)
//!
//! Values are stored as `Uint8Array` packed in the layout defined by the
//! shared [`super::codec`] module — see its module docs for the byte
//! framing. Using a shared codec keeps the on-disk schema identical across
//! every tier-2 backend and lets the framing be unit-tested on native
//! targets without spinning up a browser.
//!
//! On `get()` we read the version byte first; if it doesn't match the current
//! `BLOB_VERSION` we fire-and-forget an `invalidate()` and return `None` so
//! callers see an empty cache (one provider call per source) rather than a
//! cryptic decode error. New chartml releases bump `BLOB_VERSION` whenever the
//! tail-json shape or the byte framing changes.
//!
//! ## Threading
//!
//! IndexedDB lives on the browser's main thread; every `JsValue` op is
//! `!Send`. The [`CacheBackend`](crate::resolver::cache::CacheBackend) trait
//! is already declared `?Send` on `wasm32` via `cfg_attr`, so this struct
//! matches that bound automatically.
//!
//! ## Cross-user isolation
//!
//! Browser storage is **origin-scoped** — every user logged into a shared
//! machine sees the same IndexedDB unless the host app namespaces explicitly.
//! `IndexedDbBackend::new` accepts `namespace` as a required parameter (no
//! `Default` impl, no `Option`) precisely so this is a conscious decision per
//! consumer. Typical values: workspace id, tenant slug, or `"user-{uuid}"`.

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use idb::{
    CursorDirection, Database, DatabaseEvent, Factory, KeyRange, ObjectStoreParams,
    TransactionMode,
};
use js_sys::Uint8Array;
use wasm_bindgen::JsValue;

use crate::resolver::backends::codec::{decode_blob, encode_blob, entry_has_tag, DecodeError};
use crate::resolver::cache::{CacheBackend, CacheError, CachedEntry};

/// Single object store name we read/write under. One store keeps the
/// version-bump dance trivial (we always create exactly one in
/// `on_upgrade_needed`); namespace isolation is implemented at the key level.
const ENTRIES_STORE: &str = "entries";

/// Persistent tier-2 [`CacheBackend`] backed by IndexedDB.
///
/// Construct via [`IndexedDbBackend::new`], then hand to the resolver via
/// `ChartML::set_persistent_cache(...)` (or `Resolver::set_persistent_cache`
/// for direct callers). Cheap to clone — internally a single
/// `Rc<RefCell<Option<Database>>>` so multiple holders share one DB handle.
///
/// # Cross-user isolation
///
/// IndexedDB is origin-scoped. On a shared browser, every logged-in user sees
/// the **same** IndexedDB database for the same origin unless namespaces
/// differ. Construction therefore **requires** a namespace string — there is
/// no `Default` impl and no zero-argument constructor. Callers must pick a
/// namespace that varies per user (workspace id, tenant slug, `user-{uuid}`,
/// …) so user A cannot read user B's cached query results.
///
/// # Example
///
/// ```ignore
/// use chartml_core::resolver::backends::indexeddb::IndexedDbBackend;
/// use std::sync::Arc;
///
/// let backend = IndexedDbBackend::new("chartml", "workspace-42").await?;
/// chartml.resolver().set_persistent_cache(Arc::new(backend));
/// ```
#[derive(Clone)]
pub struct IndexedDbBackend {
    /// Database handle. Wrapped in `Rc<RefCell<Option<_>>>` so `shutdown()`
    /// can take the inner value (`Database::close` consumes by value), while
    /// the backend itself stays cheaply cloneable for multi-tier wiring.
    db: Rc<RefCell<Option<Database>>>,
    /// Namespace prefix folded into every cache key. Stored as `Rc<str>` to
    /// keep clones cheap (the resolver swaps backend `Arc`s atomically).
    namespace: Rc<str>,
}

impl IndexedDbBackend {
    /// Open (or create) the named IndexedDB database under the given
    /// namespace.
    ///
    /// `database_name` is the IndexedDB database name (origin-scoped). Pick a
    /// stable string like `"chartml-cache"` so subsequent sessions can
    /// reattach to the same store rather than creating fresh databases on
    /// every page load.
    ///
    /// `namespace` is **required** and folded into every key so multiple
    /// users/tenants on the same browser cannot read each other's cached
    /// results. There is no default namespace — pass workspace id, tenant
    /// slug, `"user-{uuid}"`, or similar.
    ///
    /// # Constraints
    ///
    /// `namespace` must be **non-empty** and must **not contain `:`**. The
    /// colon is the key-format separator (`"{namespace}:{key:016x}"`); a
    /// namespace like `"user"` and `"user:alice"` would otherwise share the
    /// same `["user:", "user;")` lexicographic key range, so namespace-scoped
    /// operations (`clear`, `invalidate_by_tag`) on `"user"` would silently
    /// nuke `"user:alice"` entries. Rejecting the colon up-front keeps the
    /// constraint visible at construction instead of as a corruption-by-design
    /// failure mode discovered in production.
    ///
    /// # Errors
    ///
    /// Returns `CacheError::Backend` when the namespace is empty or contains
    /// `:`, when the IndexedDB factory is unavailable (e.g. private browsing
    /// on some browsers, IDB disabled by policy), or when the upgrade
    /// transaction fails to create the `entries` object store.
    pub async fn new(database_name: &str, namespace: &str) -> Result<Self, CacheError> {
        if namespace.is_empty() {
            return Err(CacheError::Backend(
                "IndexedDbBackend namespace must be non-empty (cross-user isolation)".to_string(),
            ));
        }
        if namespace.contains(':') {
            return Err(CacheError::Backend(
                "IndexedDbBackend namespace must not contain ':' (key separator)".to_string(),
            ));
        }

        let factory =
            Factory::new().map_err(|e| CacheError::Backend(format!("idb factory: {e}")))?;
        let mut open_request = factory
            .open(database_name, Some(1))
            .map_err(|e| CacheError::Backend(format!("idb open request: {e}")))?;

        // Create the single entries store on first open. The closure runs on
        // the main thread synchronously while the upgrade transaction is
        // active — `database()` returns the in-flight DB handle for that
        // transaction, NOT the eventually-resolved post-open handle.
        open_request.on_upgrade_needed(|event| {
            let db = match event.database() {
                Ok(db) => db,
                Err(_) => return,
            };
            // `store_names()` is a `Vec<String>`; only create when missing so
            // re-open at the same version is a no-op.
            if !db.store_names().iter().any(|n| n == ENTRIES_STORE) {
                let _ = db.create_object_store(ENTRIES_STORE, ObjectStoreParams::new());
            }
        });

        let db: Database = open_request
            .await
            .map_err(|e| CacheError::Backend(format!("idb open: {e}")))?;

        Ok(Self {
            db: Rc::new(RefCell::new(Some(db))),
            namespace: Rc::from(namespace),
        })
    }

    /// IndexedDB key for a (namespace, hash-key) pair. Hex formatting of the
    /// `u64` keeps keys ASCII so the browser DevTools UI shows readable
    /// values, and the fixed 16-char width makes the key range bounds below
    /// trivial to compute.
    fn db_key(&self, key: u64) -> String {
        format!("{}:{:016x}", self.namespace, key)
    }

    /// Inclusive lower bound for the namespace's key range — `"{ns}:"` (one
    /// past `"{ns}"`-itself, but before any `"{ns}:0…"` entry).
    fn namespace_lower_bound(&self) -> String {
        format!("{}:", self.namespace)
    }

    /// Inclusive upper bound for the namespace's key range. The trailing
    /// `;` is `':' + 1` lexicographically, so the closed bound `["{ns}:",
    /// "{ns};"]` cleanly captures everything keyed under the namespace and
    /// nothing keyed under `"{ns}-suffix"` or any unrelated namespace.
    fn namespace_upper_bound(&self) -> String {
        format!("{};", self.namespace)
    }

    /// Borrow the live database handle. `None` if `shutdown()` already ran
    /// (subsequent ops then return `CacheError::Backend("database closed")`).
    fn with_db<F, R>(&self, op: F) -> Result<R, CacheError>
    where
        F: FnOnce(&Database) -> Result<R, CacheError>,
    {
        let guard = self.db.borrow();
        let db = guard
            .as_ref()
            .ok_or_else(|| CacheError::Backend("database closed".to_string()))?;
        op(db)
    }
}

impl std::fmt::Debug for IndexedDbBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexedDbBackend")
            .field("namespace", &&*self.namespace)
            .field("open", &self.db.borrow().is_some())
            .finish()
    }
}

// SAFETY: `IndexedDbBackend` is only ever compiled for
// `wasm32-unknown-unknown`, where the runtime is strictly single-threaded —
// JavaScript on the main thread, no shared-memory worker access to these
// values. The `Rc<RefCell<…>>` and `Rc<str>` fields therefore can never be
// observed from a second thread, so the `Send + Sync` bound required by the
// `CacheBackend` trait is trivially upheld. Removing these `unsafe impl`s
// would force every browser consumer to hand-roll `Mutex<Database>` for no
// observable benefit (the lock would never contend) and introduce poison
// failure modes that don't exist in single-threaded WASM.
//
// If/when wasm32 gains a real threading model (`wasm32-wasip2`, the
// shared-memory threads proposal, etc.) and chartml-core moves to it, these
// impls will need to be replaced with proper `Arc<Mutex<…>>` wrappers — but
// the `CacheBackend` trait itself would also need to relax its bound for
// that target, which is out of scope for phase 3b.
//
// TODO: Phase X — loosen CacheBackend supertrait to cfg-gated bound.
// Replace the unconditional `Send + Sync` requirement on `CacheBackend` (in
// `resolver/cache.rs`) with a `cfg_attr(not(target_arch = "wasm32"), …)`
// bound that mirrors the existing `?Send` async-trait split. Once that lands
// these `unsafe impl`s can be deleted entirely (and the `unsafe` keyword can
// leave the file along with them). Tracked as tech debt because `cache.rs`
// is owned by phase 3, not 3b.
unsafe impl Send for IndexedDbBackend {}
unsafe impl Sync for IndexedDbBackend {}

#[async_trait(?Send)]
impl CacheBackend for IndexedDbBackend {
    async fn get(&self, key: u64) -> Option<CachedEntry> {
        let db_key = self.db_key(key);

        // Fetch the raw blob inside a read-only transaction.
        let raw: Option<Vec<u8>> = match self.read_blob(&db_key).await {
            Ok(blob) => blob,
            Err(_) => return None,
        };

        let raw = raw?;

        match decode_blob(&raw) {
            Ok(entry) => Some(entry),
            Err(DecodeError::VersionMismatch) => {
                // Stale on-disk format — evict so subsequent gets don't keep
                // hitting the failure path. Fire-and-forget: errors here just
                // surface as another cache miss, never propagate.
                let _ = self.invalidate(key).await;
                None
            }
            Err(_) => None,
        }
    }

    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError> {
        let bytes = encode_blob(&entry)
            .map_err(|e| CacheError::Backend(format!("encode entry: {e}")))?;
        let db_key = self.db_key(key);

        // Build the put request inside `with_db` so the borrow on `self.db`
        // is released before we await. IndexedDB transactions auto-commit
        // when control returns to the event loop, so awaiting the request
        // outside the borrow is the canonical pattern.
        let (req, tx) = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadWrite)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let value = Uint8Array::from(bytes.as_slice());
            let key_js = JsValue::from_str(&db_key);
            let req = store
                .put(&value, Some(&key_js))
                .map_err(|e| classify_idb_error("put", &e))?;
            Ok((req, tx))
        })?;

        req.await
            .map_err(|e| classify_idb_error("put-await", &e))?;
        tx.await
            .map_err(|e| CacheError::Backend(format!("idb tx commit: {e}")))?;
        Ok(())
    }

    async fn invalidate(&self, key: u64) -> Result<(), CacheError> {
        let db_key = self.db_key(key);
        let (req, tx) = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadWrite)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let req = store
                .delete(JsValue::from_str(&db_key))
                .map_err(|e| CacheError::Backend(format!("idb delete: {e}")))?;
            Ok((req, tx))
        })?;
        req.await
            .map_err(|e| CacheError::Backend(format!("idb delete-await: {e}")))?;
        tx.await
            .map_err(|e| CacheError::Backend(format!("idb tx commit: {e}")))?;
        Ok(())
    }

    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError> {
        // Two-pass: collect matching keys via cursor, then delete them in a
        // fresh transaction. We don't `delete()` from inside the cursor
        // because mixing reads + writes against the same row mid-iteration is
        // brittle across IDB implementations.
        let matching = self.collect_keys_with_tag(tag).await?;
        if matching.is_empty() {
            return Ok(());
        }

        let (reqs, tx) = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadWrite)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let mut reqs = Vec::with_capacity(matching.len());
            for key in &matching {
                let req = store
                    .delete(JsValue::from_str(key))
                    .map_err(|e| CacheError::Backend(format!("idb delete: {e}")))?;
                reqs.push(req);
            }
            Ok((reqs, tx))
        })?;

        for req in reqs {
            req.await
                .map_err(|e| CacheError::Backend(format!("idb delete-await: {e}")))?;
        }
        tx.await
            .map_err(|e| CacheError::Backend(format!("idb tx commit: {e}")))?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        // Namespace-scoped clear — we can't drop the whole object store
        // without affecting other namespaces sharing this database. Iterate
        // the namespace's key range with a cursor and collect keys to delete.
        let lower = self.namespace_lower_bound();
        let upper = self.namespace_upper_bound();

        let mut keys = Vec::new();
        let cursor_opt = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadOnly)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let range = KeyRange::bound(
                &JsValue::from_str(&lower),
                &JsValue::from_str(&upper),
                Some(false),
                Some(true),
            )
            .map_err(|e| CacheError::Backend(format!("idb range: {e}")))?;
            let req = store
                .open_cursor(Some(range.into()), Some(CursorDirection::Next))
                .map_err(|e| CacheError::Backend(format!("idb open cursor: {e}")))?;
            Ok(req)
        })?;

        let mut maybe_cursor = cursor_opt
            .await
            .map_err(|e| CacheError::Backend(format!("idb cursor await: {e}")))?;
        while let Some(cursor) = maybe_cursor {
            let key_js = cursor
                .key()
                .map_err(|e| CacheError::Backend(format!("idb cursor key: {e}")))?;
            if let Some(key_str) = key_js.as_string() {
                keys.push(key_str);
            }
            maybe_cursor = cursor
                .next(None)
                .map_err(|e| CacheError::Backend(format!("idb cursor next: {e}")))?
                .await
                .map_err(|e| CacheError::Backend(format!("idb cursor next-await: {e}")))?;
        }

        if keys.is_empty() {
            return Ok(());
        }

        let (reqs, tx) = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadWrite)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let mut reqs = Vec::with_capacity(keys.len());
            for k in &keys {
                let req = store
                    .delete(JsValue::from_str(k))
                    .map_err(|e| CacheError::Backend(format!("idb delete: {e}")))?;
                reqs.push(req);
            }
            Ok((reqs, tx))
        })?;
        for req in reqs {
            req.await
                .map_err(|e| CacheError::Backend(format!("idb delete-await: {e}")))?;
        }
        tx.await
            .map_err(|e| CacheError::Backend(format!("idb tx commit: {e}")))?;
        Ok(())
    }

    async fn shutdown(&self) {
        // `Database::close` takes `&self` per idb 0.6.5, so we just call it
        // through the Option and then take() to drop the handle (subsequent
        // ops will then surface "database closed").
        let mut guard = self.db.borrow_mut();
        if let Some(db) = guard.as_ref() {
            db.close();
        }
        *guard = None;
    }
}

impl IndexedDbBackend {
    /// Read the raw stored bytes for the given IndexedDB key. Returns
    /// `Ok(None)` when the key isn't present.
    async fn read_blob(&self, db_key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let (req, tx) = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadOnly)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let req = store
                .get(JsValue::from_str(db_key))
                .map_err(|e| CacheError::Backend(format!("idb get: {e}")))?;
            Ok((req, tx))
        })?;
        let value = req
            .await
            .map_err(|e| CacheError::Backend(format!("idb get-await: {e}")))?;
        // ReadOnly tx still needs awaiting so any internal aborts surface;
        // we ignore commit errors here since the read already produced its
        // value successfully.
        let _ = tx.await;
        match value {
            None => Ok(None),
            Some(jv) => {
                if jv.is_undefined() || jv.is_null() {
                    return Ok(None);
                }
                let arr = Uint8Array::new(&jv);
                Ok(Some(arr.to_vec()))
            }
        }
    }

    /// Iterate the namespace's key range, decoding each entry's tail JSON to
    /// match against `tag`. Returns the matching IndexedDB keys.
    async fn collect_keys_with_tag(&self, tag: &str) -> Result<Vec<String>, CacheError> {
        let lower = self.namespace_lower_bound();
        let upper = self.namespace_upper_bound();

        let cursor_req = self.with_db(|db| {
            let tx = db
                .transaction(&[ENTRIES_STORE], TransactionMode::ReadOnly)
                .map_err(|e| CacheError::Backend(format!("idb tx: {e}")))?;
            let store = tx
                .object_store(ENTRIES_STORE)
                .map_err(|e| CacheError::Backend(format!("idb store: {e}")))?;
            let range = KeyRange::bound(
                &JsValue::from_str(&lower),
                &JsValue::from_str(&upper),
                Some(false),
                Some(true),
            )
            .map_err(|e| CacheError::Backend(format!("idb range: {e}")))?;
            let req = store
                .open_cursor(Some(range.into()), Some(CursorDirection::Next))
                .map_err(|e| CacheError::Backend(format!("idb open cursor: {e}")))?;
            Ok(req)
        })?;

        let mut maybe_cursor = cursor_req
            .await
            .map_err(|e| CacheError::Backend(format!("idb cursor await: {e}")))?;
        let mut matching = Vec::new();
        while let Some(cursor) = maybe_cursor {
            let key_js = cursor
                .key()
                .map_err(|e| CacheError::Backend(format!("idb cursor key: {e}")))?;
            let value_js = cursor
                .value()
                .map_err(|e| CacheError::Backend(format!("idb cursor value: {e}")))?;
            if let (Some(key_str), Some(arr)) = (
                key_js.as_string(),
                (!value_js.is_undefined() && !value_js.is_null())
                    .then(|| Uint8Array::new(&value_js)),
            ) {
                let bytes = arr.to_vec();
                if entry_has_tag(&bytes, tag) {
                    matching.push(key_str);
                }
            }
            maybe_cursor = cursor
                .next(None)
                .map_err(|e| CacheError::Backend(format!("idb cursor next: {e}")))?
                .await
                .map_err(|e| CacheError::Backend(format!("idb cursor next-await: {e}")))?;
        }
        Ok(matching)
    }
}

/// Map an `idb::Error` to a `CacheError`. Quota-exceeded errors get a
/// human-readable prefix so resolver consumers can pattern-match `"quota"` in
/// log output without needing a dedicated error variant — this avoids
/// breaking the existing `CacheError` enum's API while still giving callers
/// enough info to react.
fn classify_idb_error(stage: &str, err: &idb::Error) -> CacheError {
    let msg = err.to_string();
    let lc = msg.to_lowercase();
    if lc.contains("quota") {
        CacheError::Backend(format!("idb {stage}: storage quota exceeded ({msg})"))
    } else {
        CacheError::Backend(format!("idb {stage}: {msg}"))
    }
}

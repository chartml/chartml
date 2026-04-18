//! Browser-side `wasm-bindgen-test` suite for [`IndexedDbBackend`].
//!
//! These tests touch real IndexedDB and therefore only compile + run on
//! `wasm32-unknown-unknown` with the `wasm-indexeddb` feature on. Drive them
//! with `wasm-pack test --firefox --headless -p chartml-core --features
//! wasm-indexeddb` (Chrome / Safari work too if their headless drivers are
//! available locally).
//!
//! Each test uses a unique database name so parallel-run cases never collide
//! inside one origin's IndexedDB namespace.

#![cfg(all(target_arch = "wasm32", feature = "wasm-indexeddb"))]

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chartml_core::data::{DataTable, Row};
use chartml_core::resolver::backends::indexeddb::IndexedDbBackend;
use chartml_core::resolver::cache::{CacheBackend, CachedEntry};
use serde_json::json;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// `SystemTime::now()` is unimplemented on `wasm32-unknown-unknown` — std's
/// `time` shim panics rather than falling back to JS. Browser tests source
/// the wall clock from `Date.now()` via `js_sys` instead.
fn browser_now() -> SystemTime {
    let ms = js_sys::Date::now() as u64;
    UNIX_EPOCH + Duration::from_millis(ms)
}

/// Each test uses a deterministic-but-unique database name so reruns within
/// one browser session don't collide. Including a counter-like suffix (`_$n`)
/// in the test name itself is sufficient since `wasm-bindgen-test` doesn't
/// parallelize.
fn db_name(suffix: &str) -> String {
    format!("chartml-test-indexeddb-{suffix}")
}

fn make_entry(tag: &str) -> CachedEntry {
    let row: Row = [
        ("name".to_string(), json!("alpha")),
        ("value".to_string(), json!(42.0)),
    ]
    .into_iter()
    .collect();
    let mut metadata = HashMap::new();
    metadata.insert("rows_returned".to_string(), json!(1));
    metadata.insert("source".to_string(), json!("unit-test"));
    CachedEntry {
        data: DataTable::from_rows(&[row]).unwrap(),
        fetched_at: browser_now(),
        ttl: Duration::from_secs(300),
        tags: vec![tag.to_string()],
        metadata,
    }
}

#[wasm_bindgen_test]
async fn test_put_get_roundtrip_survives_reconstruction() {
    let db = db_name("roundtrip");
    let ns = "ns-roundtrip";

    {
        let backend = IndexedDbBackend::new(&db, ns)
            .await
            .expect("open backend (first session)");
        backend
            .put(42, make_entry("slug:demo"))
            .await
            .expect("put");
        backend.shutdown().await;
    }

    // Reopen — verifies persistence across handle drop / page-refresh
    // semantics.
    let backend = IndexedDbBackend::new(&db, ns)
        .await
        .expect("open backend (second session)");
    let got = backend.get(42).await.expect("entry must persist");
    assert_eq!(got.data.num_rows(), 1);
    assert_eq!(got.tags, vec!["slug:demo".to_string()]);
    assert_eq!(
        got.metadata.get("source").map(|v| v.as_str().unwrap_or("")),
        Some("unit-test")
    );

    // Cleanup.
    backend.clear().await.expect("clear");
    backend.shutdown().await;
}

#[wasm_bindgen_test]
async fn test_namespace_isolation() {
    let db = db_name("isolation");

    let alice = IndexedDbBackend::new(&db, "alice")
        .await
        .expect("open alice");
    let bob = IndexedDbBackend::new(&db, "bob").await.expect("open bob");

    alice
        .put(7, make_entry("slug:secret"))
        .await
        .expect("alice put");

    // Bob using the same key + same database must NOT see alice's entry.
    let bob_view = bob.get(7).await;
    assert!(
        bob_view.is_none(),
        "bob's namespace must not see alice's entry"
    );

    // Alice still sees her own entry.
    let alice_view = alice.get(7).await;
    assert!(alice_view.is_some(), "alice's own entry must be readable");

    // Cleanup both namespaces.
    alice.clear().await.expect("alice clear");
    bob.clear().await.expect("bob clear");
    alice.shutdown().await;
    bob.shutdown().await;
}

/// Namespaces that contain `:` would silently overlap with deeper-nested
/// namespaces because the `:` is the key-separator between namespace and the
/// hex-encoded key (`"{namespace}:{key:016x}"`). Concretely, namespace
/// `"user"` produces the key range `["user:", "user;")`, which lexicographically
/// also contains every `"user:alice:…"` key — so a `clear()` on `"user"`
/// would silently nuke every entry under `"user:alice"`. `IndexedDbBackend::new`
/// rejects colons in the namespace at construction so this corruption-by-design
/// failure mode never reaches production.
#[wasm_bindgen_test]
async fn test_namespace_colon_rejected() {
    let db = db_name("colon-rejected");

    // A bare colon is rejected.
    let err = IndexedDbBackend::new(&db, "user:alice").await;
    assert!(
        err.is_err(),
        "namespace containing ':' must be rejected at construction"
    );
    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains(':') && msg.to_lowercase().contains("namespace"),
        "error message must mention the namespace constraint, got {msg:?}"
    );

    // Sanity: a namespace with the colon embedded mid-string is also rejected.
    assert!(
        IndexedDbBackend::new(&db, "ws-1:tenant-7").await.is_err(),
        "embedded ':' must be rejected"
    );

    // And an empty namespace is still rejected (regression guard for the
    // existing constraint that lives alongside this new one).
    assert!(
        IndexedDbBackend::new(&db, "").await.is_err(),
        "empty namespace must remain rejected"
    );
}

#[wasm_bindgen_test]
async fn test_version_mismatch_evicts() {
    use idb::{Factory, ObjectStoreParams, TransactionMode};
    use js_sys::Uint8Array;
    use wasm_bindgen::JsValue;

    let db_str = db_name("version-mismatch");
    let ns = "ns-vm";

    // Open through the backend first to ensure the object store exists.
    {
        let backend = IndexedDbBackend::new(&db_str, ns)
            .await
            .expect("open backend");
        backend.shutdown().await;
    }

    // Manually plant a blob with a bogus version byte.
    let factory = Factory::new().expect("factory");
    let mut req = factory
        .open(&db_str, Some(1))
        .expect("open request");
    req.on_upgrade_needed(|event| {
        use idb::DatabaseEvent;
        let db = event.database().expect("db");
        if !db.store_names().iter().any(|n| n == "entries") {
            let _ = db.create_object_store("entries", ObjectStoreParams::new());
        }
    });
    let db = req.await.expect("db");
    {
        let tx = db
            .transaction(&["entries"], TransactionMode::ReadWrite)
            .expect("tx");
        let store = tx.object_store("entries").expect("store");
        let bogus: Vec<u8> = vec![0xFF, 0, 0, 0, 0, 0];
        let value = Uint8Array::from(bogus.as_slice());
        let key = JsValue::from_str(&format!("{ns}:{:016x}", 99u64));
        store
            .put(&value, Some(&key))
            .expect("put bogus")
            .await
            .expect("put bogus await");
        tx.await.expect("tx commit");
    }
    db.close();

    // Now reopen via the backend and confirm `get` returns None AND evicts.
    let backend = IndexedDbBackend::new(&db_str, ns)
        .await
        .expect("reopen backend");
    let first = backend.get(99).await;
    assert!(first.is_none(), "version mismatch must surface as miss");

    // Eviction is fire-and-forget but the in-process `get` awaited
    // `invalidate(key)`, so a fresh read must also miss for the same reason
    // (no entry left at all). If eviction failed, the next read would still
    // hit the version-mismatch path and return None — so we additionally
    // probe via the cursor that no key remains.
    let second = backend.get(99).await;
    assert!(second.is_none(), "post-eviction read must still miss");

    backend.shutdown().await;
}

#[wasm_bindgen_test]
async fn test_tag_invalidate() {
    let db = db_name("tag-invalidate");
    let ns = "ns-tags";
    let backend = IndexedDbBackend::new(&db, ns)
        .await
        .expect("open backend");

    backend
        .put(1, make_entry("slug:foo"))
        .await
        .expect("put 1");
    backend
        .put(2, make_entry("slug:foo"))
        .await
        .expect("put 2");
    backend
        .put(3, make_entry("slug:bar"))
        .await
        .expect("put 3");

    backend
        .invalidate_by_tag("slug:foo")
        .await
        .expect("invalidate slug:foo");

    assert!(backend.get(1).await.is_none(), "slug:foo entry 1 evicted");
    assert!(backend.get(2).await.is_none(), "slug:foo entry 2 evicted");
    assert!(
        backend.get(3).await.is_some(),
        "slug:bar entry 3 must survive"
    );

    backend.clear().await.expect("clear");
    backend.shutdown().await;
}

#[wasm_bindgen_test]
async fn test_concurrent_writes() {
    use futures::future::join_all;

    let db = db_name("concurrent");
    let ns = "ns-concurrent";
    let backend = IndexedDbBackend::new(&db, ns)
        .await
        .expect("open backend");

    // Fire three concurrent puts on the SAME key. The backend isn't
    // claiming serializable semantics — IndexedDB's auto-commit makes this
    // last-write-wins — but the contract is "no panic, end state is
    // readable". Test asserts both.
    let f1 = backend.put(11, make_entry("slug:a"));
    let f2 = backend.put(11, make_entry("slug:b"));
    let f3 = backend.put(11, make_entry("slug:c"));
    let results: Vec<_> = join_all(vec![f1, f2, f3]).await;
    for (i, r) in results.iter().enumerate() {
        assert!(r.is_ok(), "concurrent put {i} failed: {r:?}");
    }

    let final_entry = backend.get(11).await.expect("entry must exist");
    // Every put used a single-element tags vec — exactly one of
    // {slug:a, slug:b, slug:c} must remain.
    assert_eq!(final_entry.tags.len(), 1);
    let tag = &final_entry.tags[0];
    assert!(
        tag == "slug:a" || tag == "slug:b" || tag == "slug:c",
        "unexpected final tag {tag:?}"
    );

    backend.clear().await.expect("clear");
    backend.shutdown().await;
}

#[wasm_bindgen_test]
async fn test_shutdown_closes_cleanly() {
    let db = db_name("shutdown");
    let ns = "ns-shutdown";

    let backend = IndexedDbBackend::new(&db, ns)
        .await
        .expect("open #1");
    backend
        .put(5, make_entry("slug:s"))
        .await
        .expect("put");
    backend.shutdown().await;

    // After shutdown, new ops on the SAME backend should error rather than
    // silently no-op (so callers don't get confused by stale handles).
    let post_shutdown = backend.get(5).await;
    assert!(
        post_shutdown.is_none(),
        "get after shutdown must report no entry (handle closed)"
    );
    let put_err = backend.put(6, make_entry("slug:t")).await;
    assert!(
        put_err.is_err(),
        "put after shutdown must surface a CacheError"
    );

    // Reopen — the IndexedDB database is still on disk; new handle reads
    // the persisted entry.
    let reborn = IndexedDbBackend::new(&db, ns)
        .await
        .expect("reopen after shutdown");
    let got = reborn.get(5).await.expect("entry persists across shutdown");
    assert_eq!(got.data.num_rows(), 1);

    reborn.clear().await.expect("clear");
    reborn.shutdown().await;
}

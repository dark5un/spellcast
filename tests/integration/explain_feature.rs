// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the explain feature.

use spellcast::explainer::{Explainer, ExplainerConfig, ExplainSource};
use spellcast::memory::MemoryStore;

#[test]
fn test_explain_with_cache() {
    let store = MemoryStore::open_in_memory().unwrap();
    store.store_explanation("prose", "a greeting", "hello").unwrap();

    let config = ExplainerConfig {
        llm_available: false,
        web_search_available: true,
        ..Default::default()
    };

    let explainer = Explainer::new(config).with_memory(store);
    let result = explainer.explain("a greeting", "prose").unwrap();

    assert_eq!(result.token, "hello");
    assert_eq!(result.source, ExplainSource::LocalCache);
}

#[test]
fn test_explain_with_web_search_fallback() {
    let config = ExplainerConfig {
        llm_available: false,
        web_search_available: true,
        ..Default::default()
    };

    let explainer = Explainer::new(config);
    let result = explainer.explain("a collection of items", "prose").unwrap();

    assert_eq!(result.token, "array");
    assert_eq!(result.source, ExplainSource::WebSearch);
}

#[test]
fn test_explain_caches_result() {
    let store = MemoryStore::open_in_memory().unwrap();

    let config = ExplainerConfig {
        llm_available: false,
        web_search_available: true,
        ..Default::default()
    };

    let explainer = Explainer::new(config).with_memory(store);

    // First call should hit web search
    let result1 = explainer.explain("a key value store", "prose").unwrap();
    assert_eq!(result1.token, "hash map");

    // Second call should hit the cache
    let result2 = explainer.explain("a key value store", "prose").unwrap();
    assert_eq!(result2.token, "hash map");
    assert_eq!(result2.source, ExplainSource::LocalCache);
}

#[test]
fn test_explain_fallback_to_raw_text() {
    let config = ExplainerConfig {
        llm_available: false,
        web_search_available: true,
        ..Default::default()
    };

    let explainer = Explainer::new(config);
    let result = explainer.explain("some completely novel concept", "prose").unwrap();

    // Should fall back to the raw explanation text
    assert_eq!(result.token, "some completely novel concept");
}
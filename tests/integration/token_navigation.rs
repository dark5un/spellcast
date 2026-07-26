// SPDX-License-Identifier: Apache-2.0

//! Integration tests for token navigation and editing.

use voxkey::modes::{Mode, ModeController};
use voxkey::tokenizer::{HeuristicTokenizer, Token, TokenContext, TokenStream, TokenType, Tokenizer};
use voxkey::predictor::Predictor;

#[test]
fn test_token_navigation_basic() {
    let tokenizer = HeuristicTokenizer::new();
    let stream = tokenizer.tokenize("hello beautiful world").unwrap();

    assert_eq!(stream.len(), 5); // hello, whitespace, beautiful, whitespace, world

    // Navigate left and right
    let mut idx: Option<usize> = Some(0);
    assert_eq!(stream.get(idx.unwrap()).unwrap().text, "hello");

    idx = Some(2);
    assert_eq!(stream.get(idx.unwrap()).unwrap().text, "beautiful");

    idx = Some(4);
    assert_eq!(stream.get(idx.unwrap()).unwrap().text, "world");
}

#[test]
fn test_token_deletion() {
    let tokenizer = HeuristicTokenizer::new();
    let mut stream = tokenizer.tokenize("delete this word").unwrap();
    let original_len = stream.len();

    // Remove the middle word token (index 2)
    stream.remove(2);
    assert_eq!(stream.len(), original_len - 1);
}

#[test]
fn test_token_replacement() {
    let tokenizer = HeuristicTokenizer::new();
    let mut stream = tokenizer.tokenize("hello world").unwrap();

    let replacement = Token {
        text: "hi".to_string(),
        offset: 0,
        length: 2,
        token_type: TokenType::Word,
    };

    let old = stream.replace(0, replacement).unwrap();
    assert_eq!(old.text, "hello");
    assert_eq!(stream.get(0).unwrap().text, "hi");
}

#[test]
fn test_mode_controller_integration() {
    let mut mc = ModeController::new();
    assert_eq!(mc.current_mode(), Mode::Raw);

    mc.toggle_mode();
    assert_eq!(mc.current_mode(), Mode::Dictation);

    mc.engage_kill_switch();
    assert_eq!(mc.current_mode(), Mode::Killed);

    mc.toggle_kill_switch();
    assert_eq!(mc.current_mode(), Mode::Raw);
}

#[test]
fn test_predictor_basic_integration() {
    let mut predictor = Predictor::new();
    predictor.build_index(["hello", "help", "helm", "held", "world"]);

    let results = predictor.predict("hello", 3).unwrap();
    assert!(!results.is_empty());
    assert!(results.len() <= 3);

    // All results should have a distance value
    for r in &results {
        assert!(r.word != "hello"); // shouldn't return the input
    }
}

#[test]
fn test_heuristic_tokenizer_code_detection() {
    let tokenizer = HeuristicTokenizer::new();

    // Code snippets should be detected as code
    let code = "fn foo() -> Result<()> { Ok(()) }";
    assert_eq!(tokenizer.detect_context(code), TokenContext::Code);

    // Prose should be detected as prose
    let prose = "The quick brown fox jumps over the lazy dog.";
    assert_eq!(tokenizer.detect_context(prose), TokenContext::Prose);
}
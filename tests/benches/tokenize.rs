// SPDX-License-Identifier: Apache-2.0

//! Tokenization benchmarks.

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use spellcast::tokenizer::{HeuristicTokenizer, Tokenizer};

fn benchmark_heuristic_tokenizer(c: &mut Criterion) {
    let tokenizer = HeuristicTokenizer::new();

    let prose_text =
        "hello world, this is a test of the tokenizer system. how are you doing today?";
    let code_text = "fn main() { let x = fooBar::new(); x.bazQux(42); }";

    c.bench_function("tokenize_prose", |b| {
        b.iter(|| {
            let result = tokenizer.tokenize(black_box(prose_text)).unwrap();
            black_box(result);
        });
    });

    c.bench_function("tokenize_code", |b| {
        b.iter(|| {
            let result = tokenizer.tokenize(black_box(code_text)).unwrap();
            black_box(result);
        });
    });
}

fn benchmark_context_detection(c: &mut Criterion) {
    let tokenizer = HeuristicTokenizer::new();

    c.bench_function("detect_context_code", |b| {
        b.iter(|| {
            let ctx = tokenizer.detect_context(black_box("fn foo() { let x = 42; }"));
            black_box(ctx);
        });
    });

    c.bench_function("detect_context_prose", |b| {
        b.iter(|| {
            let ctx = tokenizer.detect_context(black_box("hello world, how are you?"));
            black_box(ctx);
        });
    });
}

criterion_group! {
    name = tokenize;
    config = Criterion::default().sample_size(100);
    targets = benchmark_heuristic_tokenizer, benchmark_context_detection
}

criterion_main!(tokenize);

// Copyright (C) 2026 Kravex, Inc.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file and at www.mariadb.com/bsl11.
// ai
//! 🧵📊🚀 Refiner Benchmark Suite — "The Thread Redemption"
//!
//! It was a quiet Tuesday. The refiners were just sitting there, accumulating drafts,
//! tapping documents, joining drums. Nobody knew how fast they really were.
//! Nobody *asked*. Until now.
//!
//! This benchmark answers the question: "How many MB/s and docs/s can a single
//! Refiner thread push through ch1 → plenum → manifold.join → ch2?"
//!
//! Auto-discovers all `ManifoldBackend` variants via `all_variants()` — add a new
//! manifold and it gets benched for free. Like a gym membership you actually use.
//!
//! 🦆 The duck wonders if we're benchmarking the refiner or the channel. Yes.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kvx_lib::taps::passthrough::Passthrough;
use kvx_lib::taps::BarrelToDraftsTapper;
use kvx_lib::manifolds::ManifoldBackend;
use kvx_lib::workers::Refiner;
use kvx_lib::{Barrel, Drum};
use std::hint::black_box;

// -- 📏 Doc counts to sweep — enough range to see if throughput scales linearly
// -- 1K = warm-up vibes, 10K = realistic batch, 50K = stress test energy
const DOC_COUNTS: &[usize] = &[1_000, 10_000, 50_000];

// -- 📐 Channel capacity — big enough that the sender doesn't block on the refiner
// -- but not so big that we're benchmarking malloc instead of join logic
const CHANNEL_CAPACITY: usize = 1024;

// -- 📏 Max request size for the refiner — 10 MiB, large enough that we flush on close
// -- not mid-stream, so we measure join throughput without flush chatter
const MAX_REQUEST_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// 🧱 Generate N synthetic JSON barrels — pre-allocated outside the hot path.
///
/// Each doc: `{"id":42,"name":"bench_doc_42","data":"aaaa..."}` ≈ 100 bytes
/// The padding ensures we're not just benchmarking `format!("{}")` on tiny strings.
///
/// "He who generates test data inline, benchmarks allocation, not logic." — Ancient proverb 📜
fn generate_barrels(count: usize) -> Vec<String> {
    // -- 🚀 pre-size the vec because reallocation mid-generation is for amateurs
    let mut barrels = Vec::with_capacity(count);
    for _ in 0..count {
        let mut barrel = String::new();
        for i in 0..count {
            barrel.push_str(&format!(
                r#"{{"id":{i},"name":"bench_doc_{i}","data":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}\n"#
            ));
        }
        barrels.push(barrel)
    }
    barrels
}

/// 📏 Total byte size of all barrels — for Throughput::Bytes reporting.
/// Counts raw barrel bytes, not post-join drum bytes, because we want to know
/// how fast the refiner *processes input*, not how big the output is. 🧮
fn total_barrel_bytes(barrels: &[String]) -> u64 {
    barrels.iter().map(|f| f.len() as u64).sum()
}

/// 🚀📡 Throughput in MB/s — how fast does the refiner chew through raw barrel bytes?
///
/// Iterates all ManifoldBackend variants × doc counts. Criterion plots MB/s curves.
/// If your manifold is slow, this benchmark will publicly shame it. No pressure.
fn refiner_throughput_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("refiner_throughput_bytes");

    for manifold in ManifoldBackend::all_variants() {
        for &doc_count in DOC_COUNTS {
            // -- 📦 Pre-generate barrels OUTSIDE the measured section
            let barrels = generate_barrels(doc_count);
            let total_bytes = total_barrel_bytes(&barrels);

            group.throughput(Throughput::Bytes(total_bytes));
            group.bench_with_input(
                BenchmarkId::new(format!("{:?}", manifold), doc_count),
                &doc_count,
                |b, &_n| {
                    b.iter(|| {
                        // -- 🧵 Fresh channels + refiner per iteration — no stale state leaking between runs
                        let (tx1, rx1) = async_channel::bounded::<Barrel>(CHANNEL_CAPACITY);
                        let (tx2, rx2) = async_channel::bounded::<Drum>(CHANNEL_CAPACITY);

                        let refiner = Refiner::new(
                            rx1,
                            tx2,
                            BarrelToDraftsTapper::Passthrough(Passthrough),
                            manifold.clone(),
                            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(MAX_REQUEST_SIZE_BYTES)),
                        );

                        // -- 🚀 Launch the refiner thread — it blocks on recv_blocking until barrels arrive
                        let the_refiner_handle = refiner.start();

                        // -- 📤 Sender thread: shove all barrels into ch1, then close
                        let barrels_clone = barrels.clone();
                        let sender_handle = std::thread::spawn(move || {
                            for barrel in barrels_clone {
                                tx1.send_blocking(Barrel(barrel)).unwrap();
                            }
                            // -- 🏁 Close ch1 — triggers refiner's final flush
                            drop(tx1);
                        });

                        // -- 📥 Main thread: drain ch2 until closed, black_box each drum
                        while let Ok(drum) = rx2.recv_blocking() {
                            black_box(drum);
                        }

                        // -- 🧹 Wait for threads to finish — clean exits only, no zombies 🧟
                        sender_handle.join().expect("💀 Sender thread panicked");
                        the_refiner_handle
                            .join()
                            .expect("💀 Refiner thread panicked")
                            .expect("💀 Refiner returned an error");
                    });
                },
            );
        }
    }
    group.finish();
}

/// 🚀🔢 Throughput in docs/s — how many barrels can the refiner process per second?
///
/// Same setup as bytes bench but with `Throughput::Elements`. Because sometimes you
/// want to know "how many docs" not "how many bytes." Both are valid life questions.
fn refiner_throughput_docs(c: &mut Criterion) {
    let mut group = c.benchmark_group("refiner_throughput_docs");

    for manifold in ManifoldBackend::all_variants() {
        for &doc_count in DOC_COUNTS {
            // -- 📦 Pre-generate barrels OUTSIDE the measured section
            let barrels = generate_barrels(doc_count);

            group.throughput(Throughput::Elements(doc_count as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{:?}", manifold), doc_count),
                &doc_count,
                |b, &_n| {
                    b.iter(|| {
                        // -- 🧵 Fresh channels + refiner per iteration
                        let (tx1, rx1) = async_channel::bounded::<Barrel>(CHANNEL_CAPACITY);
                        let (tx2, rx2) = async_channel::bounded::<Drum>(CHANNEL_CAPACITY);

                        let refiner = Refiner::new(
                            rx1,
                            tx2,
                            BarrelToDraftsTapper::Passthrough(Passthrough),
                            manifold.clone(),
                            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(MAX_REQUEST_SIZE_BYTES)),
                        );

                        // -- 🚀 Launch the refiner thread — it blocks on recv_blocking until barrels arrive
                        let the_refiner_handle = refiner.start();

                        // -- 📤 Sender thread: barrel the beast
                        let barrels_clone = barrels.clone();
                        let sender_handle = std::thread::spawn(move || {
                            for barrel in barrels_clone {
                                tx1.send_blocking(Barrel(barrel)).unwrap();
                            }
                            drop(tx1);
                        });

                        // -- 📥 Drain ch2 — every drum gets black_box'd so the optimizer
                        // -- doesn't get clever and optimize away our entire benchmark 🧠
                        while let Ok(drum) = rx2.recv_blocking() {
                            black_box(drum);
                        }

                        sender_handle.join().expect("💀 Sender thread panicked");
                        the_refiner_handle
                            .join()
                            .expect("💀 Refiner thread panicked")
                            .expect("💀 Refiner returned an error");
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, refiner_throughput_bytes, refiner_throughput_docs);
criterion_main!(benches);
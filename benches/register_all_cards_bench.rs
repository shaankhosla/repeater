use std::path::PathBuf;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use repeater::crud::DB;
use repeater::parser::register_all_cards;
use std::hint::black_box;
use tokio::runtime::Runtime;

fn bench_register_all_cards(c: &mut Criterion) {
    let rt = Runtime::new().expect("failed to build Tokio runtime");
    let db: Arc<DB> = Arc::new(rt.block_on(DB::new()).expect("failed to init DB"));
    let paths = vec![PathBuf::from("test_data")];

    c.bench_function("register_all_cards", |b| {
        b.to_async(&rt).iter(|| {
            let db = Arc::clone(&db);
            let paths = paths.clone();
            async move {
                let (cards, stats) = register_all_cards(db.as_ref(), paths)
                    .await
                    .expect("failed to register cards");
                black_box(cards);
                black_box(stats);
            }
        });
    });
}

criterion_group!(benches, bench_register_all_cards);
criterion_main!(benches);

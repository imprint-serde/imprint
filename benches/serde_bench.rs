mod data;

use bytes::BytesMut;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use data::{Order, Product};
use imprint::{ImprintRecord, Merge, Project, Read, Write};

pub fn serde_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("serde");
    let product = Product::fake(10).to_imprint().unwrap();
    group.bench_function("serialize", |b| {
        b.iter(|| {
            let mut buf = BytesMut::new();
            product.write(&mut buf).unwrap();
            black_box(buf.freeze());
        })
    });

    let mut buf = BytesMut::new();
    product.write(&mut buf).unwrap();

    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let record = ImprintRecord::read(buf.clone().freeze()).unwrap();
            black_box(record);
        })
    });
}

pub fn ops_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ops");

    let product = Product::fake(10).to_imprint().unwrap();
    let order = Order::fake(10).to_imprint().unwrap();

    let mut product_buf = BytesMut::new();
    product.write(&mut product_buf).unwrap();
    let mut order_buf = BytesMut::new();
    order.write(&mut order_buf).unwrap();

    group.bench_function("merge", |b| {
        b.iter(|| {
            let (product, _) = ImprintRecord::read(product_buf.clone().freeze()).unwrap();
            let (order, _) = ImprintRecord::read(order_buf.clone().freeze()).unwrap();

            let enriched = product.merge(&order).unwrap();
            let mut buf = BytesMut::new();
            enriched.write(&mut buf).unwrap();
            black_box(buf.freeze());
        })
    });

    group.bench_function("project", |b| {
        b.iter(|| {
            let (product, _) = ImprintRecord::read(product_buf.clone().freeze()).unwrap();
            let projected = product.project(&[1, 3, 6]).unwrap();
            let mut buf = BytesMut::new();
            projected.write(&mut buf).unwrap();
            black_box(buf.freeze());
        })
    });
}

criterion_group!(benches, serde_benchmark, ops_benchmark);
criterion_main!(benches);

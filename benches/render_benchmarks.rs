use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rand::RngExt;

use mrc_to_tiff::render::{
    get_quantile_by_pdqselect, get_quantile_by_random_sample, get_quantile_by_select,
    get_quantile_by_sort, render_to_rgb,
};

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::rng();

    // something large
    let mut random_input = vec![0u32; 400 * 400];
    rng.fill(&mut random_input);

    let random_input: Vec<f32> = random_input.into_iter().map(|i| i as f32).collect();

    let mut gq = c.benchmark_group("Quantile");

    gq.bench_with_input("get_quantile_by_sort", &random_input, |b, i| {
        b.iter(|| {
            get_quantile_by_sort(black_box(&i[..]), 0.9999);
        })
    });

    gq.bench_with_input("get_quantile_by_select", &random_input, |b, i| {
        b.iter(|| {
            get_quantile_by_select(black_box(&i[..]), 0.9999);
        })
    });

    gq.bench_with_input("get_quantile_by_pdqselect", &random_input, |b, i| {
        b.iter(|| {
            get_quantile_by_pdqselect(black_box(&i[..]), 0.9999);
        })
    });

    gq.bench_with_input("get_quantile_by_random_sample", &random_input, |b, i| {
        b.iter(|| {
            get_quantile_by_random_sample(black_box(&i[..]), 0.9999);
        })
    });

    gq.finish();

    let mut render = c.benchmark_group("render_to_rgb");

    render.bench_with_input("render_to_rgb", &random_input, |b, i| {
        b.iter(|| {
            render_to_rgb(black_box(i), 400, 400, 0.9999);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

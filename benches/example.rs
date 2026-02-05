#![feature(test)]
extern crate test;

#[cfg(test)]
mod tests {
    use test::Bencher;

    #[bench]
    fn bench_placeholder(b: &mut Bencher) {
        b.iter(|| std::hint::black_box(42));
    }
}

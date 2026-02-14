use cli::solution::Solution;
use libsql::Builder;
use std::sync::LazyLock;

fn main() {
    divan::main();
}

struct Input {
    data: String,
    solve: Box<dyn Fn(&str) -> String + Send + Sync>,
}

static INPUTS: LazyLock<Vec<(u16, u8, u8, Input)>> = LazyLock::new(|| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aoc.db");
    let db = rt.block_on(Builder::new_local(db_path).build()).unwrap();
    let conn = db.connect().unwrap();

    inventory::iter::<&dyn Solution>
        .into_iter()
        .filter_map(|solution| {
            let year = solution.year();
            let day = solution.day();
            let part = solution.part();

            let mut rows = rt.block_on(conn.query(
                "SELECT input FROM solutions WHERE year = ? AND day = ? AND part = ?",
                (year, day, part),
            )).ok()?;

            let row = rt.block_on(rows.next()).ok()??;
            let input: String = row.get(0).ok()?;
            if input.is_empty() { return None; }

            let s: &'static &'static dyn Solution = solution;
            Some((year, day, part, Input {
                data: input,
                solve: Box::new(move |input: &str| s.solve(input)),
            }))
        })
        .collect()
});

fn find_input(year: u16, day: u8, part: u8) -> &'static Input {
    &INPUTS.iter()
        .find(|(y, d, p, _)| *y == year && *d == day && *p == part)
        .unwrap_or_else(|| panic!("no input for {year}-{day:02}-{part}"))
        .3
}

#[divan::bench]
fn y2025_d01_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 1, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d01_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 1, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d02_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 2, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d02_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 2, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d03_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 3, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d03_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 3, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d04_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 4, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d04_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 4, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d05_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 5, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d05_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 5, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d06_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 6, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d06_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 6, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d07_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 7, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d07_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 7, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d08_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 8, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d08_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 8, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d09_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 9, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d09_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 9, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d10_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 10, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d10_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 10, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d11_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 11, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d11_p2(bencher: divan::Bencher) {
    let input = find_input(2025, 11, 2);
    bencher.bench_local(|| (input.solve)(&input.data));
}

#[divan::bench]
fn y2025_d12_p1(bencher: divan::Bencher) {
    let input = find_input(2025, 12, 1);
    bencher.bench_local(|| (input.solve)(&input.data));
}


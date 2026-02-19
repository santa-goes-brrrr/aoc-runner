use std::fs;
use std::path::Path;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

fn main() {
    let solutions_file = Path::new("src/solutions.rs");

    println!("cargo:rerun-if-changed=src/solutions.rs");

    let mut entries: Vec<(u16, u8, u8)> = Vec::new();

    let content = fs::read_to_string(solutions_file).unwrap();
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("#[solution(") {
            if let Some(args_str) = rest.strip_suffix(")]") {
                let args: Vec<&str> = args_str.splitn(3, ',').collect();
                if args.len() == 3 {
                    let year: u16 = args[0].trim().parse().unwrap();
                    let day: u8 = args[1].trim().parse().unwrap();
                    let part: u8 = args[2].trim().parse().unwrap();
                    entries.push((year, day, part));
                }
            }
        }
    }

    entries.sort();

    generate_benches(&entries);
    generate_tests(&entries);
}

fn generate_benches(entries: &[(u16, u8, u8)]) {
    let bench_fns: Vec<TokenStream> = entries
        .iter()
        .map(|(year, day, part)| {
            let fn_name = format_ident!("y{}_d{:02}_p{}", year, day, part);
            let year_lit = Literal::u16_unsuffixed(*year);
            let day_lit = Literal::u8_unsuffixed(*day);
            let part_lit = Literal::u8_unsuffixed(*part);

            quote! {
                #[divan::bench]
                fn #fn_name(bencher: divan::Bencher) {
                    let input = find_input(#year_lit, #day_lit, #part_lit);
                    bencher.bench_local(|| (input.solve)(&input.data));
                }
            }
        })
        .collect();

    let code = quote! {
        use cli::Solution;
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

        #(#bench_fns)*
    };

    write_generated(Path::new("benches/bench.rs"), code);
}

fn generate_tests(entries: &[(u16, u8, u8)]) {
    let test_fns: Vec<TokenStream> = entries
        .iter()
        .map(|(year, day, part)| {
            let fn_name = format_ident!("y{}_d{:02}_p{}", year, day, part);
            let year_lit = Literal::u16_unsuffixed(*year);
            let day_lit = Literal::u8_unsuffixed(*day);
            let part_lit = Literal::u8_unsuffixed(*part);

            quote! {
                #[test]
                fn #fn_name() {
                    let entry = find_entry(#year_lit, #day_lit, #part_lit);
                    if entry.input.is_empty() || entry.expected.is_empty() {
                        return;
                    }
                    let actual = (entry.solve)(&entry.input);
                    assert_eq!(
                        actual.trim(),
                        entry.expected.trim(),
                        "year {} day {} part {}: expected '{}', got '{}'",
                        #year_lit, #day_lit, #part_lit,
                        entry.expected.trim(),
                        actual.trim(),
                    );
                }
            }
        })
        .collect();

    let code = quote! {
        use cli::Solution;
        use libsql::Builder;
        use std::sync::LazyLock;

        struct TestEntry {
            input: String,
            expected: String,
            solve: Box<dyn Fn(&str) -> String + Send + Sync>,
        }

        static ENTRIES: LazyLock<Vec<(u16, u8, u8, TestEntry)>> = LazyLock::new(|| {
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
                        "SELECT input, output FROM solutions WHERE year = ? AND day = ? AND part = ?",
                        (year, day, part),
                    )).ok()?;

                    let row = rt.block_on(rows.next()).ok()??;
                    let input: String = row.get(0).ok()?;
                    let expected: String = row.get(1).ok()?;

                    let s: &'static &'static dyn Solution = solution;
                    Some((year, day, part, TestEntry {
                        input,
                        expected,
                        solve: Box::new(move |input: &str| s.solve(input)),
                    }))
                })
                .collect()
        });

        fn find_entry(year: u16, day: u8, part: u8) -> &'static TestEntry {
            &ENTRIES.iter()
                .find(|(y, d, p, _)| *y == year && *d == day && *p == part)
                .unwrap_or_else(|| panic!("no entry for {year}-{day:02}-{part}"))
                .3
        }

        #(#test_fns)*
    };

    write_generated(Path::new("tests/verify.rs"), code);
}

fn write_generated(path: &Path, code: TokenStream) {
    let syntax_tree = syn::parse2(code).unwrap();
    let formatted = prettyplease::unparse(&syntax_tree);
    fs::write(path, format!("// @generated by build.rs — do not edit\n\n{formatted}")).unwrap();
}

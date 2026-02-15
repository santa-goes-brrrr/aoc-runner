use cli::Solution;
use libsql::Builder;
use libtest_mimic::{Arguments, Trial};

fn main() {
    let args = Arguments::from_args();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let db_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aoc.db");
    let db = rt.block_on(Builder::new_local(db_path).build()).unwrap();
    let conn = db.connect().unwrap();

    let tests: Vec<Trial> = inventory::iter::<&dyn Solution>
        .into_iter()
        .map(|solution| {
            let year = solution.year();
            let day = solution.day();
            let part = solution.part();
            let name = format!("y{year}_d{day:02}_p{part}");

            let rows = rt.block_on(conn.query(
                "SELECT input, output FROM solutions WHERE year = ? AND day = ? AND part = ?",
                (year, day, part),
            ));

            let (input, expected) = match rows {
                Ok(mut rows) => match rt.block_on(rows.next()) {
                    Ok(Some(row)) => {
                        let input: String = row.get(0).unwrap();
                        let output: String = row.get(1).unwrap();
                        (input, output)
                    }
                    _ => (String::new(), String::new()),
                },
                Err(_) => (String::new(), String::new()),
            };

            let ignored = input.is_empty() || expected.is_empty();

            let solve: Box<dyn Fn(&str) -> String + Send + Sync> = {
                let s: &'static &'static dyn Solution = solution;
                Box::new(move |input: &str| s.solve(input))
            };

            Trial::test(name, move || {
                let actual = solve(&input);

                if actual.trim() != expected.trim() {
                    return Err(
                        format!("expected '{}', got '{}'", expected.trim(), actual.trim()).into(),
                    );
                }

                Ok(())
            })
            .with_ignored_flag(ignored)
        })
        .collect();

    libtest_mimic::run(&args, tests).exit();
}

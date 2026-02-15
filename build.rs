use std::fs;
use std::path::Path;

fn main() {
    let solutions_file = Path::new("src/solutions.rs");
    let out_path = Path::new("benches/bench.rs");

    println!("cargo:rerun-if-changed=src/solutions.rs");

    let mut entries: Vec<(u16, u8, u8)> = Vec::new();

    let content = fs::read_to_string(solutions_file).unwrap();
    for line in content.lines() {
        let line = line.trim();
        // Match solution!(YYYY, DD, P, {
        if let Some(rest) = line.strip_prefix("solution!(") {
            let args: Vec<&str> = rest.splitn(4, ',').collect();
            if args.len() >= 3 {
                let year: u16 = args[0].trim().parse().unwrap();
                let day: u8 = args[1].trim().parse().unwrap();
                let part: u8 = args[2].trim().trim_end_matches('{').trim().parse().unwrap();
                entries.push((year, day, part));
            }
        }
    }

    entries.sort();

    let mut code = String::new();
    code.push_str("use cli::Solution;\n");
    code.push_str("use libsql::Builder;\n");
    code.push_str("use std::sync::LazyLock;\n\n");

    code.push_str("fn main() {\n");
    code.push_str("    divan::main();\n");
    code.push_str("}\n\n");

    code.push_str("struct Input {\n");
    code.push_str("    data: String,\n");
    code.push_str("    solve: Box<dyn Fn(&str) -> String + Send + Sync>,\n");
    code.push_str("}\n\n");

    code.push_str("static INPUTS: LazyLock<Vec<(u16, u8, u8, Input)>> = LazyLock::new(|| {\n");
    code.push_str("    let rt = tokio::runtime::Runtime::new().unwrap();\n");
    code.push_str(
        "    let db_path = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"aoc.db\");\n",
    );
    code.push_str(
        "    let db = rt.block_on(Builder::new_local(db_path).build()).unwrap();\n",
    );
    code.push_str("    let conn = db.connect().unwrap();\n\n");
    code.push_str("    inventory::iter::<&dyn Solution>\n");
    code.push_str("        .into_iter()\n");
    code.push_str("        .filter_map(|solution| {\n");
    code.push_str("            let year = solution.year();\n");
    code.push_str("            let day = solution.day();\n");
    code.push_str("            let part = solution.part();\n\n");
    code.push_str("            let mut rows = rt.block_on(conn.query(\n");
    code.push_str("                \"SELECT input FROM solutions WHERE year = ? AND day = ? AND part = ?\",\n");
    code.push_str("                (year, day, part),\n");
    code.push_str("            )).ok()?;\n\n");
    code.push_str("            let row = rt.block_on(rows.next()).ok()??;\n");
    code.push_str("            let input: String = row.get(0).ok()?;\n");
    code.push_str("            if input.is_empty() { return None; }\n\n");
    code.push_str("            let s: &'static &'static dyn Solution = solution;\n");
    code.push_str("            Some((year, day, part, Input {\n");
    code.push_str("                data: input,\n");
    code.push_str("                solve: Box::new(move |input: &str| s.solve(input)),\n");
    code.push_str("            }))\n");
    code.push_str("        })\n");
    code.push_str("        .collect()\n");
    code.push_str("});\n\n");

    code.push_str("fn find_input(year: u16, day: u8, part: u8) -> &'static Input {\n");
    code.push_str("    &INPUTS.iter()\n");
    code.push_str("        .find(|(y, d, p, _)| *y == year && *d == day && *p == part)\n");
    code.push_str("        .unwrap_or_else(|| panic!(\"no input for {year}-{day:02}-{part}\"))\n");
    code.push_str("        .3\n");
    code.push_str("}\n\n");

    for (year, day, part) in &entries {
        code.push_str(&format!(
            "#[divan::bench]\nfn y{year}_d{day:02}_p{part}(bencher: divan::Bencher) {{\n"
        ));
        code.push_str(&format!(
            "    let input = find_input({year}, {day}, {part});\n"
        ));
        code.push_str(
            "    bencher.bench_local(|| (input.solve)(&input.data));\n",
        );
        code.push_str("}\n\n");
    }

    fs::write(out_path, code).unwrap();
}

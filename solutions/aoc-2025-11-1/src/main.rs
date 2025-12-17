use ahash::{HashMap, HashMapExt};
use petgraph::{algo::all_simple_paths, graph::DiGraph};
use std::{hash::RandomState, io::Read};

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

fn solve_p1(s: &str) -> usize {
    let mut graph = DiGraph::<&str, i32>::new();

    let mut names = HashMap::new();

    for line in s.lines() {
        let mut tokens = line.split_whitespace();

        let origin = tokens.next().unwrap().trim_matches(|c| c == ':');

        let origin = *names
            .entry(origin)
            .or_insert_with(|| graph.add_node(origin));

        for target in tokens {
            let target = names
                .entry(target)
                .or_insert_with(|| graph.add_node(target));
            graph.add_edge(origin, *target, 1);
        }
    }

    let you = names.get("you").unwrap();
    let out = names.get("out").unwrap();

    all_simple_paths::<Box<_>, _, RandomState>(&graph, *you, *out, 0, None).count()
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p1() {
        const SAMPLE: &str = "aaa: you hhh
you: bbb ccc
bbb: ddd eee
ccc: ddd eee fff
ddd: ggg
eee: out
fff: out
ggg: out
hhh: ccc fff iii
iii: out
";
        assert_eq!(solve_p1(SAMPLE), 5)
    }
}

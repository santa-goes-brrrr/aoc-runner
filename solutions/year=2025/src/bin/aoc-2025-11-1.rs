#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::boxed::Box;
use hashbrown::{DefaultHashBuilder, HashMap};
use petgraph::{algo::all_simple_paths, graph::DiGraph};

aoc_runtime::solution!(solve);

pub fn solve(s: &str) -> usize {
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

    all_simple_paths::<Box<_>, _, DefaultHashBuilder>(&graph, *you, *out, 0, None).count()
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
        assert_eq!(solve(SAMPLE), 5)
    }
}

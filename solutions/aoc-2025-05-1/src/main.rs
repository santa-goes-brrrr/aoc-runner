use ahash::{HashSet, HashSetExt};
use std::io::Read;

type Range = (u64, u64);

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

fn solve_p1(s: &str) -> usize {
    let (ranges, ids) = s.split_once("\n\n").unwrap();

    let ids: Vec<u64> = ids.lines().map(|s| s.parse().unwrap()).collect();
    let ranges: Vec<Range> = ranges
        .lines()
        .map(|s| s.split_once('-').unwrap())
        .map(|(i, j)| (i.parse().unwrap(), j.parse().unwrap()))
        .collect();

    let mut chosen = HashSet::with_capacity(ids.len());

    for id in ids {
        for range in &ranges {
            if range.0 <= id && id <= range.1 {
                chosen.insert(id);
            }
        }
    }

    chosen.len()
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "3-5
10-14
16-20
12-18

1
5
8
11
17
32
";

    #[test]
    fn sample_p1() {
        assert_eq!(solve_p1(SAMPLE), 3)
    }
}

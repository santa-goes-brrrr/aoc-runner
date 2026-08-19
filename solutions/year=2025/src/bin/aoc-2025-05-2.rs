#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::vec::Vec;

aoc_runtime::solution!(solve);

type Range = (u64, u64);

pub fn solve(s: &str) -> u64 {
    let (ranges, _) = s.split_once("\n\n").unwrap();

    let mut ranges: Vec<Range> = ranges
        .lines()
        .map(|s| s.split_once('-').unwrap())
        .map(|(i, j)| (i.parse().unwrap(), j.parse().unwrap()))
        .collect();

    ranges.sort_by_key(|a| a.0);

    let mut ranges = ranges.into_iter();

    let mut res = 0;

    let mut current = ranges.next().unwrap();

    for range in ranges {
        if range.0 <= current.1 {
            current.1 = range.1.max(current.1)
        } else {
            res += current.1 - current.0 + 1;
            current.0 = range.0;
            current.1 = range.1;
        }
    }

    res += current.1 - current.0 + 1;

    res
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
    fn sample_p2() {
        assert_eq!(solve(SAMPLE), 14)
    }
}

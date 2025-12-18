use std::io::Read;

type Range = (u64, u64);

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p2(&s));
}

pub fn solve_p2(s: &str) -> u64 {
    let (ranges, _) = s.split_once("\n\n").unwrap();

    let mut ranges: Vec<Range> = ranges
        .lines()
        .map(|s| s.split_once('-').unwrap())
        .map(|(i, j)| (i.parse().unwrap(), j.parse().unwrap()))
        .collect();

    ranges.sort_by(|a, b| a.0.cmp(&b.0));

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
        assert_eq!(solve_p2(SAMPLE), 14)
    }
}

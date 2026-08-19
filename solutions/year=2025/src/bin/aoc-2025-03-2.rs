#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

pub fn solve(s: &str) -> u64 {
    let mut result: u64 = 0;

    for line in s.lines() {
        let bytes = line.as_bytes();

        let mut j = 0;
        let mut current = 0;

        for remaining in (0..12).rev() {
            let (i, n) = bytes[j..(line.len() - remaining)].iter().enumerate().fold(
                (0, 0),
                |(i, n), (k, v)| if *v > n { (k, *v) } else { (i, n) },
            );

            j += i + 1;
            current *= 10;
            current += n as u64 - 48;
        }

        result += current;
    }

    result
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p2() {
        let s = "987654321111111
811111111111119
234234234234278
818181911112111";

        assert_eq!(solve(s), 3121910778619)
    }
}

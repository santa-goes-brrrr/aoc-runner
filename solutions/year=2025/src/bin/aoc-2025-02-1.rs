#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use hashbrown::HashMap;

aoc_runtime::solution!(solve);

fn is_invalid(n: u64) -> bool {
    let num_digits = n.ilog10() + 1;

    if !num_digits.is_multiple_of(2) {
        return false;
    }

    let cut = 10_u64.pow(num_digits / 2);

    n % cut == n / cut
}

pub fn solve(input: &str) -> u64 {
    let mut total: u64 = 0;
    let mut cache: HashMap<u64, bool> = HashMap::new();

    let input = input.replace('\n', "");

    for pair in input.split_terminator(',') {
        let (start, end) = pair.split_once('-').unwrap();

        let start: u64 = start.parse().unwrap();
        let end: u64 = end.parse().unwrap();

        for n in start..=end {
            if *cache.entry(n).or_insert_with(|| is_invalid(n)) {
                total += n;
            }
        }
    }

    total
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "11-22,95-115,998-1012,1188511880-1188511890,222220-222224,1698522-1698528,446443-446449,38593856-38593862,565653-565659,824824821-824824827,2121212118-2121212124
";

    #[test]
    fn sample_p1() {
        assert_eq!(solve(SAMPLE), 1227775554)
    }
}

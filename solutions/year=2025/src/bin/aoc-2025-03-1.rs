#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

pub fn solve(s: &str) -> u32 {
    let mut result: u32 = 0;

    for line in s.lines() {
        let mut m = 48;
        let mut j = 48;

        for (i, b) in line.bytes().enumerate() {
            if i == line.len() - 1 {
                continue;
            }
            if b > m {
                m = b;
                j = i;
            }
        }

        let n = line.bytes().skip(j + 1).max().unwrap();
        result += (m as u32 - 48) * 10 + n as u32 - 48;
    }

    result
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p1() {
        let s = "987654321111111
811111111111119
234234234234278
818181911112111";

        assert_eq!(solve(s), 357)
    }
}

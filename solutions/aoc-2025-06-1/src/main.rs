use ahash::{HashMap, HashMapExt};
use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1::<4>(&s));
}

pub fn solve_p1<const N: usize>(s: &str) -> u128 {
    let ops = s
        .lines()
        .last()
        .unwrap()
        .split_whitespace()
        .enumerate()
        .collect::<HashMap<usize, &str>>();

    let mut accs = HashMap::<usize, u128>::with_capacity(ops.len());

    for line in s.lines().take(N) {
        for (i, num) in line.split_whitespace().enumerate() {
            accs.entry(i)
                .and_modify(|v| {
                    let op = ops.get(&i).unwrap();
                    *v = match *op {
                        "*" => *v * num.parse::<u128>().unwrap(),
                        "+" => *v + num.parse::<u128>().unwrap(),
                        _ => unreachable!(),
                    };
                })
                .or_insert(num.parse().unwrap());
        }
    }

    accs.values().sum()
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "123 328  51 64
 45 64  387 23
  6 98  215 314
*   +   *   +
";

    #[test]
    fn sample_p1() {
        assert_eq!(solve_p1::<3>(SAMPLE), 4277556)
    }
}

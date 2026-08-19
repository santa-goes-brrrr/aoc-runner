#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::boxed::Box;
use hashbrown::HashMap;

aoc_runtime::solution!(solve);

pub fn solve(s: &str) -> usize {
    let mut adj = HashMap::new();

    for line in s.lines() {
        let mut tokens = line.split_whitespace();

        let origin = encode(tokens.next().unwrap().trim_matches(|c| c == ':'));
        let targets = tokens.map(encode).collect::<Box<_>>();

        adj.insert(origin, targets);
    }

    let svr = encode("svr");
    let dac = encode("dac");
    let fft = encode("fft");
    let out = encode("out");

    let svr_dac = count_paths(svr, dac, &adj);
    let dac_fft = count_paths(dac, fft, &adj);
    let fft_out = count_paths(fft, out, &adj);

    let svr_fft = count_paths(svr, fft, &adj);
    let fft_dac = count_paths(fft, dac, &adj);
    let dac_out = count_paths(dac, out, &adj);

    svr_fft * fft_dac * dac_out + svr_dac * dac_fft * fft_out
}

fn count_paths(start: u16, target: u16, adjacency: &HashMap<u16, Box<[u16]>>) -> usize {
    count_paths_memoized(start, target, adjacency, &mut HashMap::new())
}

fn count_paths_memoized(
    start: u16,
    target: u16,
    adjacency: &HashMap<u16, Box<[u16]>>,
    memo: &mut HashMap<u16, usize>,
) -> usize {
    if start == target {
        return 1;
    }
    if let Some(&count) = memo.get(&start) {
        return count;
    }

    let count = adjacency
        .get(&start)
        .map(|neighbors| {
            neighbors
                .iter()
                .map(|&neighbor| count_paths_memoized(neighbor, target, adjacency, memo))
                .sum()
        })
        .unwrap_or(0);
    memo.insert(start, count);
    count
}

fn encode(str: &str) -> u16 {
    str.as_bytes().iter().fold(0, |acc, b| {
        acc * (b'z' - b'a' + 1) as u16 + (*b as u16 - b'a' as u16)
    })
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p2() {
        const SAMPLE: &str = "svr: aaa bbb
aaa: fft
fft: ccc
bbb: tty
tty: ccc
ccc: ddd eee
ddd: hub
hub: fff
eee: dac
dac: fff
fff: ggg hhh
ggg: out
hhh: out
";
        assert_eq!(solve(SAMPLE), 2)
    }
}

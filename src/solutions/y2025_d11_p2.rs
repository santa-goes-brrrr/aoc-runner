use ahash::{HashMap, HashMapExt};
use pathfinding::prelude::count_paths;
use solution_macro::Solution;

#[Solution(2025, 11, 2)]
fn solve_p2(s: &str) -> usize {
    let mut adj = HashMap::new();

    for line in s.lines() {
        let mut tokens = line.split_whitespace();

        let origin = encode(tokens.next().unwrap().trim_matches(|c| c == ':'));
        let targets = tokens.map(|s| encode(s)).collect::<Box<_>>();

        adj.insert(origin, targets);
    }

    let svr = encode("svr");
    let dac = encode("dac");
    let fft = encode("fft");
    let out = encode("out");

    let empty = vec![].into_boxed_slice();

    let svr_dac = count_paths(&svr, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == dac);
    let dac_fft = count_paths(&dac, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == fft);
    let fft_out = count_paths(&fft, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == out);

    let svr_fft = count_paths(&svr, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == fft);
    let fft_dac = count_paths(&fft, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == dac);
    let dac_out = count_paths(&dac, |&n| adj.get(n).unwrap_or(&empty), |&n| *n == out);

    svr_fft * fft_dac * dac_out + svr_dac * dac_fft * fft_out
}

fn encode(str: &str) -> u16 {
    str.as_bytes().iter().fold(0, |acc, b| {
        acc * (b'z' - b'a' + 1) as u16 + (*b as u16 - b'a' as u16)
    })
}

#[cfg(test)]
mod test {
    use super::*;

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
        assert_eq!(solve_p2(SAMPLE), 2)
    }
}

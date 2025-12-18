use std::collections::{HashSet, VecDeque};
use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

pub fn solve_p1(s: &str) -> usize {
    let mut res = 0;

    for line in s.lines() {
        let mut sections = line.split_whitespace();

        let end_state = sections
            .next()
            .unwrap()
            .trim_matches(|c| c == '[' || c == ']')
            .chars()
            .map(|c| c == '#')
            .enumerate()
            .fold(0u64, |acc, (i, b)| acc | (b as u64) << i);

        let actions = sections
            .take_while(|&s| s.starts_with('('))
            .map(|section| {
                section
                    .trim_matches(|c| c == '(' || c == ')')
                    .split(',')
                    .map(|s| s.parse::<usize>().unwrap())
                    .fold(0u64, |acc, b| acc | (1u64 << b))
            })
            .collect::<Vec<_>>();

        //                  (state, depth)
        let initial_state = ( 0, 0 );

        let mut visited: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<_> = vec![initial_state].into();

        while let Some(state) = queue.pop_back() {
            if visited.contains(&state.0) {
                continue;
            }

            if state.0 == end_state {
                res += state.1;
                break;
            }

            let steps = state.1 + 1;

            for action in actions.iter() {
                queue.push_front((state.0 ^ *action, steps));
            }

            visited.insert(state.0);
        }
    }

    res
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "[.##.] (3) (1,3) (2) (2,3) (0,2) (0,1) {3,5,4,7}
[...#.] (0,2,3,4) (2,3) (0,4) (0,1,2) (1,2,3,4) {7,5,12,7,2}
[.###.#] (0,1,2,3,4) (0,3,4) (0,1,2,4,5) (1,2) {10,11,11,5,10,5}
";

    #[test]
    fn sample_p1() {
        assert_eq!(solve_p1(SAMPLE), 7)
    }
}

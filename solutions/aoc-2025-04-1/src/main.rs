use ahash::{HashSet, HashSetExt as _};
use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

pub fn solve_p1(s: &str) -> usize {
    let mut grid = HashSet::new();

    for (y, l) in s.lines().enumerate() {
        for (x, _) in l.chars().enumerate().filter(|(_, c)| *c == '@') {
            grid.insert((x + 1, y + 1));
        }
    }

    grid.iter().filter(|p| num_neighbors(p, &grid) < 4).count()
}

fn num_neighbors(p: &(usize, usize), grid: &HashSet<(usize, usize)>) -> usize {
    let mut n = 0;

    n += grid.contains(&(p.0 - 1, p.1 - 1)) as usize;
    n += grid.contains(&(p.0, p.1 - 1)) as usize;
    n += grid.contains(&(p.0 + 1, p.1 - 1)) as usize;
    n += grid.contains(&(p.0 - 1, p.1)) as usize;
    n += grid.contains(&(p.0 + 1, p.1)) as usize;
    n += grid.contains(&(p.0 - 1, p.1 + 1)) as usize;
    n += grid.contains(&(p.0, p.1 + 1)) as usize;
    n += grid.contains(&(p.0 + 1, p.1 + 1)) as usize;

    n
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p1() {
        let s = "..@@.@@@@.
@@@.@.@.@@
@@@@@.@.@@
@.@@@@..@.
@@.@@@@.@@
.@@@@@@@.@
.@.@.@.@@@
@.@@@.@@@@
.@@@@@@@@.
@.@.@@@.@.";

        assert_eq!(solve_p1(s), 13)
    }
}

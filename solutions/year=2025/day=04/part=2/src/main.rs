use ahash::{HashSet, HashSetExt as _};
use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p2(&s));
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

pub fn solve_p2(s: &str) -> usize {
    let mut grid = HashSet::new();

    for (y, l) in s.lines().enumerate() {
        for (x, _) in l.chars().enumerate().filter(|(_, c)| *c == '@') {
            grid.insert((x + 1, y + 1));
        }
    }

    let n = grid.len();

    let mut has_changed = true;

    while has_changed {
        let num_before = grid.len();

        let points: Vec<_> = grid.iter().copied().collect();

        for p in points {
            try_remove(&p, &mut grid);
        }

        has_changed = num_before - grid.len() > 0;
    }

    n - grid.len()
}

fn try_remove(p: &(usize, usize), grid: &mut HashSet<(usize, usize)>) {
    if num_neighbors(p, grid) < 4 {
        grid.remove(p);
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p2() {
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

        assert_eq!(solve_p2(s), 43)
    }
}

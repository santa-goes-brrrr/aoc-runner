#![cfg_attr(target_os = "none", no_std, no_main)]

use hashbrown::HashSet;

aoc_runtime::solution!(solve);

pub fn solve(s: &str) -> usize {
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

        assert_eq!(solve(s), 13)
    }
}

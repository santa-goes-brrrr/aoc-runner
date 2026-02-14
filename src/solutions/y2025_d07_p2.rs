use solution_macro::Solution;

#[Solution(2025, 7, 2)]
fn solve_p2(s: &str) -> u64 {
    let width = s.chars().position(|c| c == '\n').unwrap();
    let height = s.chars().filter(|&c| c == '\n').count() - 1;

    let mut lines = s.lines();

    let start = lines
        .next()
        .unwrap()
        .chars()
        .position(|c| c == 'S')
        .unwrap();

    let mut grid: Vec<Vec<char>> = Vec::with_capacity(height);
    let mut paths: Vec<Vec<u64>> = Vec::with_capacity(height);

    for line in lines {
        let mut row = Vec::with_capacity(width);
        let mut row_paths = Vec::with_capacity(width);

        for c in line.chars() {
            row.push(c);
            row_paths.push(0);
        }

        grid.push(row);
        paths.push(row_paths);
    }

    paths[0][start] = 1;

    for y in 1..height {
        for x in 0..width {
            if grid[y][x] == '^' {
                continue;
            }
            if x > 1 && grid[y][x - 1] == '^' {
                paths[y][x] += paths[y - 1][x - 1];
            }
            if x + 1 < height && grid[y][x + 1] == '^' {
                paths[y][x] += paths[y - 1][x + 1];
            }
            paths[y][x] += paths[y - 1][x];
        }
    }

    paths[height - 1].iter().sum()
}

#[cfg(test)]
mod test {
    use super::*;

    const SAMPLE: &str = ".......S.......
...............
.......^.......
...............
......^.^......
...............
.....^.^.^.....
...............
....^.^...^....
...............
...^.^...^.^...
...............
..^...^.....^..
...............
.^.^.^.^.^...^.
...............
";

    #[test]
    fn sample_p2() {
        assert_eq!(solve_p2(SAMPLE), 40)
    }
}

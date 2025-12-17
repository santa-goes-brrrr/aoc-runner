use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

fn solve_p1(s: &str) -> usize {
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
    let mut seen: Vec<Vec<bool>> = Vec::with_capacity(height);

    for line in lines {
        let mut row = Vec::with_capacity(width);
        let mut row_seen = Vec::with_capacity(width);

        for c in line.chars() {
            row.push(c);
            row_seen.push(false);
        }

        grid.push(row);
        seen.push(row_seen);
    }

    let mut res = 0;

    let mut queue = vec![(0, start)];

    while let Some((y, x)) = queue.pop() {
        if seen[y][x] {
            continue;
        }
        seen[y][x] = true;
        if y + 1 >= height {
            continue;
        }
        match grid[y + 1][x] {
            '.' => queue.push((y + 1, x)),
            '^' => {
                queue.push((y + 1, x - 1));
                queue.push((y + 1, x + 1));
                res += 1;
            }
            _ => unreachable!(),
        }
    }

    res
}

#[cfg(test)]
mod test {
    use crate::*;

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
    fn sample_p1() {
        assert_eq!(solve_p1(SAMPLE), 21)
    }
}

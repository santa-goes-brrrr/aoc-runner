solution!(2025, 01, 1, {
    fn solve(s: &str) -> u32 {
        let mut num_zeros: u32 = 0;
        let mut current: i32 = 50;

        for line in s.lines() {
            let delta = line[1..].parse::<i32>().unwrap();

            match line.chars().next().unwrap() {
                'R' => current += delta,
                'L' => current -= delta,
                _ => unreachable!(),
            }

            current = current.rem_euclid(100);

            num_zeros += (current == 0) as u32;
        }

        num_zeros
    }
});

solution!(2025, 01, 2, {
    fn solve(s: &str) -> i32 {
        let mut num_zeros: i32 = 0;
        let mut current: i32 = 50;

        for line in s.lines() {
            let delta = line[1..].parse::<i32>().unwrap();

            match line.chars().next().unwrap() {
                'R' => {
                    num_zeros += (current + delta).div_euclid(100);
                    current = (current + delta).rem_euclid(100);
                }
                'L' => {
                    num_zeros += (delta + 100 - current).div_euclid(100) - (current == 0) as i32;
                    current = (current - delta).rem_euclid(100);
                }
                _ => unreachable!(),
            };
        }

        num_zeros
    }
});

solution!(2025, 02, 1, {
    use ahash::{HashMap, HashMapExt};

    fn is_invalid(n: u64) -> bool {
        let num_digits = n.ilog10() + 1;

        if !num_digits.is_multiple_of(2) {
            return false;
        }

        let cut = 10_u64.pow(num_digits / 2);

        n % cut == n / cut
    }

    fn solve(input: &str) -> u64 {
        let mut total: u64 = 0;
        let mut cache: HashMap<u64, bool> = HashMap::new();

        let input = input.replace('\n', "");

        for pair in input.split_terminator(',') {
            let (start, end) = pair.split_once('-').unwrap();

            let start: u64 = start.parse().unwrap();
            let end: u64 = end.parse().unwrap();

            for n in start..=end {
                if *cache.entry(n).or_insert_with(|| is_invalid(n)) {
                    total += n;
                }
            }
        }

        total
    }
});

solution!(2025, 02, 2, {
    use ahash::{HashMap, HashMapExt};

    // u64::MAX.ilog10() == 19
    const PRIMES: [u64; 8] = [2, 3, 5, 7, 11, 13, 17, 19];

    fn is_invalid_p2(n: u64) -> bool {
        let num_digits = (n.ilog10() + 1) as u64;

        for s in PRIMES {
            if is_invalid_s(n, s, num_digits) {
                return true;
            }
        }

        false
    }

    fn is_invalid_s(n: u64, s: u64, num_digits: u64) -> bool {
        if !num_digits.is_multiple_of(s) {
            return false;
        }

        let cut = 10_u64.pow((num_digits / s) as u32);

        let lower = n % cut;

        let mut n = n / cut;

        while n != 0 {
            if (n % cut) != lower {
                return false;
            }

            n /= cut;
        }

        true
    }

    fn solve(input: &str) -> u64 {
        let mut total: u64 = 0;
        let mut cache: HashMap<u64, bool> = HashMap::new();

        let input = input.replace('\n', "");

        for pair in input.split_terminator(',') {
            let (start, end) = pair.split_once('-').unwrap();

            let start: u64 = start.parse().unwrap();
            let end: u64 = end.parse().unwrap();

            for n in start..=end {
                if *cache.entry(n).or_insert_with(|| is_invalid_p2(n)) {
                    total += n;
                }
            }
        }

        total
    }
});

solution!(2025, 03, 1, {
    fn solve(s: &str) -> u32 {
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
});

solution!(2025, 03, 2, {
    fn solve(s: &str) -> u64 {
        let mut result: u64 = 0;

        for line in s.lines() {
            let bytes = line.as_bytes();

            let mut j = 0;
            let mut current = 0;

            for remaining in (0..12).rev() {
                let (i, n) = bytes[j..(line.len() - remaining)].iter().enumerate().fold(
                    (0, 0),
                    |(i, n), (k, v)| if *v > n { (k, *v) } else { (i, n) },
                );

                j += i + 1;
                current *= 10;
                current += n as u64 - 48;
            }

            result += current;
        }

        result
    }
});

solution!(2025, 04, 1, {
    use ahash::{HashSet, HashSetExt as _};

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

    fn solve(s: &str) -> usize {
        let mut grid = HashSet::new();

        for (y, l) in s.lines().enumerate() {
            for (x, _) in l.chars().enumerate().filter(|(_, c)| *c == '@') {
                grid.insert((x + 1, y + 1));
            }
        }

        grid.iter().filter(|p| num_neighbors(p, &grid) < 4).count()
    }
});

solution!(2025, 04, 2, {
    use ahash::{HashSet, HashSetExt as _};

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

    fn try_remove(p: &(usize, usize), grid: &mut HashSet<(usize, usize)>) {
        if num_neighbors(p, grid) < 4 {
            grid.remove(p);
        }
    }

    fn solve(s: &str) -> usize {
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
});

solution!(2025, 05, 1, {
    use ahash::{HashSet, HashSetExt};

    type Range = (u64, u64);

    fn solve(s: &str) -> usize {
        let (ranges, ids) = s.split_once("\n\n").unwrap();

        let ids: Vec<u64> = ids.lines().map(|s| s.parse().unwrap()).collect();
        let ranges: Vec<Range> = ranges
            .lines()
            .map(|s| s.split_once('-').unwrap())
            .map(|(i, j)| (i.parse().unwrap(), j.parse().unwrap()))
            .collect();

        let mut chosen = HashSet::with_capacity(ids.len());

        for id in ids {
            for range in &ranges {
                if range.0 <= id && id <= range.1 {
                    chosen.insert(id);
                }
            }
        }

        chosen.len()
    }
});

solution!(2025, 05, 2, {
    type Range = (u64, u64);

    fn solve(s: &str) -> u64 {
        let (ranges, _) = s.split_once("\n\n").unwrap();

        let mut ranges: Vec<Range> = ranges
            .lines()
            .map(|s| s.split_once('-').unwrap())
            .map(|(i, j)| (i.parse().unwrap(), j.parse().unwrap()))
            .collect();

        ranges.sort_by(|a, b| a.0.cmp(&b.0));

        let mut ranges = ranges.into_iter();

        let mut res = 0;

        let mut current = ranges.next().unwrap();

        for range in ranges {
            if range.0 <= current.1 {
                current.1 = range.1.max(current.1)
            } else {
                res += current.1 - current.0 + 1;
                current.0 = range.0;
                current.1 = range.1;
            }
        }

        res += current.1 - current.0 + 1;

        res
    }
});

solution!(2025, 06, 1, {
    use ahash::{HashMap, HashMapExt};

    fn solve_p1<const N: usize>(s: &str) -> u128 {
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

    fn solve(s: &str) -> u128 {
        solve_p1::<4>(s)
    }
});

solution!(2025, 06, 2, {
    use ahash::{HashMap, HashMapExt};

    fn solve_p2<const N: usize>(s: &str) -> u128 {
        let ops = s
            .lines()
            .last()
            .unwrap()
            .split_whitespace()
            .enumerate()
            .collect::<HashMap<usize, &str>>();

        let mut data = HashMap::<usize, [char; N]>::with_capacity(ops.len());

        for (j, line) in s.lines().take(N).enumerate() {
            for (i, char) in line.chars().enumerate() {
                data.entry(i).and_modify(|v| v[j] = char).or_insert({
                    let mut new = [' '; N];
                    new[0] = char;
                    new
                });
            }
        }

        let mut separators = data
            .iter()
            .filter(|(_, v)| v.iter().all(|c| c.is_whitespace()))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();

        separators.sort();

        let mut cols = HashMap::<usize, [Vec<Option<u8>>; N]>::with_capacity(ops.len());

        for (j, line) in s.lines().take(N).enumerate() {
            for (i, char) in line.chars().enumerate() {
                if separators.contains(&&i) {
                    continue;
                }
                let k = separators
                    .iter()
                    .position(|&s| &i <= s)
                    .unwrap_or(ops.len() - 1);
                cols.entry(k)
                    .and_modify(|rows| {
                        rows[j].push(char.to_digit(10).map(|d| d as u8));
                    })
                    .or_insert({
                        let mut empty = [const { Vec::new() }; N];
                        empty[j].push(char.to_digit(10).map(|d| d as u8));
                        empty
                    });
            }
        }

        let mut accs = HashMap::<usize, u128>::with_capacity(ops.len());

        for (i, col) in cols {
            let col_size = col.iter().map(|v| v.len()).max().unwrap_or(0);

            for j in 0..col_size {
                let mut acc = 0;

                for row in &col {
                    if let Some(Some(d)) = row.get(j) {
                        acc *= 10;
                        acc += *d as u32;
                    }
                }

                accs.entry(i)
                    .and_modify(|v| {
                        let op = ops.get(&i).unwrap();
                        *v = match *op {
                            "*" => *v * acc as u128,
                            "+" => *v + acc as u128,
                            _ => unreachable!(),
                        };
                    })
                    .or_insert(acc as u128);
            }
        }

        accs.values().sum()
    }

    fn solve(s: &str) -> u128 {
        solve_p2::<4>(s)
    }
});

solution!(2025, 07, 1, {
    fn solve(s: &str) -> usize {
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
});

solution!(2025, 07, 2, {
    fn solve(s: &str) -> u64 {
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
});

solution!(2025, 08, 1, {
    use ahash::{HashSet, HashSetExt};
    use std::collections::BinaryHeap;

    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Default)]
    struct Coords(i64, i64, i64);

    #[derive(Hash, PartialEq, Eq, Clone)]
    struct Pair(Coords, Coords);

    impl Ord for Pair {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            distance(&other.0, &other.1).cmp(&distance(&self.0, &self.1))
        }
    }

    impl PartialOrd for Pair {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    fn distance(rhs: &Coords, lhs: &Coords) -> i64 {
        (rhs.0 - lhs.0).pow(2) + (rhs.1 - lhs.1).pow(2) + (rhs.2 - lhs.2).pow(2)
    }

    fn solve(s: &str) -> usize {
        let mut points: Vec<Coords> = vec![];

        for line in s.lines() {
            let mut numbers = line.split(",");

            let x = numbers.next().unwrap().parse().unwrap();
            let y = numbers.next().unwrap().parse().unwrap();
            let z = numbers.next().unwrap().parse().unwrap();

            points.push(Coords(x, y, z));
        }

        let mut distances = BinaryHeap::<Pair>::with_capacity(points.len() * points.len());

        for i in 0..points.len() {
            for j in 0..i {
                distances.push(Pair(points[i], points[j]));
            }
        }

        let mut edges = HashSet::with_capacity(points.len() * points.len());

        for _ in 0..1_000 {
            let p = distances.pop().unwrap();
            edges.insert(p);
        }

        let mut sizes: Vec<usize> = vec![];

        let mut remaining = points.clone();

        while let Some(point) = remaining.pop() {
            let mut visited: HashSet<Coords> = HashSet::new();
            let mut queue: Vec<Coords> = vec![point];

            while let Some(point) = queue.pop() {
                if !visited.contains(&point) {
                    let e = edges.iter().filter(|p| p.0 == point || p.1 == point);

                    for p in e {
                        queue.push(p.0);
                        queue.push(p.1);
                    }
                }

                visited.insert(point);
            }

            sizes.push(visited.len());

            remaining.retain(|c| !visited.contains(c))
        }

        sizes.sort();

        sizes.iter().rev().take(3).product()
    }
});

solution!(2025, 08, 2, {
    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    fn union(parent: &mut Vec<usize>, rank: &mut Vec<u8>, a: usize, b: usize) -> bool {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra == rb {
            return false;
        }
        if rank[ra] < rank[rb] {
            parent[ra] = rb;
        } else if rank[ra] > rank[rb] {
            parent[rb] = ra;
        } else {
            parent[rb] = ra;
            rank[ra] += 1;
        }
        true
    }

    fn solve(s: &str) -> usize {
        let points: Vec<(i64, i64, i64)> = s
            .lines()
            .map(|line| {
                let mut nums = line.split(',');
                let x = nums.next().unwrap().parse().unwrap();
                let y = nums.next().unwrap().parse().unwrap();
                let z = nums.next().unwrap().parse().unwrap();
                (x, y, z)
            })
            .collect();

        let n = points.len();
        let mut edges: Vec<(i64, usize, usize)> = Vec::with_capacity(n * (n - 1) / 2);

        for i in 0..n {
            for j in 0..i {
                let d = (points[i].0 - points[j].0).pow(2)
                    + (points[i].1 - points[j].1).pow(2)
                    + (points[i].2 - points[j].2).pow(2);
                edges.push((d, i, j));
            }
        }

        edges.sort_unstable();

        let mut parent: Vec<usize> = (0..n).collect();
        let mut rank = vec![0u8; n];
        let mut components = n;

        for &(_, i, j) in &edges {
            if union(&mut parent, &mut rank, i, j) {
                components -= 1;
                if components == 1 {
                    return (points[i].0 * points[j].0) as usize;
                }
            }
        }

        unreachable!();
    }
});

solution!(2025, 09, 1, {
    fn compute_area(a: &(usize, usize), b: &(usize, usize)) -> usize {
        let p = (a.0.min(b.0), a.1.min(b.1));
        let q = (a.0.max(b.0), a.1.max(b.1));

        (q.0 - p.0 + 1) * (q.1 - p.1 + 1)
    }

    fn solve(s: &str) -> usize {
        let mut points = vec![];

        for line in s.lines() {
            let coords = line.split_once(',').unwrap();
            let x: usize = coords.0.parse().unwrap();
            let y: usize = coords.1.parse().unwrap();

            points.push((x, y));
        }

        let mut res = 0;

        for i in 0..points.len() {
            for j in 0..i {
                let a = compute_area(&points[i], &points[j]);
                if a > res {
                    res = a;
                }
            }
        }

        res
    }
});

solution!(2025, 09, 2, {
    fn normalize(p: &(u64, u64), q: &(u64, u64)) -> ((u64, u64), (u64, u64)) {
        ((p.0.min(q.0), p.1.min(q.1)), (p.0.max(q.0), p.1.max(q.1)))
    }

    fn solve(s: &str) -> u64 {
        let mut points = vec![];

        for line in s.lines() {
            let coords = line.split_once(',').unwrap();

            let x: u64 = coords.0.parse().unwrap();
            let y: u64 = coords.1.parse().unwrap();

            points.push((x, y));
        }

        let mut rectangles = Vec::with_capacity(points.len() * points.len());

        for i in 0..points.len() {
            for j in 0..i {
                let (a, b) = normalize(&points[i], &points[j]);
                let area = (b.0 - a.0 + 1) * (b.1 - a.1 + 1);

                rectangles.push((a, b, area));
            }
        }

        rectangles.sort_by_key(|(_, _, a)| *a);

        let n = points.len();
        points.push(*points.first().unwrap());

        for (a, b, area) in rectangles.iter().rev() {
            let mut intersects = false;

            for i in 0..n {
                let (p, q) = normalize(&points[i], &points[i + 1]);

                if !(b.0 <= p.0 || a.0 >= q.0 || b.1 <= p.1 || a.1 >= p.1) {
                    intersects = true;
                    break;
                }
            }

            if !intersects {
                return *area;
            }
        }

        unreachable!()
    }
});

solution!(2025, 10, 1, {
    use std::collections::{HashSet, VecDeque};

    fn solve(s: &str) -> usize {
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
});

solution!(2025, 10, 2, {
    use z3::{
        Optimize,
        ast::{Ast, Int},
    };

    fn solve(s: &str) -> u64 {
        let mut res = 0;

        for line in s.lines() {
            let optimize = Optimize::new();

            let targets = line
                .split_whitespace()
                .last()
                .unwrap()
                .trim_matches(|c| c == '{' || c == '}')
                .split(',')
                .map(|s| s.parse::<u64>().unwrap())
                .map(|n| Int::from_u64(n))
                .collect::<Box<[Int]>>();

            let coeficients = line
                .split_whitespace()
                .skip(1)
                .take_while(|&s| s.starts_with('('))
                .map(|section| {
                    section
                        .trim_matches(|c| c == '(' || c == ')')
                        .split(',')
                        .map(|s| s.parse::<usize>().unwrap())
                        .collect::<Box<_>>()
                })
                .collect::<Vec<Box<[usize]>>>();

            let coeficients = coeficients
                .into_iter()
                .map(|indexes| {
                    let mut expanded = vec![Int::from_u64(0u64); targets.len()].into_boxed_slice();
                    for ind in indexes {
                        expanded[ind] = Int::from_u64(1);
                    }
                    expanded
                })
                .collect::<Vec<_>>();

            let variables = (0..coeficients.len())
                .map(|i| Int::new_const(format!("n_{i}")))
                .collect::<Vec<_>>();

            for var in &variables {
                optimize.assert(&var.ge(Int::from_u64(0)));
            }

            let equations = coeficients
                .into_iter()
                .zip(&variables)
                .map(|(coef, var)| coef.into_iter().map(|c| c * var).collect::<Vec<_>>())
                .fold(vec![Int::from_u64(0); targets.len()], |acc, term| {
                    acc.into_iter()
                        .zip(term)
                        .map(|(acc, term)| acc + term)
                        .collect()
                });

            let equations = equations
                .into_iter()
                .zip(targets)
                .map(|(equations, target)| equations.eq(&target))
                .collect::<Vec<_>>();

            for equation in &equations {
                optimize.assert(equation);
            }

            let sum_vars = variables
                .into_iter()
                .fold(Int::from_u64(0), |acc, var| acc + var)
                .simplify();

            optimize.minimize(&sum_vars);

            assert!(matches!(optimize.check(&[]), z3::SatResult::Sat));

            let model = optimize.get_model().unwrap();

            let sum_vars = model.eval(&sum_vars, true).unwrap();

            res += sum_vars.as_u64().unwrap();
        }

        res
    }
});

solution!(2025, 11, 1, {
    use ahash::{HashMap, HashMapExt};
    use petgraph::{algo::all_simple_paths, graph::DiGraph};
    use std::hash::RandomState;

    fn solve(s: &str) -> usize {
        let mut graph = DiGraph::<&str, i32>::new();

        let mut names = HashMap::new();

        for line in s.lines() {
            let mut tokens = line.split_whitespace();

            let origin = tokens.next().unwrap().trim_matches(|c| c == ':');

            let origin = *names
                .entry(origin)
                .or_insert_with(|| graph.add_node(origin));

            for target in tokens {
                let target = names
                    .entry(target)
                    .or_insert_with(|| graph.add_node(target));
                graph.add_edge(origin, *target, 1);
            }
        }

        let you = names.get("you").unwrap();
        let out = names.get("out").unwrap();

        all_simple_paths::<Box<_>, _, RandomState>(&graph, *you, *out, 0, None).count()
    }
});

solution!(2025, 11, 2, {
    use ahash::{HashMap, HashMapExt};
    use pathfinding::prelude::count_paths;

    fn encode(str: &str) -> u16 {
        str.as_bytes().iter().fold(0, |acc, b| {
            acc * (b'z' - b'a' + 1) as u16 + (*b as u16 - b'a' as u16)
        })
    }

    fn solve(s: &str) -> usize {
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
});

solution!(2025, 12, 1, {
    type Shape = [[bool; 3]; 3];

    fn solve(s: &str) -> usize {
        let mut solvable = 0;

        let mut blocks = s.split("\n\n");

        let mut shapes: Box<[Shape]> = Box::new([[[false; 3]; 3]; 6]);
        let mut areas: Box<[u8]> = Box::new([0; 6]);

        for i in 0..=5 {
            let shape = blocks.next().unwrap().lines().skip(1);
            for (y, line) in shape.enumerate() {
                for (x, char) in line.chars().enumerate() {
                    if char == '#' {
                        shapes[i][y][x] = true;
                        areas[i] += 1;
                    }
                }
            }
        }

        for grid in blocks.next().unwrap().lines() {
            let mut tokens = grid.split_whitespace();

            let (width, height) = tokens
                .next()
                .unwrap()
                .trim_matches(|c| c == ':')
                .split_once('x')
                .unwrap();

            let width = width.parse::<u8>().unwrap();
            let height = height.parse::<u8>().unwrap();

            let grid_size = width as usize * height as usize;

            let shapes_count: Vec<u8> = tokens.map(|s| s.parse::<u8>().unwrap()).collect();

            let shapes_area: usize = shapes_count
                .iter()
                .enumerate()
                .map(|(i, &count)| count as usize * areas[i] as usize)
                .sum();

            if grid_size >= shapes_area {
                solvable += 1;
            }
        }

        solvable
    }
});

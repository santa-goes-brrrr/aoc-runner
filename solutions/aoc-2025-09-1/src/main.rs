use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

fn solve_p1(s: &str) -> usize {
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

fn compute_area(a: &(usize, usize), b: &(usize, usize)) -> usize {
    let p = (a.0.min(b.0), a.1.min(b.1));
    let q = (a.0.max(b.0), a.1.max(b.1));

    (q.0 - p.0 + 1) * (q.1 - p.1 + 1)
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "7,1
11,1
11,7
9,7
9,5
2,5
2,3
7,3
";

    #[test]
    fn sample_p1() {
        assert_eq!(solve_p1(SAMPLE), 50)
    }
}

use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p2(&s));
}

pub fn solve_p2(s: &str) -> u64 {
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

    // rectangles.sort_by_key(|(_, _, a)| *a);

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

fn normalize(p: &(u64, u64), q: &(u64, u64)) -> ((u64, u64), (u64, u64)) {
    ((p.0.min(q.0), p.1.min(q.1)), (p.0.max(q.0), p.1.max(q.1)))
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
    fn sample_p2() {
        assert_eq!(solve_p2(SAMPLE), 24)
    }
}

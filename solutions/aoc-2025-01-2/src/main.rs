use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p2(&s));
}

fn solve_p2(s: &str) -> i32 {
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

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p2() {
        let s = "L68
L30
R48
L5
R60
L55
L1
L99
R14
L82";

        assert_eq!(solve_p2(s), 6)
    }
}

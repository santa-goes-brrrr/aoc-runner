use std::io::Read;

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p1(&s));
}

pub fn solve_p1(s: &str) -> u32 {
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

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn sample_p1() {
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

        assert_eq!(solve_p1(s), 3)
    }
}

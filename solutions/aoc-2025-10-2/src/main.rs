use std::io::Read;
use z3::{
    Optimize,
    ast::{Ast, Int},
};

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();

    println!("{}", solve_p2(&s));
}

fn solve_p2(s: &str) -> u64 {
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

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "[.##.] (3) (1,3) (2) (2,3) (0,2) (0,1) {3,5,4,7}
[...#.] (0,2,3,4) (2,3) (0,4) (0,1,2) (1,2,3,4) {7,5,12,7,2}
[.###.#] (0,1,2,3,4) (0,3,4) (0,1,2,4,5) (1,2) {10,11,11,5,10,5}
";

    #[test]
    fn sample_p2() {
        assert_eq!(solve_p2(SAMPLE), 33)
    }
}

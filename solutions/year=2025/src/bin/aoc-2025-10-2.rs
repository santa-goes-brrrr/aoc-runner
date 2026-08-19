#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::{vec, vec::Vec};

aoc_runtime::solution!(solve);

#[derive(Clone, Copy, Debug)]
struct Fraction {
    numerator: i128,
    denominator: i128,
}

impl Fraction {
    const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    fn new(numerator: i128, denominator: i128) -> Self {
        assert_ne!(denominator, 0);
        if numerator == 0 {
            return Self::ZERO;
        }

        let sign = if denominator < 0 { -1 } else { 1 };
        let divisor = gcd(numerator.abs(), denominator.abs());
        Self {
            numerator: sign * numerator / divisor,
            denominator: denominator.abs() / divisor,
        }
    }

    fn is_zero(self) -> bool {
        self.numerator == 0
    }

    fn subtract(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.denominator - other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }

    fn multiply(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.numerator,
            self.denominator * other.denominator,
        )
    }

    fn divide(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.denominator,
            self.denominator * other.numerator,
        )
    }
}

#[derive(Debug)]
struct PivotFormula {
    denominator: i128,
    constant: i128,
    coefficients: Vec<i128>,
}

pub fn solve(input: &str) -> u64 {
    input.lines().map(minimum_presses).sum()
}

fn minimum_presses(line: &str) -> u64 {
    let target_section = line
        .split_whitespace()
        .last()
        .unwrap()
        .trim_matches(|character| character == '{' || character == '}');
    let targets = target_section
        .split(',')
        .map(|number| number.parse::<i128>().unwrap())
        .collect::<Vec<_>>();

    let buttons = line
        .split_whitespace()
        .skip(1)
        .take_while(|section| section.starts_with('('))
        .map(|section| {
            section
                .trim_matches(|character| character == '(' || character == ')')
                .split(',')
                .map(|index| index.parse::<usize>().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let row_count = targets.len();
    let column_count = buttons.len();
    let coefficients = (0..row_count)
        .map(|row| {
            buttons
                .iter()
                .map(|button| i128::from(button.contains(&row)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut matrix = coefficients
        .iter()
        .zip(&targets)
        .map(|(row, &target)| {
            row.iter()
                .copied()
                .chain([target])
                .map(|value| Fraction::new(value, 1))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut pivot_columns = Vec::new();
    let mut pivot_row = 0;

    for column in 0..column_count {
        let Some(candidate) = (pivot_row..row_count).find(|&row| !matrix[row][column].is_zero())
        else {
            continue;
        };

        matrix.swap(pivot_row, candidate);
        let pivot = matrix[pivot_row][column];
        for value in &mut matrix[pivot_row][column..=column_count] {
            *value = value.divide(pivot);
        }

        let normalized_pivot = matrix[pivot_row].clone();
        for (row, values) in matrix.iter_mut().enumerate() {
            if row == pivot_row || values[column].is_zero() {
                continue;
            }
            let factor = values[column];
            for index in column..=column_count {
                values[index] = values[index].subtract(factor.multiply(normalized_pivot[index]));
            }
        }

        pivot_columns.push(column);
        pivot_row += 1;
        if pivot_row == row_count {
            break;
        }
    }

    for row in &matrix[pivot_row..] {
        assert!(
            row[..column_count].iter().any(|value| !value.is_zero()) || row[column_count].is_zero()
        );
    }

    let free_columns = (0..column_count)
        .filter(|column| !pivot_columns.contains(column))
        .collect::<Vec<_>>();

    let formulas = pivot_columns
        .iter()
        .enumerate()
        .map(|(row, _)| {
            let denominator = free_columns
                .iter()
                .map(|&column| matrix[row][column].denominator)
                .chain([matrix[row][column_count].denominator])
                .fold(1, lcm);
            let constant = matrix[row][column_count].numerator
                * (denominator / matrix[row][column_count].denominator);
            let coefficients = free_columns
                .iter()
                .map(|&column| {
                    matrix[row][column].numerator * (denominator / matrix[row][column].denominator)
                })
                .collect();
            PivotFormula {
                denominator,
                constant,
                coefficients,
            }
        })
        .collect::<Vec<_>>();

    let bounds = free_columns
        .iter()
        .map(|&column| {
            coefficients
                .iter()
                .zip(&targets)
                .filter_map(|(row, &target)| (row[column] != 0).then_some(target))
                .min()
                .unwrap()
        })
        .collect::<Vec<_>>();

    let mut values = vec![0; free_columns.len()];
    let mut best = i128::MAX;
    search_free_variables(0, 0, &bounds, &mut values, &formulas, &mut best);
    assert_ne!(best, i128::MAX);
    best as u64
}

fn search_free_variables(
    index: usize,
    free_sum: i128,
    bounds: &[i128],
    values: &mut [i128],
    formulas: &[PivotFormula],
    best: &mut i128,
) {
    if free_sum >= *best {
        return;
    }

    if index < values.len() {
        for value in 0..=bounds[index] {
            values[index] = value;
            search_free_variables(index + 1, free_sum + value, bounds, values, formulas, best);
        }
        return;
    }

    let mut total = free_sum;
    for formula in formulas {
        let numerator = formula
            .coefficients
            .iter()
            .zip(values.iter())
            .fold(formula.constant, |value, (coefficient, free)| {
                value - coefficient * free
            });
        if numerator < 0 || numerator % formula.denominator != 0 {
            return;
        }
        total += numerator / formula.denominator;
        if total >= *best {
            return;
        }
    }

    *best = total;
}

fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn lcm(left: i128, right: i128) -> i128 {
    left / gcd(left, right) * right
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
        assert_eq!(solve(SAMPLE), 33)
    }
}

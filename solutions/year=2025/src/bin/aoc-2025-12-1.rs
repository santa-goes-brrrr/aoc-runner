#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

aoc_runtime::solution!(solve);

type Shape = [[bool; 3]; 3];

pub fn solve(s: &str) -> usize {
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

        let width = width.parse::<usize>().unwrap();
        let height = height.parse::<usize>().unwrap();
        let grid_size = width * height;
        let shape_counts = tokens
            .map(|token| token.parse::<u8>().unwrap())
            .collect::<Box<_>>();
        let shape_total = shape_counts
            .iter()
            .map(|&count| count as usize)
            .sum::<usize>();

        if grid_size >= shape_total * 9 {
            solvable += 1;
            continue;
        }

        let occupied_area = shape_counts
            .iter()
            .zip(areas.iter())
            .map(|(&count, &area)| count as usize * area as usize)
            .sum::<usize>();
        if occupied_area <= grid_size && can_pack(width, height, &shape_counts, &shapes) {
            solvable += 1;
        }
    }

    solvable
}

fn can_pack(width: usize, height: usize, counts: &[u8], shapes: &[Shape]) -> bool {
    if width < 3 || height < 3 {
        return counts.iter().all(|&count| count == 0);
    }

    let orientations = shapes.iter().map(shape_orientations).collect::<Vec<_>>();
    let mut pieces = counts
        .iter()
        .enumerate()
        .flat_map(|(shape, &count)| core::iter::repeat_n(shape, count as usize))
        .collect::<Vec<_>>();
    pieces.sort_by_key(|&shape| core::cmp::Reverse(orientations[shape].len()));

    place_piece(
        0,
        &pieces,
        &orientations,
        width,
        height,
        &mut vec![false; width * height],
    )
}

fn place_piece(
    index: usize,
    pieces: &[usize],
    orientations: &[Vec<Shape>],
    width: usize,
    height: usize,
    occupied: &mut [bool],
) -> bool {
    if index == pieces.len() {
        return true;
    }

    for shape in &orientations[pieces[index]] {
        for y in 0..=height - 3 {
            for x in 0..=width - 3 {
                let fits = (0..3).all(|dy| {
                    (0..3).all(|dx| !shape[dy][dx] || !occupied[(y + dy) * width + x + dx])
                });
                if !fits {
                    continue;
                }

                for dy in 0..3 {
                    for dx in 0..3 {
                        if shape[dy][dx] {
                            occupied[(y + dy) * width + x + dx] = true;
                        }
                    }
                }
                if place_piece(index + 1, pieces, orientations, width, height, occupied) {
                    return true;
                }
                for dy in 0..3 {
                    for dx in 0..3 {
                        if shape[dy][dx] {
                            occupied[(y + dy) * width + x + dx] = false;
                        }
                    }
                }
            }
        }
    }

    false
}

fn shape_orientations(shape: &Shape) -> Vec<Shape> {
    let mut orientations = Vec::new();
    for flipped in [false, true] {
        let mut current = if flipped { flip(shape) } else { *shape };
        for _ in 0..4 {
            if !orientations.contains(&current) {
                orientations.push(current);
            }
            current = rotate(&current);
        }
    }
    orientations
}

fn rotate(shape: &Shape) -> Shape {
    let mut rotated = [[false; 3]; 3];
    for (y, row) in shape.iter().enumerate() {
        for (x, &value) in row.iter().enumerate() {
            rotated[x][2 - y] = value;
        }
    }
    rotated
}

fn flip(shape: &Shape) -> Shape {
    let mut flipped = [[false; 3]; 3];
    for (y, row) in shape.iter().enumerate() {
        for (x, &value) in row.iter().enumerate() {
            flipped[y][2 - x] = value;
        }
    }
    flipped
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "0:
###
##.
##.

1:
###
##.
.##

2:
.##
###
##.

3:
##.
###
##.

4:
###
#..
###

5:
###
.#.
###

4x4: 0 0 0 0 2 0
12x5: 1 0 1 0 2 2
12x5: 1 0 1 0 3 2";

    #[test]
    fn sample_p1() {
        assert_eq!(solve(SAMPLE), 2)
    }
}

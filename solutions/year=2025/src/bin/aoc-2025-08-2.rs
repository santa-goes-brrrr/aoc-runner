#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::{collections::BinaryHeap, vec, vec::Vec};

aoc_runtime::solution!(solve);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
struct Coords(i64, i64, i64);

#[derive(PartialEq, Eq, Clone)]
struct Pair {
    distance: i64,
    left: usize,
    right: usize,
}

impl Ord for Pair {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.distance.cmp(&self.distance)
    }
}

impl PartialOrd for Pair {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn distance(rhs: &Coords, lhs: &Coords) -> i64 {
    (rhs.0 - lhs.0).pow(2) + (rhs.1 - lhs.1).pow(2) + (rhs.2 - lhs.2).pow(2)
}

pub fn solve(s: &str) -> usize {
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
            distances.push(Pair {
                distance: distance(&points[i], &points[j]),
                left: i,
                right: j,
            });
        }
    }

    let mut parents = (0..points.len()).collect::<Vec<_>>();
    let mut sizes = vec![1_usize; points.len()];
    let mut components = points.len();

    while let Some(pair) = distances.pop() {
        if union(pair.left, pair.right, &mut parents, &mut sizes) {
            components -= 1;
            if components == 1 {
                return (points[pair.left].0 * points[pair.right].0) as usize;
            }
        }
    }

    unreachable!();
}

fn find(mut node: usize, parents: &mut [usize]) -> usize {
    while parents[node] != node {
        parents[node] = parents[parents[node]];
        node = parents[node];
    }
    node
}

fn union(left: usize, right: usize, parents: &mut [usize], sizes: &mut [usize]) -> bool {
    let mut left_root = find(left, parents);
    let mut right_root = find(right, parents);
    if left_root == right_root {
        return false;
    }
    if sizes[left_root] < sizes[right_root] {
        core::mem::swap(&mut left_root, &mut right_root);
    }
    parents[right_root] = left_root;
    sizes[left_root] += sizes[right_root];
    true
}

#[cfg(test)]
mod test {
    use crate::*;

    const SAMPLE: &str = "162,817,812
57,618,57
906,360,560
592,479,940
352,342,300
466,668,158
542,29,236
431,825,988
739,650,466
52,470,668
216,146,977
819,987,18
117,168,530
805,96,715
346,949,466
970,615,88
941,993,340
862,61,35
984,92,344
425,690,689";

    #[test]
    fn sample_p2() {
        assert_eq!(solve(SAMPLE), 25272)
    }
}

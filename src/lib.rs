pub use aoc_macros::solution;

pub trait Solution: Send + Sync {
    fn year(&self) -> u16;
    fn day(&self) -> u8;
    fn part(&self) -> u8;
    fn solve(&self, input: &str) -> String;
}

inventory::collect!(&'static dyn Solution);

mod solutions;

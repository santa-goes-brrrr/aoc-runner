pub trait Solution: Send + Sync {
    fn year(&self) -> u16;
    fn day(&self) -> u8;
    fn part(&self) -> u8;
    fn solve(&self, input: &str) -> String;
}

inventory::collect!(&'static dyn Solution);

#[macro_export]
macro_rules! solution {
    ($year:literal, $day:literal, $part:literal, { $($body:item)* }) => {
        paste::paste! {
            mod [<y $year _d $day _p $part>] {
                pub struct S;

                impl $crate::Solution for S {
                    fn year(&self) -> u16 { $year }
                    fn day(&self) -> u8 { $day }
                    fn part(&self) -> u8 { $part }
                    fn solve(&self, input: &str) -> String {
                        solve(input).to_string()
                    }
                }

                inventory::submit!(&S as &dyn $crate::Solution);

                $($body)*
            }
        }
    };
}

mod solutions;

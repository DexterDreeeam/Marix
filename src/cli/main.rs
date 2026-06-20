mod input;
mod interface;
mod output;

pub use input::UserInput;
pub use interface::{CliInterface, Interface};
pub use output::Output;

fn main() {
    let input = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if !input.is_empty() {
        println!("{input}");
    }
}

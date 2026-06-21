use std::io;

use marix_common::PipeError;

fn main() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        PipeError::Unavailable("PipeServer implementation is not linked".to_owned()),
    ))
}

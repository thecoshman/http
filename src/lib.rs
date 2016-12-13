#[macro_use]
extern crate clap;
extern crate iron;

mod error;
mod options;

pub mod util;

pub use error::Error;
pub use options::Options;

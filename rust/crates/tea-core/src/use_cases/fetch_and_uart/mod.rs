pub mod domain;
pub mod logic;

pub use domain::{Cmd, Model, Msg, UpdateResult};
pub use logic::{init, update};

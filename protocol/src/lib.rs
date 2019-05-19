mod barrier;
mod lines;

pub use barrier::{Barrier, BarrierReadyFuture};
pub use lines::Lines;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Request {
    Subscribe(String),
    Send { channel: String, message: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Reply {
    Subscribed(String),
    Error(String),
    Message { channel: String, message: String },
}

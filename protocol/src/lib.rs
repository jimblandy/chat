#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

mod lines;
pub use lines::Lines;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Request {
    Subscribe(String),
    Send { channel: String, message: String }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Reply {
    Subscribed(String),
    Error(String),
    Message { channel: String, message: String }
}

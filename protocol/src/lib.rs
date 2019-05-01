#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

mod lines;
pub use lines::Lines;

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    Create(String),
    Subscribe(String),
    Send { channel: String, message: String }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Reply {
    Created(String),
    Subscribed(String),
    Error(String),
    Message { channel: String, message: String }
}

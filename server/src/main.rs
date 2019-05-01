#![feature(async_await, await_macro)]
#![allow(unused_imports, dead_code)]

use futures::executor::{self, ThreadPool};
use futures::prelude::*;
use futures::lock::Mutex as FutureMutex;
use futures::task::SpawnExt;
use protocol::{Lines, Reply, Request};
use romio::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;

/// A connection to a client, from which we can read `Request` objects.
struct Incoming(Box<dyn AsyncRead>);

/// A connection to a client, to which we can write serialized `Reply` objects.
#[derive(Clone, Debug)]
struct Outgoing(Arc<FutureMutex<Box<dyn AsyncWrite>>>);

struct Channel {
    subscribers: Vec<Outgoing>
}

struct ChannelMap {
    channels: HashMap<String, Channel>,
}

fn main() -> io::Result<()> {
    executor::block_on(async {
        let mut threadpool = ThreadPool::new()?;

        let addr = "0.0.0.0:9999".parse().unwrap();
        let mut listener = TcpListener::bind(&addr)?;
        let mut incoming = listener.incoming();

        println!("Listening on {:?}", addr);

        while let Some(stream) = await!(incoming.next()) {
            let stream = stream?;
            let peer_addr = stream.peer_addr()?;
            threadpool.spawn(async move {
                if let Err(e) = await!(handle_client(stream)) {
                    eprintln!("Connection with {} closed for error: {}", peer_addr, e);
                }
            }).expect("error spawning task");
        }

        Ok(())
    })
}

async fn handle_client(stream: TcpStream) -> io::Result<()> {
    let peer_addr = stream.peer_addr().expect("getting socket peer address");
    println!("Accepted connection from: {}", peer_addr);

    let mut lines = Lines::new(stream);
    while let Some(line) = await!(lines.next()) {
        let line = line?;
        println!("Received line: {:?}", line);
    }

    println!("Closing connection from: {}", peer_addr);

    Ok(())
}

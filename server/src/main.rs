#![feature(async_await, await_macro)]
#![allow(unused_imports, dead_code)]

use futures::executor::{self, ThreadPool};
use futures::io::{AsyncWrite, AsyncWriteExt};
use futures::prelude::*;
use futures::lock::Mutex as FutureMutex;
use futures::task::SpawnExt;
use protocol::{Lines, Reply, Request};
use romio::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use std::marker::Unpin;
use std::sync::Arc;

/// A connection to a client, from which we can read `Request` objects.
struct Incoming(Box<dyn AsyncRead>);

/// A connection to a client, to which we can write serialized `Reply` objects.
#[derive(Clone, Debug)]
struct Outbound(Arc<FutureMutex<Box<dyn 'static + AsyncWrite + Send + Unpin>>>);

#[derive(Debug, Default)]
struct Channel {
    subscribers: Vec<Outbound>
}

#[derive(Debug, Default)]
struct ChannelMap {
    channels: HashMap<String, Channel>,
}

fn main() -> io::Result<()> {
    executor::block_on(async {
        let channels = Arc::new(FutureMutex::new(ChannelMap::default()));

        let mut threadpool = ThreadPool::new()?;

        let addr = "0.0.0.0:9999".parse().unwrap();
        let mut listener = TcpListener::bind(&addr)?;
        let mut incoming = listener.incoming();

        println!("Listening on {:?}", addr);

        while let Some(stream) = await!(incoming.next()) {
            let stream = stream?;
            let peer_addr = stream.peer_addr()?;
            let my_channels = channels.clone();
            threadpool.spawn(async move {
                if let Err(e) = await!(handle_client(stream, my_channels)) {
                    eprintln!("Connection with {} closed for error: {}", peer_addr, e);
                }
            }).expect("error spawning task");
        }

        Ok(())
    })
}

async fn handle_client(stream: TcpStream, channel_map: Arc<FutureMutex<ChannelMap>>) -> io::Result<()> {
    let peer_addr = stream.peer_addr().expect("getting socket peer address");
    println!("Accepted connection from: {}", peer_addr);

    let (inbound, outbound) = stream.split();
    let outbound = Outbound(Arc::new(FutureMutex::new(Box::new(outbound))));

    let mut lines = Lines::new(inbound);
    while let Some(line) = await!(lines.next()) {
        match serde_json::de::from_reader(line?.as_bytes())? {
            Request::Subscribe(name) => {
                let mut map = await!(channel_map.lock());
                let channel = map.channels.entry(name.clone()).or_insert(Channel::default());
                channel.subscribers.push(outbound.clone());
                await!(send_reply(&outbound, Reply::Subscribed(name)))?;
            }
            Request::Send { channel: name, message } => {
                let maybe_subscribers = {
                    let map = await!(channel_map.lock());
                    map.channels
                        .get(&name)
                        .map(|channel| channel.subscribers.clone())
                };

                if let Some(subscribers) = maybe_subscribers {
                    let reply = Reply::Message {
                        channel: name,
                        message
                    };

                    for subscriber in &subscribers {
                        await!(send_reply(subscriber, reply.clone()))?;
                    }
                } else {
                    await!(send_reply(&outbound,
                                      Reply::Error(format!("no such channel: {}", name))))?;
                }
            }
        }
    }

    println!("Closing connection from: {}", peer_addr);

    Ok(())
}

async fn send_reply<'a>(outbound: &'a Outbound, reply: Reply) -> io::Result<()> {
    let mut encoded = serde_json::ser::to_string(&reply)?;
    encoded.push('\n');

    let mut lock = await!(outbound.0.lock());
    await!(lock.write_all(encoded.as_bytes()))?;
    await!(lock.flush())?;
    Ok(())
}

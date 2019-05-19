#![feature(async_await, await_macro)]

use futures::executor::LocalPool;
use futures::io::{AsyncWrite, AsyncWriteExt};
use futures::lock::Mutex;
use futures::prelude::*;
use futures::stream::FuturesUnordered;
use futures::task::SpawnExt;
use protocol::{Reply, Request};
use romio::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use std::iter::FromIterator;
use std::marker::Unpin;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A connection to a client, to which we can write serialized `Reply` objects.
#[derive(Clone)]
struct Outbound(Arc<Mutex<Box<dyn 'static + AsyncWrite + Send + Unpin>>>);

#[derive(Default)]
struct Channel {
    subscribers: Vec<Outbound>,
}

#[derive(Default)]
struct ChannelMap {
    channels: HashMap<String, Channel>,
}

static REQUESTS_SERVED: AtomicUsize = AtomicUsize::new(0);

fn main() -> io::Result<()> {
    let mut threadpool = LocalPool::new();
    let mut spawner = threadpool.spawner();

    threadpool.run_until(async {
        let channels = Arc::new(Mutex::new(ChannelMap::default()));

        let addr = SocketAddr::from_str("0.0.0.0:9999").unwrap();
        let mut listener = TcpListener::bind(&addr)?;
        let mut incoming = listener.incoming();

        println!("Listening on {:?}", addr);

        while let Some(stream) = incoming.next().await {
            let stream = stream?;
            let peer_addr = stream.peer_addr()?;
            let my_channels = channels.clone();
            spawner
                .spawn(async move {
                    match handle_client(stream, my_channels).await {
                        Ok(()) => println!("Closing connection from: {}", peer_addr),
                        Err(e) => {
                            eprintln!("Connection with {} closed for error: {}", peer_addr, e);
                        }
                    }
                })
                .expect("error spawning task");
        }

        Ok(())
    })
}

async fn handle_client(stream: TcpStream, channel_map: Arc<Mutex<ChannelMap>>) -> io::Result<()> {
    let peer_addr = stream.peer_addr().expect("getting socket peer address");
    println!("Accepted connection from: {}", peer_addr);

    let (inbound, outbound) = stream.split();
    let outbound = Outbound(Arc::new(Mutex::new(Box::new(outbound))));

    let mut lines = protocol::Lines::new(inbound);
    while let Some(line) = lines.next().await {
        if REQUESTS_SERVED.fetch_add(1, Ordering::SeqCst) % 1000 == 0 {
            eprint!(".");
        }
        let line = line?;
        match serde_json::de::from_reader(line.as_bytes()) {
            Err(e) => {
                eprintln!("serde didn't like: {:?}", line);
                return Err(e.into());
            }
            Ok(Request::Subscribe(name)) => {
                let mut map = channel_map.lock().await;
                let channel = map
                    .channels
                    .entry(name.clone())
                    .or_insert(Channel::default());
                channel.subscribers.push(outbound.clone());
                send_reply(&outbound, Reply::Subscribed(name)).await?;
            }
            Ok(Request::Send {
                channel: name,
                message,
            }) => {
                let maybe_subscribers = {
                    let map = channel_map.lock().await;
                    map.channels
                        .get(&name)
                        .map(|channel| channel.subscribers.clone())
                };

                if let Some(subscribers) = maybe_subscribers {
                    let reply = Reply::Message {
                        channel: name.clone(),
                        message,
                    };

                    // Send to each subscriber, in order.
                    // for subscriber in &subscribers {
                    //     send_reply(subscriber, reply.clone()).await?;
                    // }

                    // Do all the sends in parallel.
                    let sends = subscribers.into_iter().map(|subscriber| {
                        async {
                            match send_reply(&subscriber, reply.clone()).await {
                                Ok(()) => None,
                                Err(_e) => {
                                    //eprintln!("Error sending (dropping subscriber): {}", e);
                                    Some(subscriber)
                                }
                            }
                        }
                    });

                    let failed = FuturesUnordered::from_iter(sends)
                        .filter_map(|maybe_failed| async { maybe_failed })
                        .collect::<Vec<_>>()
                        .await;

                    if !failed.is_empty() {
                        let mut map = channel_map.lock().await;
                        if let Some(channel) = map.channels.get_mut(&name) {
                            channel
                                .subscribers
                                .retain(|out| !failed.iter().any(|f| Arc::ptr_eq(&f.0, &out.0)));
                        }
                    }
                } else {
                    send_reply(
                        &outbound,
                        Reply::Error(format!("no such channel: {}", name)),
                    )
                    .await?;
                }
            }
        }
    }

    Ok(())
}

async fn send_reply<'a>(outbound: &'a Outbound, reply: Reply) -> io::Result<()> {
    let mut encoded = serde_json::ser::to_string(&reply)?;
    encoded.push('\n');

    let mut lock = outbound.0.lock().await;
    lock.write_all(encoded.as_bytes()).await?;
    lock.flush().await?;
    Ok(())
}

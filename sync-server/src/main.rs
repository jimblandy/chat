use protocol::{Reply, Request};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// A connection to a client, to which we can write serialized `Reply` objects.
#[derive(Clone)]
struct Outbound(Arc<Mutex<Box<dyn 'static + Write + Send>>>);

#[derive(Default)]
struct Channel {
    subscribers: Vec<Outbound>,
}

#[derive(Default)]
struct ChannelMap {
    channels: HashMap<String, Channel>,
}

fn main() -> io::Result<()> {
    {
        let channels = Arc::new(Mutex::new(ChannelMap::default()));

        let addr = "0.0.0.0:9999";
        let listener = TcpListener::bind(addr)?;
        let mut incoming = listener.incoming();

        println!("Listening on {:?}", addr);

        while let Some(stream) = incoming.next() {
            let stream = stream?;
            let peer_addr = stream.peer_addr()?;
            let my_channels = channels.clone();
            std::thread::spawn(move || match handle_client(stream, my_channels) {
                Ok(()) => println!("Closing connection from: {}", peer_addr),
                Err(e) => {
                    eprintln!("Connection with {} closed for error: {}", peer_addr, e);
                }
            });
        }

        Ok(())
    }
}

fn handle_client(stream: TcpStream, channel_map: Arc<Mutex<ChannelMap>>) -> io::Result<()> {
    let peer_addr = stream.peer_addr().expect("getting socket peer address");
    println!("Accepted connection from: {}", peer_addr);

    let (inbound, outbound) = (stream.try_clone()?, stream);
    let outbound = Outbound(Arc::new(Mutex::new(Box::new(outbound))));

    let mut lines = BufReader::new(inbound).lines();
    while let Some(line) = lines.next() {
        match serde_json::de::from_reader(line?.as_bytes())? {
            Request::Subscribe(name) => {
                let mut map = channel_map.lock().unwrap();
                let channel = map
                    .channels
                    .entry(name.clone())
                    .or_insert(Channel::default());
                channel.subscribers.push(outbound.clone());
                send_reply(&outbound, Reply::Subscribed(name))?;
            }
            Request::Send {
                channel: name,
                message,
            } => {
                let maybe_subscribers = {
                    let map = channel_map.lock().unwrap();
                    map.channels
                        .get(&name)
                        .map(|channel| channel.subscribers.clone())
                };

                if let Some(subscribers) = maybe_subscribers {
                    let reply = Reply::Message {
                        channel: name,
                        message,
                    };

                    for subscriber in &subscribers {
                        send_reply(subscriber, reply.clone())?;
                    }
                } else {
                    send_reply(
                        &outbound,
                        Reply::Error(format!("no such channel: {}", name)),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn send_reply<'a>(outbound: &'a Outbound, reply: Reply) -> io::Result<()> {
    let mut encoded = serde_json::ser::to_string(&reply)?;
    encoded.push('\n');

    let mut lock = outbound.0.lock().unwrap();
    lock.write_all(encoded.as_bytes())?;
    lock.flush()?;
    Ok(())
}

#![feature(async_await, await_macro, error_iter)]

use futures::executor::ThreadPool;
use futures::future::{join, ready};
use futures::io::BufReader;
use futures::prelude::*;
use futures::stream::{FuturesUnordered, StreamExt};
use protocol::{Barrier, Request};
use romio::TcpStream;
use std::borrow::Cow;
use std::error::Error;
use std::io;
use std::marker::Unpin;
use std::net::SocketAddr;
use std::str::FromStr;

fn main() {
    if let Err(err) = run() {
        /*
        for e in err.iter_chain() {
            eprintln!("{}", e);
        }
        */
        let mut err = Some(&err as &dyn Error);
        while let Some(e) = err {
            eprintln!("{}", e);
            err = e.source();
        }

        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.len() != 2 {
        return Err(other_error("usage: massive-client CONNECTIONS MESSAGES"));
    }
    let num_clients = usize::from_str(&args[0]).or_else(|_| {
        Err(other_error(format!(
            "couldn't parse number of connections: {:?}",
            args[0]
        )))
    })?;
    let num_messages = usize::from_str(&args[1]).or_else(|_| {
        Err(other_error(format!(
            "couldn't parse number of messages: {:?}",
            args[1]
        )))
    })?;

    let addr = SocketAddr::from_str("0.0.0.0:9999").unwrap();
    let barrier = Barrier::new(num_clients);

    let all_clients = (0..num_clients)
        .map(|_client_ix| {
            let barrier = barrier.wait();
            async move {
                let stream = TcpStream::connect(&addr).await?;
                let (inbound, mut outbound) = stream.split();
                let mut inbound_lines = BufReader::new(inbound).lines();

                let subscribe_request = {
                    let mut request =
                        serde_json::ser::to_string(&Request::Subscribe("rust".to_string()))?;
                    request.push('\n');
                    request
                };
                send_line(&mut outbound, &subscribe_request).await?;

                let _line = inbound_lines.next().await.unwrap_or(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "unexpected end of stream",
                )))?;
                //println!("{}", line);
                eprint!("S");

                barrier.await;

                eprint!("M");
                let results = join(
                    async {
                        //let mut msg = String::new();
                        for _msg_ix in 0..num_messages {
                            //msg.clear();
                            //write!(&mut msg, "Message {} from client {}", msg_ix, client_ix).unwrap();
                            send_message(&mut outbound, "rust", "hi").await?;
                        }
                        Ok(())
                    },
                    async {
                        for _msg_ix in 0..num_messages * num_clients {
                            let _line =
                                inbound_lines.next().await.unwrap_or(Err(io::Error::new(
                                    io::ErrorKind::BrokenPipe,
                                    "unexpected end of stream",
                                )))?;
                        }
                        Ok(())
                    },
                )
                .await;

                results.0.and(results.1)
            }
        })
        .collect::<FuturesUnordered<_>>();

    let mut thread_pool = ThreadPool::new().expect("failed to create threadpool");

    // Run until someone returns an error, or everyone succeeds.
    let result = thread_pool
        .run(all_clients.filter(|r| ready(r.is_err())).next())
        .unwrap_or(Ok(()));

    eprintln!();

    result
}

async fn send_message<'a, Out>(
    outbound: &'a mut Out,
    channel: &'a str,
    message: &'a str,
) -> io::Result<()>
where
    Out: AsyncWriteExt + Unpin + 'a,
{
    let mut request = serde_json::ser::to_string(&Request::Send {
        channel: channel.to_string(),
        message: message.to_string(),
    })?;
    request.push('\n');

    send_line(outbound, &request).await
}

async fn send_line<'a, Out: AsyncWriteExt + Unpin + 'a>(
    outbound: &'a mut Out,
    line: &'a str,
) -> io::Result<()> {
    outbound.write_all(line.as_bytes()).await?;
    outbound.flush().await?;
    Ok(())
}

fn other_error<M: Into<Cow<'static, str>>>(msg: M) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg.into())
}

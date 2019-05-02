use protocol::{Reply, Request};
use std::borrow::Cow;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::net::TcpStream;

fn main() -> io::Result<()> {
    let addr = "127.0.0.1:9999";
    let stream = TcpStream::connect(addr)?;
    let inbound = stream.try_clone()?;
    let outbound = stream;

    let sending_thread = std::thread::spawn(move || {
        if let Err(e) = handle_sending(outbound) {
            eprintln!("Error sending: {}", e);
        }
    });

    let receiving_thread = std::thread::spawn(move || {
        if let Err(e) = handle_receiving(inbound) {
            eprintln!("Error receiving: {}", e);
        }
    });

    sending_thread.join().unwrap();
    receiving_thread.join().unwrap();

    Ok(())
}

fn handle_sending(outbound: TcpStream) -> io::Result<()> {
    let mut outbound = BufWriter::new(outbound);

    let stdin = std::io::stdin();
    for result in stdin.lock().lines() {
        match parse_input(result?) {
            Ok(request) => send_request(&mut outbound, &request)?,
            Err(error) => eprintln!("error: {}", error),
        }
    }

    outbound.into_inner().unwrap().shutdown(std::net::Shutdown::Both)?;

    Ok(())
}

fn parse_input(command: String) -> Result<Request, Cow<'static, str>> {
    let words = command.split_whitespace().collect::<Vec<_>>();
    match words.first() {
        None | Some(&"help") => {
            return Err("Commands:\n\
                        subscribe CHANNEL\n\
                        send CHANNEL MESSAGE...".into());
        }

        Some(&"subscribe") => {
            if words.len() != 2 {
                return Err("Usage: subscribe CHANNEL".into());
            }

            Ok(Request::Subscribe(words[1].to_string()))
        }

        Some(&"send") => {
            if words.len() < 3 {
                return Err("Usage: send CHANNEL MESSAGE...".into());
            }

            Ok(Request::Send {
                channel: words[1].to_string(),
                message: words[2..].join(" "),
            })
        }
        Some(other) => {
            return Err(format!("Unrecognized command: {}", other).into());
        }
    }
}

fn send_request(outbound: &mut BufWriter<TcpStream>, request: &Request) -> io::Result<()> {
    serde_json::ser::to_writer(&mut *outbound, &request)?;
    writeln!(outbound)?;
    outbound.flush()?;
    Ok(())
}

fn handle_receiving(inbound: TcpStream) -> io::Result<()> {
    let inbound = BufReader::new(inbound);
    for line in inbound.lines() {
        let reply: Reply = serde_json::de::from_reader(line?.as_bytes())?;
        println!("Reply: {:?}", reply);
    }
    Ok(())
}

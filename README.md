# A toy chat server, in both sync and async styles

This workspace contains a client and two servers for a trivial chat protocol:
clients can subscribe to channels, and send messages on channels. The two
servers are equivalent, but:

- One (*sync-server*) is written in traditional synchronous style, using
  blocking I/O calls and starting a separate thread for each connection.

- The other (*async-server*) is written using Rust's experimental async/await
  syntax, and future-based asychronous I/O primitives. It serves all clients on
  a `ThreadPool` executor, but it could also work almost entirely unchanged on a
  single thread.

What's interesting is to diff the two *src/main.rs* files from *async-server*
and *sync-server*. I've tried to keep the differences to a minimum; if you can
refine the diff to highlight the sections of each line that have changed, it
should be even clearer what's going on.

This uses the *romio* crate's asynchronous sockets, and the futures-preview 0.3
line (in alpha as of this writing).

Example use: In one window, run the server:

    $ cd async-server/
    $ cargo run
        Finished dev [unoptimized + debuginfo] target(s) in 0.07s
         Running `/home/jimb/rust/chat/target/debug/async-server`
    Listening on V4(0.0.0.0:9999)

Then, in another window on the same machine, run the client, and type in a few
commands:

    $ cd client
    $ cargo run
       Compiling client v0.1.0 (/home/jimb/rust/chat/client)
        Finished dev [unoptimized + debuginfo] target(s) in 0.70s
         Running `/home/jimb/rust/chat/target/debug/client`
    subscribe rust
    Reply: Subscribed("rust")
    send rust This is a chat message! "yo"
    Reply: Message { channel: "rust", message: "This is a chat message! \"yo\"" }

Other clients in other windows should be able to connect as well, and receive
whatever messages are sent to channels they're subscribed to.

Known bugs:

- The server doesn't properly clean up the subscribers lists when a client
  closes the connection.

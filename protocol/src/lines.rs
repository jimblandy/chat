use futures::io::AsyncRead;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use std::collections::VecDeque;
use std::io;
use std::marker::Unpin;
use std::mem::replace;
use std::pin::Pin;

pub struct Lines<R> {
    stream: R,
    buf: VecDeque<u8>,
}

impl<R> Lines<R> {
    pub fn new(stream: R) -> Lines<R> {
        Lines {
            stream,
            buf: VecDeque::new(),
        }
    }
}

impl<R> Lines<R> {
    fn poll_buffered(&mut self) -> Poll<io::Result<String>> {
        if let Some(end) = self.buf.iter().position(|&b| b == b'\n') {
            //if let Some(end) = self.find_newline() {
            let tail = self.buf.split_off(end + 1);
            let line_bytes = Vec::from(replace(&mut self.buf, tail));
            let line_result = String::from_utf8(line_bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
            return Poll::Ready(line_result);
        }

        return Poll::Pending;
    }

    fn drain_to_end(&mut self) -> io::Result<String> {
        let line_bytes = Vec::from(replace(&mut self.buf, VecDeque::new()));
        String::from_utf8(line_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    #[allow(dead_code)]
    fn find_newline(&self) -> Option<usize> {
        let (first, second) = self.buf.as_slices();
        first.iter().position(|&b| b == b'\n').or_else(|| {
            second
                .iter()
                .position(|&b| b == b'\n')
                .map(|pos| first.len() + pos)
        })
    }
}

impl<R: AsyncRead + Unpin> Stream for Lines<R> {
    type Item = io::Result<String>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<io::Result<String>>> {
        loop {
            if let Poll::Ready(result) = self.poll_buffered() {
                return Poll::Ready(Some(result));
            }

            let mut buf = [0u8; 8192];
            match AsyncRead::poll_read(Pin::new(&mut self.stream), cx, &mut buf) {
                Poll::Pending => {
                    return Poll::Pending;
                }
                Poll::Ready(Ok(length)) => {
                    if length == 0 {
                        if self.buf.is_empty() {
                            return Poll::Ready(None);
                        }
                        return Poll::Ready(Some(self.drain_to_end()));
                    }

                    self.buf.extend(&buf[0..length]);
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Some(Err(e.into())));
                }
            }
        }
    }
}

use futures::future::Future;
use futures::task::{Context, Poll, Waker};
use std::mem::replace;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub struct Barrier(Arc<Mutex<Inner>>);
struct Inner {
    total: usize,
    polled: usize,
    wakers: Vec<Option<Waker>>,
}

pub struct BarrierReadyFuture {
    inner: Arc<Mutex<Inner>>,
    index: usize,
}

impl Barrier {
    pub fn new(total: usize) -> Barrier {
        Barrier(Arc::new(Mutex::new(Inner {
            total,
            polled: 0,
            wakers: Vec::new(),
        })))
    }

    pub fn wait(&self) -> BarrierReadyFuture {
        let inner = self.0.clone();
        let index;

        {
            let mut guard = inner.lock().unwrap();
            index = guard.wakers.len();
            assert!(index < guard.total); // we don't support multiple wait cycles yet
            guard.wakers.push(None);
        }

        BarrierReadyFuture { inner, index }
    }
}

impl Future for BarrierReadyFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut guard = self.inner.lock().unwrap();

        // Once we wake everybody, we drop the vector, but they're all going to
        // come back and poll again. (Well, all of them except the last, since that
        // task got a Poll::Ready immediately.)
        if guard.polled == guard.total {
            return Poll::Ready(());
        }

        if let None = replace(&mut guard.wakers[self.index], Some(cx.waker().clone())) {
            guard.polled += 1;
            if guard.polled == guard.total {
                for waker in replace(&mut guard.wakers, Vec::new()) {
                    waker.unwrap().wake();
                }
            }
        }

        if guard.polled == guard.total {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

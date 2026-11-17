//! A small unbounded channel whose receiver can be awaited by a task on GPUI's foreground executor.
//!
//! The application sends core events from Tokio to GPUI over Tokio's own channel (design 5.10, decision 0008). The
//! cookbook has no Tokio and adds no dependency, so this std-only channel stands in for it. What carries over is the
//! receiving side: a foreground task awaits the first item, drains whatever else has arrived with `try_recv`, and applies
//! the batch in one view update, so many items arriving between two frames cost one re-render.
//!
//! Two limits to know before copying it:
//!
//! - It is unbounded: `send` never waits and the queue grows as long as the producer outpaces the consumer. A
//!   high-rate producer, such as a terminal's output, needs backpressure, which the real application must add.
//! - It is single-consumer: it keeps one waker, so only one task may await the receiver. A second task awaiting it
//!   replaces the first one's waker, and the first is then never woken.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

struct State<T> {
    queue: VecDeque<T>,
    waker: Option<Waker>,
    senders: usize,
    receiver_alive: bool,
}

type Shared<T> = Arc<Mutex<State<T>>>;

fn lock<T>(shared: &Shared<T>) -> MutexGuard<'_, State<T>> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The sending half; clone it for more producers. Sending never blocks.
pub struct Sender<T> {
    shared: Shared<T>,
}

/// The receiving half.
pub struct Receiver<T> {
    shared: Shared<T>,
}

/// Creates a channel with no capacity limit.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Mutex::new(State {
        queue: VecDeque::new(),
        waker: None,
        senders: 1,
        receiver_alive: true,
    }));
    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}

impl<T> Sender<T> {
    /// Queues `value`. Returns it back once the receiver is gone, which tells a producer to stop.
    pub fn send(&self, value: T) -> Result<(), T> {
        let waker = {
            let mut state = lock(&self.shared);
            if !state.receiver_alive {
                return Err(value);
            }
            state.queue.push_back(value);
            state.waker.take()
        };
        // Woken outside the lock: waking may run the receiver's executor on this thread.
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        lock(&self.shared).senders += 1;
        Sender {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let waker = {
            let mut state = lock(&self.shared);
            state.senders -= 1;
            if state.senders == 0 {
                state.waker.take()
            } else {
                None
            }
        };
        // The last sender wakes a waiting receiver so it can see the end.
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Receiver<T> {
    /// The next queued item, if any, without waiting.
    pub fn try_recv(&self) -> Option<T> {
        lock(&self.shared).queue.pop_front()
    }

    /// Waits for the next item. Resolves to `None` once every sender is gone and the queue is empty.
    pub fn recv(&self) -> Recv<'_, T> {
        Recv { receiver: self }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Queued items are dropped after the lock is released, in case dropping one touches this channel.
        let _queued = {
            let mut state = lock(&self.shared);
            state.receiver_alive = false;
            std::mem::take(&mut state.queue)
        };
    }
}

/// The future [`Receiver::recv`] returns.
pub struct Recv<'a, T> {
    receiver: &'a Receiver<T>,
}

impl<T> Future for Recv<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut state = lock(&self.receiver.shared);
        if let Some(value) = state.queue.pop_front() {
            Poll::Ready(Some(value))
        } else if state.senders == 0 {
            Poll::Ready(None)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Wake;
    use std::thread;

    /// Counts how often it was woken.
    struct Count(AtomicUsize);

    impl Wake for Count {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn counting_waker() -> (Arc<Count>, Waker) {
        let count = Arc::new(Count(AtomicUsize::new(0)));
        (count.clone(), Waker::from(count))
    }

    /// Wakes the thread that waits in `block_on`.
    struct Unpark(thread::Thread);

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let waker = Waker::from(Arc::new(Unpark(thread::current())));
        let mut cx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return value;
            }
            thread::park();
        }
    }

    #[test]
    fn a_waiting_receiver_is_woken_by_a_send() {
        let (tx, rx) = unbounded::<u32>();
        let (count, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);
        let mut recv = pin!(rx.recv());
        assert_eq!(recv.as_mut().poll(&mut cx), Poll::Pending);
        tx.send(7).unwrap();
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert_eq!(recv.as_mut().poll(&mut cx), Poll::Ready(Some(7)));
    }

    #[test]
    fn try_recv_drains_in_order() {
        let (tx, rx) = unbounded();
        for n in 0..5 {
            tx.send(n).unwrap();
        }
        let drained: Vec<i32> = std::iter::from_fn(|| rx.try_recv()).collect();
        assert_eq!(drained, vec![0, 1, 2, 3, 4]);
        assert_eq!(rx.try_recv(), None);
    }

    #[test]
    fn the_receiver_ends_after_the_last_sender_and_the_queue() {
        let (tx, rx) = unbounded();
        let tx2 = tx.clone();
        tx.send("a").unwrap();
        drop(tx);
        tx2.send("b").unwrap();
        let (count, waker) = counting_waker();
        let mut cx = Context::from_waker(&waker);
        {
            let mut recv = pin!(rx.recv());
            assert_eq!(recv.as_mut().poll(&mut cx), Poll::Ready(Some("a")));
        }
        assert_eq!(rx.try_recv(), Some("b"));
        let mut recv = pin!(rx.recv());
        assert_eq!(recv.as_mut().poll(&mut cx), Poll::Pending);
        // Dropping the last sender wakes the waiting receiver so it can end.
        drop(tx2);
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert_eq!(recv.as_mut().poll(&mut cx), Poll::Ready(None));
    }

    #[test]
    fn send_fails_once_the_receiver_is_gone() {
        let (tx, rx) = unbounded();
        drop(rx);
        assert_eq!(tx.send(1), Err(1));
    }

    #[test]
    fn items_from_another_thread_arrive_in_order() {
        let (tx, rx) = unbounded();
        let producer = thread::spawn(move || {
            for n in 0..10_000u32 {
                tx.send(n).unwrap();
            }
        });
        let received = block_on(async {
            let mut all = Vec::new();
            while let Some(first) = rx.recv().await {
                all.push(first);
                while let Some(more) = rx.try_recv() {
                    all.push(more);
                }
            }
            all
        });
        producer.join().unwrap();
        assert_eq!(received, (0..10_000).collect::<Vec<_>>());
    }
}

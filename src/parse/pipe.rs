use std::cell::*;
use std::future::*;
use std::pin::*;
use std::rc::Rc;
use std::task::*;

struct Pipe<T> {
    value: Option<T>,
    wake_sender: Option<Waker>,
    wake_receiver: Option<Waker>,
}

pub struct Sending<T> {
    pipe: Rc<RefCell<Pipe<T>>>,
}

impl<T> Future for Sending<T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut pipe = self.pipe.borrow_mut();
        match pipe.value {
            None => {
                pipe.wake_sender = None;
                Poll::Ready(())
            }
            Some(_) => {
                pipe.wake_sender = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

fn send<T>(pipe: Rc<RefCell<Pipe<T>>>, value: T) -> Sending<T> {
    {
        let mut pipe = pipe.borrow_mut();
        assert!(pipe.value.is_none());
        pipe.value = Some(value);
        if let Some(waker) = pipe.wake_receiver.take() {
            waker.wake();
        }
    }
    Sending { pipe }
}

pub struct Receiving<T> {
    pipe: Rc<RefCell<Pipe<T>>>,
}

impl<T> Future for Receiving<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut pipe = self.pipe.borrow_mut();
        match pipe.value.take() {
            None => {
                pipe.wake_receiver = Some(cx.waker().clone());
                Poll::Pending
            }
            Some(v) => {
                pipe.wake_receiver = None;
                if let Some(waker) = pipe.wake_sender.take() {
                    waker.wake();
                }
                Poll::Ready(v)
            }
        }
    }
}

fn receive<T>(pipe: Rc<RefCell<Pipe<T>>>) -> Receiving<T> {
    Receiving { pipe }
}

pub fn create_pipe<T>() -> (impl FnMut(T) -> Sending<T>, impl FnMut() -> Receiving<T>) {
    let pipe = Rc::new(RefCell::new(Pipe::<T> {
        value: None,
        wake_sender: None,
        wake_receiver: None,
    }));
    let sender = {
        let pipe = pipe.clone();
        move |v| send(pipe.clone(), v)
    };
    let receiver = move || receive(pipe.clone());
    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures::future::join;

    async fn produce<Sender: FnMut(i32) -> SenderFuture, SenderFuture: Future<Output = ()>>(
        mut send: Sender,
    ) {
        send(1).await;
        send(2).await;
        let mut n = 2;
        while n < 20 {
            send(n).await;
            n *= 2;
        }
        send(-1).await;
    }

    async fn consume<
        Receiver: FnMut() -> ReceivingFuture,
        ReceivingFuture: Future<Output = i32>,
    >(
        mut receive: Receiver,
    ) -> Vec<i32> {
        let mut v = vec![];
        let mut s = 0;
        loop {
            let n = receive().await;
            if n == -1 {
                break v;
            }
            s += n;
            v.push(s);
        }
    }

    #[test]
    fn pipe() {
        let (sender, receiver) = create_pipe::<i32>();
        let task = join(produce(sender), consume(receiver));
        assert_eq!(block_on(task), ((), vec![1, 3, 5, 9, 17, 33]))
    }
}

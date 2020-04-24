use std::cell::*;
use std::future::Future;
use std::pin::*;
use std::rc::*;
use std::task::*;

struct Pipe<T> {
    value: Option<T>,
}

pub struct Sender<T> {
    pipe: Rc<RefCell<Pipe<T>>>,
}

pub struct SenderFuture<T> {
    pipe: Rc<RefCell<Pipe<T>>>,
}

impl<T> Future for SenderFuture<T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pipe.borrow().value.is_some() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl<T> Sender<T> {
    pub async fn send(&self, value: T) {
        assert!(std::mem::replace(&mut self.pipe.borrow_mut().value, Some(value)).is_none());
        SenderFuture {
            pipe: self.pipe.clone(),
        }
        .await
    }
}

pub struct Receiver<DriverFuture, T> {
    driver_future: Pin<Box<DriverFuture>>,
    pipe: Rc<RefCell<Pipe<T>>>,
    waker: Waker,
}

impl<DriverFuture: Future<Output = ()>, T> Iterator for Receiver<DriverFuture, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        match self
            .driver_future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
        {
            Poll::Ready(()) => None,
            Poll::Pending => self.pipe.borrow_mut().value.take(),
        }
    }
}

fn waker_clone(_: *const ()) -> RawWaker {
    RAW_WAKER
}
fn waker_wake(_: *const ()) {}
fn waker_wake_by_ref(_: *const ()) {}
fn drop(_: *const ()) {}

const WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, drop);

const RAW_WAKER: RawWaker = RawWaker::new(&(), &WAKER_VTABLE);

pub fn generator<T, Driver, DriverFuture>(driver: Driver) -> impl Iterator<Item = T>
where
    Driver: FnOnce(Sender<T>) -> DriverFuture,
    DriverFuture: Future<Output = ()>,
{
    let pipe = Rc::new(RefCell::new(Pipe { value: None }));
    let sender = Sender { pipe: pipe.clone() };
    let driver_future = Box::new(driver(sender)).into();
    let waker = unsafe { Waker::from_raw(RAW_WAKER) };
    Receiver {
        driver_future,
        pipe,
        waker,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn generate(sender: Sender<i32>) {
        sender.send(1).await;
        sender.send(2).await;
        let mut n = 2;
        while n < 20 {
            sender.send(n).await;
            n *= 2;
        }
        sender.send(-1).await;
    }

    #[test]
    fn test_generator() {
        let result = generator(generate).map(|x| x + 3).collect::<Vec<_>>();
        assert_eq!(&result, &[4, 5, 5, 7, 11, 19, 2])
    }
}

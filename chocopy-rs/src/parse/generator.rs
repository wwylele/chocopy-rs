use std::cell::*;
use std::future::Future;
use std::pin::*;
use std::rc::*;
use std::task::*;

pub struct Sender<T> {
    pipe: Rc<RefCell<Option<T>>>,
}

pub struct SenderFuture<T> {
    pipe: Rc<RefCell<Option<T>>>,
}

impl<T> Future for SenderFuture<T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pipe.borrow().is_some() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl<T> Sender<T> {
    pub async fn send(&self, value: T) {
        assert!(self.pipe.replace(Some(value)).is_none());
        SenderFuture {
            pipe: self.pipe.clone(),
        }
        .await
    }
}

pub struct Receiver<FFuture, T> {
    f_future: Pin<Box<FFuture>>,
    pipe: Rc<RefCell<Option<T>>>,
    waker: Waker,
}

impl<FFuture: Future<Output = ()>, T> Iterator for Receiver<FFuture, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        match self
            .f_future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
        {
            Poll::Ready(()) => None,
            Poll::Pending => self.pipe.borrow_mut().take(),
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

pub fn generator<T, F, FFuture>(f: F) -> impl Iterator<Item = T>
where
    F: FnOnce(Sender<T>) -> FFuture,
    FFuture: Future<Output = ()>,
{
    let pipe = Rc::new(RefCell::new(None));
    let sender = Sender { pipe: pipe.clone() };
    let f_future = Box::new(f(sender)).into();
    let waker = unsafe { Waker::from_raw(RAW_WAKER) };
    Receiver {
        f_future,
        pipe,
        waker,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn primes(sender: Sender<usize>) {
        let mut table = [true; 20];
        for i in 2..20 {
            if table[i] {
                sender.send(i).await;
                for j in (i + i..20).step_by(i) {
                    table[j] = false;
                }
            }
        }
    }

    #[test]
    fn test() {
        let result = generator(primes).map(|x| x * 10).collect::<Vec<_>>();
        assert_eq!(&result, &[20, 30, 50, 70, 110, 130, 170, 190])
    }
}

#![feature(test)]

use std::{
    cell::RefCell,
    future::Future,
    task::{Context, Waker},
    thread,
};

extern crate test;

use crossbeam::sync::Parker;
use pin_utils;

pub fn block_on_my<F: Future>(f: F) -> F::Output {
    let thread = thread::current();
    pin_utils::pin_mut!(f);
    let waker = async_task::waker_fn(move || thread.unpark());
    let cx = &mut Context::from_waker(&waker);
    loop {
        match f.as_mut().poll(cx) {
            std::task::Poll::Ready(o) => return o,
            std::task::Poll::Pending => thread::park(),
        }
    }
}

pub fn block_on_my2<F: Future>(f: F) -> F::Output {
    pin_utils::pin_mut!(f);

    let parker = Parker::new();
    let unparker = parker.unparker().clone();
    let waker = async_task::waker_fn(move || unparker.unpark());

    let cx = &mut Context::from_waker(&waker);

    loop {
        match f.as_mut().poll(cx) {
            std::task::Poll::Ready(o) => return o,
            std::task::Poll::Pending => parker.park(),
        }
    }
}

pub fn block_on_my3<F: Future>(f: F) -> F::Output {
    pin_utils::pin_mut!(f);
    thread_local! {
        static CACHE:(Parker, Waker) =  {
            let parker = Parker::new();
            let unparker = parker.unparker().clone();
            let waker = async_task::waker_fn(move || unparker.unpark());
            (parker, waker)
        };
    }

    CACHE.with(|(parker, waker)| {
        let cx = &mut Context::from_waker(&waker);
        loop {
            match f.as_mut().poll(cx) {
                std::task::Poll::Ready(o) => return o,
                std::task::Poll::Pending => parker.park(),
            }
        }
    })
}

pub fn block_on_my4<F: Future>(f: F) -> F::Output {
    pin_utils::pin_mut!(f);
    thread_local! {
        static CACHE:RefCell<(Parker, Waker)> =  {
            let parker = Parker::new();
            let unparker = parker.unparker().clone();
            let waker = async_task::waker_fn(move || unparker.unpark());
            RefCell::new((parker, waker))
        };
    }

    CACHE.with(|cache| {
        let (parker, waker) = &mut *cache.try_borrow_mut().ok().expect("forbiden recursive");
        let cx = &mut Context::from_waker(&waker);
        loop {
            match f.as_mut().poll(cx) {
                std::task::Poll::Ready(o) => return o,
                std::task::Poll::Pending => parker.park(),
            }
        }
    })
}

struct Yield(usize);

impl Future for Yield {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        if this.0 == 0 {
            return std::task::Poll::Ready(());
        } else {
            this.0 -= 1;
            cx.waker().wake_by_ref();
            return std::task::Poll::Pending;
        }
    }
}

#[bench]
fn custom_block_on_10_yields(b: &mut test::Bencher) {
    b.iter(|| block_on_my4(Yield(10000)));
}

#[bench]
fn futures_block_on_10_yields(b: &mut test::Bencher) {
    b.iter(|| futures::executor::block_on(Yield(10000)));
}

fn main() {
    let async_fn = async {
        println!("hello world");
    };
    // block_on_my(async_fn);
    // block_on_my2(async_fn);
    block_on_my4(async_fn);
    println!("done");
}

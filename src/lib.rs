#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "slab")]
compile_error!("TODO slab support");

use core::task::Waker;
use core::cell::UnsafeCell;
use core::{ptr, mem, fmt};

pub trait WakersRef {
    fn wake_by_ref(&self);
}

pub trait Wakers: WakersRef {
    fn pend_by_ref(&self, waker: &Waker);
}

pub trait WakersMut {
    fn pend(&mut self, waker: &Waker);
    fn wake(&mut self);
}

// TODO parameterize by backing storage array size
#[derive(Debug, Clone, Default)]
pub struct WakerQueue {
    waker: Option<Waker>,
}

impl WakersMut for WakerQueue {
    fn pend(&mut self, waker: &Waker) {
        for w in &self.waker {
            if w.will_wake(waker) {
                return
            }
        }

        if let Some(w) = self.waker.take() {
            // we ran out of space, just start going wild...
            w.wake()
        }
        self.waker = Some(waker.clone());
    }

    fn wake(&mut self) {
        if let Some(w) = self.waker.take() {
            w.wake()
        }
    }
}

impl WakersRef for WakerQueue {
    fn wake_by_ref(&self) {
        for w in &self.waker {
            w.wake_by_ref()
        }
    }
}

#[cfg(feature = "const-default")]
impl const_default::ConstDefault for WakerQueue {
    const DEFAULT: Self = Self {
        waker: None,
    };
}

#[derive(Default)]
pub struct SendWakers<W> {
    /// # Safety
    ///
    /// Relies on the container not being Sync, and never exposing a shared reference to the inner data.
    ///
    /// ## Caveat
    ///
    /// Don't make a circular reference to one of these with a rawwaker, please?
    /// (waker vtables are an unsafe API so does this technically even count as a safety hole?)
    wakers: UnsafeCell<W>,
}

impl<W> SendWakers<W> {
    pub fn new(wakers: W) -> Self {
        Self {
            wakers: UnsafeCell::new(wakers),
        }
    }

    unsafe fn inner(&self) -> &W {
        &*self.wakers.get()
    }

    unsafe fn inner_mut(&self) -> &mut W {
        &mut *self.wakers.get()
    }

    pub fn get_mut(&mut self) -> &mut W {
        unsafe {
            self.inner_mut()
        }
    }

    pub fn into_inner(self) -> W {
        let res = unsafe { ptr::read(self.wakers.get()) };
        mem::forget(self);
        res
    }
}

impl<W: fmt::Debug> fmt::Debug for SendWakers<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(unsafe { self.inner() }, f)
    }
}

impl<W: Clone> Clone for SendWakers<W> {
    fn clone(&self) -> Self {
        Self::new(unsafe { self.inner() }.clone())
    }
}

#[cfg(feature = "const-default")]
impl<W: const_default::ConstDefault> const_default::ConstDefault for SendWakers<W> {
    const DEFAULT: Self = Self {
        wakers: const_default::ConstDefault::DEFAULT,
    };
}

unsafe impl<W: Send> Send for SendWakers<W> { }

impl<W: WakersMut> WakersMut for SendWakers<W> {
    fn pend(&mut self, waker: &Waker) {
        self.get_mut().pend(waker)
    }

    fn wake(&mut self) {
        self.get_mut().wake()
    }
}

impl<W: WakersMut> WakersRef for SendWakers<W> {
    fn wake_by_ref(&self) {
        unsafe { self.inner_mut() }.wake()
    }
}

impl<W: WakersMut> Wakers for SendWakers<W> {
    fn pend_by_ref(&self, waker: &Waker) {
        unsafe { self.inner_mut() }.pend(waker)
    }
}

#[cfg(feature = "std")]
mod sync_wakers {
    use std::task::Waker;
    use std::sync::Mutex;
    use std::fmt;
    use super::{Wakers, WakersRef, WakersMut};

    // TODO a Vec-backed waker queue?

    #[derive(Default)]
    pub struct SyncWakers<W> {
        wakers: Mutex<W>,
    }

    impl<W: Clone> Clone for SyncWakers<W> {
        fn clone(&self) -> Self {
            Self::new(self.wakers.lock().unwrap().clone())
        }
    }

    impl<W: fmt::Debug> fmt::Debug for SyncWakers<W> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Debug::fmt(&self.wakers.lock().unwrap(), f)
        }
    }

    impl<W> SyncWakers<W> {
        pub fn new(wakers: W) -> Self {
            Self {
                wakers: Mutex::new(wakers),
            }
        }

        pub fn get_mut(&mut self) -> &mut W {
            self.wakers.get_mut().unwrap()
        }

        pub fn into_inner(self) -> W {
            self.wakers.into_inner().unwrap()
        }
    }

    impl<W: WakersMut> WakersMut for SyncWakers<W> {
        fn pend(&mut self, waker: &Waker) {
            self.get_mut().pend(waker)
        }

        fn wake(&mut self) {
            self.get_mut().wake()
        }
    }

    impl<W: WakersMut> Wakers for SyncWakers<W> {
        fn pend_by_ref(&self, waker: &Waker) {
            self.wakers.lock().unwrap().pend(waker)
        }
    }

    impl<W: WakersMut> WakersRef for SyncWakers<W> {
        fn wake_by_ref(&self) {
            self.wakers.lock().unwrap().wake()
        }
    }
}
#[cfg(feature = "std")]
pub use sync_wakers::SyncWakers;

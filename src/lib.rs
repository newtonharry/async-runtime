use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

// A timer which utilizes a shared_state to interact with the executor
pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

// Shared state between the future and the waiting thread which is polling the future
struct SharedState {
    // Weather or not the sleep time has elapsed
    completed: bool,

    // The waker for the task that TimerFuture is running on
    // After the thread has set completed to true, it tells TimerFutures task to wake up
    // and see that completed is true. It then moves on after.
    waker: Option<Waker>,
}

// Implement the future trait for the TimerFuture
impl Future for TimerFuture {
    type Output = ();
    // The polling function which will be called to check if the future is done or pending
    // The context contains the waker function
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Get the shared state which is being shared between this function and the thread in its new function
        let mut shared_state = self.shared_state.lock().unwrap();

        if shared_state.completed {
            Poll::Ready(())
        } else {
            // Assign the waker in the shared state  a created waker for this future so it can be polled again
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = shared_state.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().unwrap();
            shared_state.completed = true;

            // If a waker has been created for this future, wake up the poller and poll this future again to see if it is done
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        TimerFuture { shared_state }
    }
}

use futures::{
    future::{BoxFuture, FutureExt},
    task::{waker_ref, ArcWake},
};

use std::{
    future::Future,
    sync::mpsc::{sync_channel, Receiver, SyncSender},
    sync::{Arc, Mutex},
    task::Context,
    time::Duration,
};

// The timer we wrote in the previous section:
use timer_future::TimerFuture;

// Contains the receiver of a channel which receives tasks
struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    // Checks the receiving end of the channel for queued tasks
    // If it is pending, then
    fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            // Gets the future from a task
            let mut future_slot = task.future.lock().unwrap();
            // If the there is a task
            if let Some(mut future) = future_slot.take() {
                // Get the waker from the task
                let waker = waker_ref(&task);
                // Create a context for the future with its assocated waker
                let context = &mut Context::from_waker(&*waker);
                // Pass the context with the waker function to the polling function of the future being executed
                // Check if the future is still pending
                if future.as_mut().poll(context).is_pending() {
                    // The future is not done processing so put it back in its tasks to be run again in the future
                    *future_slot = Some(future); // This will unlock the future as it has been re-assigned
                }
                // The future_slot is left as none therefore task is not going be polled again
            }
        }
    }
}

// Contains the sending end of a channel to send tasks to the executor
struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    // Takes a function which implements a future, wraps it in a Task, and sends it to the executor
    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        // Take the passed in future (function or anonymous function) and box it (make a Pin type)
        let future = future.boxed();
        // Create a new Task with the future and its associated sender
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Send the task to the executor
        self.task_sender.send(task).expect("Too many tasks queued")
    }
}

// Holds the future to be polled and the task sender in order to be re-queued on the executor
struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Task>>,
}

// Implement the ArcWake trait for sent tasks to the executor
// This allows for the task to be re-queued on the channel to be polled again
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned)
            .expect("too many tasks queued");
    }
}

// Create a sender and receiver for the executor and spawner
fn new_executor_and_spawner() -> (Executor, Spawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;

    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}

fn main() {
    // Create the executor and spawner
    let (executor, spawner) = new_executor_and_spawner();

    // Spawn an anonoymous future which gets sent to the executor
    spawner.spawn(async {
        println!("Howdy!");
        TimerFuture::new(Duration::new(2, 0)).await;
        println!("Done!");
    });

    // Drop the spawner as we no longer need it
    drop(spawner);

    // Run the executor to start processing queued tasks on the channel
    executor.run();
}

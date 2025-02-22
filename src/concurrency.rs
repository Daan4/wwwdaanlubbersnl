use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of workers in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        Self {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Create a new Worker.
    ///
    /// The id is the id of the worker and thread is the thread that the worker is running on.
    ///
    /// Todo: use std::thread::Builder and handle panics
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::Builder::new()
            .name(format!("Worker {}", id))
            .spawn(move || loop {
                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");

                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            });

        let thread = match thread {
            Ok(thread) => thread,
            Err(e) => panic!("Failed to create thread: {e:?}"),
        };

        Self {
            id,
            thread: Some(thread),
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time;

    #[test]
    #[should_panic]
    fn threadpool_new_panics_with_zero_size() {
        ThreadPool::new(0);
    }

    #[test]
    fn threadpool_new() {
        let pool = ThreadPool::new(1);
        assert_eq!(pool.workers.len(), 1);
        let pool = ThreadPool::new(4);
        assert_eq!(pool.workers.len(), 4);
        let pool = ThreadPool::new(32);
        assert_eq!(pool.workers.len(), 32);
    }

    #[test]
    fn threadpool_execute() {
        let pool = ThreadPool::new(4);
        for _ in 0..99 {
            pool.execute(|| {
                thread::sleep(time::Duration::from_millis(10));
            });
        }
    }

    #[test]
    fn worker_stops_when_sender_disconnects() {
        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));
        let mut worker = Worker::new(0, receiver);
        drop(sender);
        if let Some(thread) = worker.thread.take() {
            thread.join().unwrap();
        }
    }
}

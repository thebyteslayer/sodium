// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;

pub type TaskResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub enum Task {
    CacheGet {
        key: String,
        sender: oneshot::Sender<TaskResult<Option<String>>>,
    },
    CacheSet {
        key: String,
        value: String,
        sender: oneshot::Sender<TaskResult<()>>,
    },
    CacheDelete {
        key: String,
        sender: oneshot::Sender<TaskResult<bool>>,
    },
    CacheKeys {
        sender: oneshot::Sender<TaskResult<Vec<String>>>,
    },
}

struct WorkQueue {
    queue: Mutex<VecDeque<Task>>,
    is_shutdown: AtomicBool,
}

impl WorkQueue {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            is_shutdown: AtomicBool::new(false),
        }
    }

    fn push(&self, task: Task) -> bool {
        if self.is_shutdown.load(Ordering::Relaxed) {
            return false;
        }
        
        if let Ok(mut queue) = self.queue.try_lock() {
            queue.push_back(task);
            true
        } else {
            false
        }
    }

    fn pop(&self) -> Option<Task> {
        if let Ok(mut queue) = self.queue.try_lock() {
            queue.pop_front()
        } else {
            None
        }
    }

    fn steal(&self) -> Option<Task> {
        if let Ok(mut queue) = self.queue.try_lock() {
            queue.pop_back()
        } else {
            None
        }
    }

    fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed);
    }
}

pub struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    queues: Vec<Arc<WorkQueue>>,
    next_queue: AtomicUsize,
    shutdown: Arc<AtomicBool>,
}

impl ThreadPool {
    pub fn new() -> Self {
        let num_threads = num_cpus::get();
        let mut workers = Vec::with_capacity(num_threads);
        let mut queues = Vec::with_capacity(num_threads);
        let shutdown = Arc::new(AtomicBool::new(false));

        for _ in 0..num_threads {
            queues.push(Arc::new(WorkQueue::new()));
        }

        for i in 0..num_threads {
            let worker_queues = queues.clone();
            let worker_shutdown = shutdown.clone();
            let worker_id = i;

            let handle = thread::spawn(move || {
                Self::worker_loop(worker_id, worker_queues, worker_shutdown);
            });

            workers.push(handle);
        }

        Self {
            workers,
            queues,
            next_queue: AtomicUsize::new(0),
            shutdown,
        }
    }

    pub fn execute(&self, task: Task) -> bool {
        if self.shutdown.load(Ordering::Relaxed) {
            return false;
        }

        let queue_index = self.next_queue.fetch_add(1, Ordering::Relaxed) % self.queues.len();
        let queue = &self.queues[queue_index];
        
        queue.push(task)
    }

    fn worker_loop(
        worker_id: usize,
        queues: Vec<Arc<WorkQueue>>,
        shutdown: Arc<AtomicBool>,
    ) {
        let my_queue = &queues[worker_id];
        let mut idle_count = 0u32;
        
        while !shutdown.load(Ordering::Relaxed) {
            if let Some(task) = my_queue.pop() {
                Self::execute_task(task);
                idle_count = 0;
                continue;
            }

            let mut found_work = false;
            for (i, queue) in queues.iter().enumerate() {
                if i != worker_id {
                    if let Some(task) = queue.steal() {
                        Self::execute_task(task);
                        found_work = true;
                        idle_count = 0;
                        break;
                    }
                }
            }

            if !found_work {
                idle_count += 1;
                let sleep_duration = std::cmp::min(idle_count, 50);
                thread::sleep(Duration::from_millis(sleep_duration as u64));
            }
        }
    }

    fn execute_task(task: Task) {
        match task {
            Task::CacheGet { key, sender } => {
                let result = crate::cache::execute_get(&key);
                let _ = sender.send(result);
            }
            Task::CacheSet { key, value, sender } => {
                let result = crate::cache::execute_set(key, value);
                let _ = sender.send(result);
            }
            Task::CacheDelete { key, sender } => {
                let result = crate::cache::execute_delete(&key);
                let _ = sender.send(result);
            }
            Task::CacheKeys { sender } => {
                let result = crate::cache::execute_keys();
                let _ = sender.send(result);
            }
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        
        for queue in &self.queues {
            queue.shutdown();
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.shutdown();
        
        while let Some(handle) = self.workers.pop() {
            let _ = handle.join();
        }
    }
}

static THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

pub fn initialize_threading() {
    let _ = THREAD_POOL.set(ThreadPool::new());
}

pub fn get_thread_pool() -> &'static ThreadPool {
    THREAD_POOL.get().expect("Thread pool not initialized")
}

pub async fn execute_cache_get(key: String) -> TaskResult<Option<String>> {
    let (sender, receiver) = oneshot::channel();
    let task = Task::CacheGet { key, sender };
    
    if get_thread_pool().execute(task) {
        receiver.await.unwrap_or_else(|_| Err("Task execution failed".into()))
    } else {
        Err("Failed to queue task".into())
    }
}

pub async fn execute_cache_set(key: String, value: String) -> TaskResult<()> {
    let (sender, receiver) = oneshot::channel();
    let task = Task::CacheSet { key, value, sender };
    
    if get_thread_pool().execute(task) {
        receiver.await.unwrap_or_else(|_| Err("Task execution failed".into()))
    } else {
        Err("Failed to queue task".into())
    }
}

pub async fn execute_cache_delete(key: String) -> TaskResult<bool> {
    let (sender, receiver) = oneshot::channel();
    let task = Task::CacheDelete { key, sender };
    
    if get_thread_pool().execute(task) {
        receiver.await.unwrap_or_else(|_| Err("Task execution failed".into()))
    } else {
        Err("Failed to queue task".into())
    }
}

pub async fn execute_cache_keys() -> TaskResult<Vec<String>> {
    let (sender, receiver) = oneshot::channel();
    let task = Task::CacheKeys { sender };
    
    if get_thread_pool().execute(task) {
        receiver.await.unwrap_or_else(|_| Err("Task execution failed".into()))
    } else {
        Err("Failed to queue task".into())
    }
} 
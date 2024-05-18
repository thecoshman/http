use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::sync::Arc;
use std::thread;

use net::NetworkListener;


pub struct ListenerPool<A: NetworkListener> {
    acceptor: A
}

impl<A: NetworkListener + Send + 'static> ListenerPool<A> {
    /// Create a thread pool to manage the acceptor.
    pub fn new(acceptor: A) -> ListenerPool<A> {
        ListenerPool { acceptor: acceptor }
    }

    /// Runs the acceptor pool. Blocks until the acceptors are closed.
    ///
    /// ## Panics
    ///
    /// Panics if max_threads == 0.
    pub fn accept<F>(mut self, work: F, max_threads: usize)
        where F: Fn(A::Stream) + Send + Sync + 'static {
        assert!(max_threads != 0, "Can't accept on 0 threads.");

        let work = Arc::new(work);
        let live_threads = Arc::new(AtomicUsize::new(0));
        let free_threads = Arc::new(AtomicUsize::new(0));
        let (send, recv) = crossbeam_channel::bounded(20);

        loop {
            let msg = match self.acceptor.accept() {
                Ok(stream) => stream,
                Err(e) => {
                    info!("Connection failed: {}", e);
                    continue;
                }
            };

            let free = free_threads.load(Ordering::Acquire);
            let live = live_threads.load(Ordering::SeqCst);
            // eprintln!("free = {}, live = {}", free, live);
            if (live == 0 || free == 0) && live != max_threads {
                spawn_with::<A, _>(recv.clone(), work.clone(), live_threads.clone(), free_threads.clone(), msg);
            } else {
                let _ = send.send(msg);
            }
        }
    }
}

fn spawn_with<A, F>(recv: crossbeam_channel::Receiver<A::Stream>, work: Arc<F>, live_threads: Arc<AtomicUsize>, free_threads: Arc<AtomicUsize>, first: A::Stream)
where A: NetworkListener + Send + 'static,
      F: Fn(<A as NetworkListener>::Stream) + Send + Sync + 'static {
    thread::spawn(move || {
        let thread_id = live_threads.fetch_add(1, Ordering::SeqCst);
        let _sentinel = LiveSentinel { live_threads };

        let mut _free_sentinel = FreeSentinel { free_threads: &free_threads, subbed: true };
        work(first);
        _free_sentinel.unsub();

        loop {
            let stream = match if thread_id == 0 {
                recv.recv().ok()  // infallible
            } else {
                recv.recv_timeout(Duration::from_secs((thread_id * 5).min(300) as u64)).ok()
            } {
                None => return,
                Some(stream) => stream,
            };

            _free_sentinel.sub();
            work(stream);
            _free_sentinel.unsub();
        }
    });
}

struct LiveSentinel {
    live_threads: Arc<AtomicUsize>,
}
impl Drop for LiveSentinel {
    fn drop(&mut self) {
        self.live_threads.fetch_sub(1, Ordering::SeqCst);
    }
}

struct FreeSentinel<'t> {
    free_threads: &'t Arc<AtomicUsize>,
    subbed: bool,
}
impl<'t> FreeSentinel<'t> {
    fn sub(&mut self) {
        self.free_threads.fetch_sub(1, Ordering::AcqRel);
        self.subbed = true;
    }
    fn unsub(&mut self) {
        self.free_threads.fetch_add(1, Ordering::AcqRel);
        self.subbed = false;
    }
}
impl<'t> Drop for FreeSentinel<'t> {
    fn drop(&mut self) {
        if !self.subbed {
            self.free_threads.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

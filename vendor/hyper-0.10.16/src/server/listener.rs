use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::sync::Arc;
use std::thread;

use net::NetworkListener;


type Message<A> = Option<<A as NetworkListener>::Stream>;

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
        let queue_depth = Arc::new(AtomicUsize::new(0));
        let (mut send, recv) = spmc::channel();

        loop {
            let msg = match self.acceptor.accept() {
                Ok(stream) => Some(stream),
                Err(::error::Error::__Nonexhaustive(..)) => None,  // timeout
                Err(e) => {
                    info!("Connection failed: {}", e);
                    continue;
                }
            };

            let free = free_threads.load(Ordering::Acquire);
            let live = live_threads.load(Ordering::SeqCst);
            // eprintln!("free = {}, live = {}", free, live);
            if msg.is_some() && free == 0 && live != max_threads {
                spawn_with::<A, _>(recv.clone(), work.clone(), live_threads.clone(), free_threads.clone(), queue_depth.clone(), msg.unwrap());
                if live + 1 > 1 {
                    self.acceptor.accept_with_timeout(true);
                }
                continue;
            }
            self.acceptor.accept_with_timeout(live > 1);
            if msg.is_none() && (free == 0 || live <= 1) {
                continue;
            }

            let depth = queue_depth.fetch_add(1, Ordering::Relaxed);
            send.send(msg).expect("impossible");
            if depth > 20 {
                while queue_depth.load(Ordering::Relaxed) > 5 {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

fn spawn_with<A, F>(recv: spmc::Receiver<Message<A>>, work: Arc<F>, live_threads: Arc<AtomicUsize>, free_threads: Arc<AtomicUsize>, queue_depth: Arc<AtomicUsize>, first: A::Stream)
where A: NetworkListener + Send + 'static,
      F: Fn(<A as NetworkListener>::Stream) + Send + Sync + 'static {
    thread::spawn(move || {
        let impervious = live_threads.fetch_add(1, Ordering::SeqCst) == 0;
        let _sentinel = LiveSentinel { live_threads };

        work(first);
        free_threads.fetch_add(1, Ordering::AcqRel);
        let _sentinel = FreeSentinel { free_threads: &free_threads };
        loop {
            let msg = recv.recv().expect("impossible");
            queue_depth.fetch_sub(1, Ordering::Relaxed);
            match msg {
                None => if !impervious { return },
                Some(stream) => {
                    free_threads.fetch_sub(1, Ordering::AcqRel);
                    work(stream);
                    free_threads.fetch_add(1, Ordering::AcqRel);
                }
            }
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
}
impl<'t> Drop for FreeSentinel<'t> {
    fn drop(&mut self) {
        self.free_threads.fetch_sub(1, Ordering::AcqRel);
    }
}

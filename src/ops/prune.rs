use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use iron::{IronResult, Response, Handler, Request};
use self::super::super::util::human_readable_size;
use self::super::super::Options;
use self::super::HttpHandler;
use time::precise_time_ns;
use std::fs;


pub struct PruneChain {
    pub handler: HttpHandler,
    pub encoded_filesystem_limit: Option<u64>,
    pub encoded_generated_limit: Option<u64>,
    pub encoded_prune: Option<u64>,

    pub prune_interval: u64, // s
    last_prune: AtomicU64, // ns
}

impl PruneChain {
    pub fn new(opts: &Options) -> PruneChain {
        PruneChain {
            handler: HttpHandler::new(opts),
            encoded_filesystem_limit: opts.encoded_filesystem_limit,
            encoded_generated_limit: opts.encoded_generated_limit,
            encoded_prune: opts.encoded_prune,

            prune_interval: (opts.encoded_prune.unwrap_or(0) / 6).max(10),
            last_prune: AtomicU64::new(0),
        }
    }

    pub fn prune(&self) {
        let mut start = 0u64;
        let mut freed_fs = 0u64;
        let mut freed_gen = 0u64;


        if let Some(limit) = self.encoded_filesystem_limit {
            if self.handler.cache_fs_size.load(AtomicOrdering::Relaxed) > limit {
                start = precise_time_ns();

                let mut cache = self.handler.cache_fs.write().expect("Filesystem cache write lock poisoned");
                let size = self.handler.cache_fs_size.load(AtomicOrdering::Relaxed);
                while size - freed_fs > limit {
                    let key = match cache.iter().min_by_key(|i| (i.1).1.load(AtomicOrdering::Relaxed)) {
                        Some((key, ((path, _, _), _))) => {
                            match fs::remove_file(path) {
                                Ok(()) => key.clone(),
                                Err(_) => break,
                            }
                        }
                        None => break,
                    };
                    let ((_, _, sz), _) = cache.remove(&key).unwrap();
                    freed_fs += sz;
                }
                self.handler.cache_fs_size.fetch_sub(freed_fs, AtomicOrdering::Relaxed);
            }
        }

        if let Some(limit) = self.encoded_generated_limit {
            if self.handler.cache_gen_size.load(AtomicOrdering::Relaxed) > limit {
                if start == 0 {
                    start = precise_time_ns();
                }

                let mut cache = self.handler.cache_gen.write().expect("Generated file cache write lock poisoned");
                let size = self.handler.cache_gen_size.load(AtomicOrdering::Relaxed);
                while size - freed_gen > limit {
                    let key = match cache.iter().min_by_key(|i| (i.1).1.load(AtomicOrdering::Relaxed)) {
                        Some((key, _)) => key.clone(),
                        None => break,
                    };
                    let (data, _) = cache.remove(&key).unwrap();
                    freed_gen += data.len() as u64;
                }
                self.handler.cache_gen_size.fetch_sub(freed_gen, AtomicOrdering::Relaxed);
            }
        }

        if let Some(limit) = self.encoded_prune {
            if start == 0 {
                start = precise_time_ns();
            }

            let last = self.last_prune.swap(start, AtomicOrdering::Relaxed);
            if last < start && (start - last) / 1000 / 1000 / 1000 >= self.prune_interval {
                {
                    let mut cache = self.handler.cache_fs.write().expect("Filesystem cache write lock poisoned");
                    cache.retain(|_, ((path, _, sz), atime)| {
                        let atime = atime.load(AtomicOrdering::Relaxed);
                        if atime > start || (start - atime) / 1000 / 1000 / 1000 <= limit {
                            return true;
                        }

                        if fs::remove_file(path).is_err() {
                            return true;
                        }
                        freed_fs += *sz;
                        self.handler.cache_fs_size.fetch_sub(*sz, AtomicOrdering::Relaxed);
                        false
                    });
                }
                {
                    let mut cache = self.handler.cache_gen.write().expect("Generated file cache write lock poisoned");
                    cache.retain(|_, (data, atime)| {
                        let atime = atime.load(AtomicOrdering::Relaxed);
                        if atime > start || (start - atime) / 1000 / 1000 / 1000 <= limit {
                            return true;
                        }

                        freed_gen += data.len() as u64;
                        self.handler.cache_gen_size.fetch_sub(data.len() as u64, AtomicOrdering::Relaxed);
                        false
                    });
                }
            }
        }

        if freed_fs != 0 || freed_gen != 0 {
            let end = precise_time_ns();
            log!(self.handler.log,
                 "Pruned {} + {} in {}ns; used: {} + {}",
                 human_readable_size(freed_fs),
                 human_readable_size(freed_gen),
                 end - start,
                 human_readable_size(self.handler.cache_fs_size.load(AtomicOrdering::Relaxed)),
                 human_readable_size(self.handler.cache_gen_size.load(AtomicOrdering::Relaxed)));
        }
    }
}

impl Handler for PruneChain {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let resp = self.handler.handle(req);
        self.prune();
        resp
    }
}

// SPDX-License-Identifier: MIT

//! One shared, nonblocking provider admission pool per runtime instance.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_CONCURRENT_REMOTE_FETCHES: usize = 4;

#[derive(Clone, Default)]
pub struct RemoteFetchPool {
    active: Arc<AtomicUsize>,
}

pub struct RemoteFetchPermit {
    active: Arc<AtomicUsize>,
}

impl RemoteFetchPool {
    /// Clones share the same limit. A future batch executor must use the
    /// runtime's pool, not construct a second pool for its background work.
    pub fn try_acquire(&self) -> Option<RemoteFetchPermit> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_CONCURRENT_REMOTE_FETCHES).then_some(count + 1)
            })
            .ok()
            .map(|_| RemoteFetchPermit {
                active: Arc::clone(&self.active),
            })
    }
}

impl Drop for RemoteFetchPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn cloned_pools_share_the_limit_and_release_capacity_on_drop() {
        let pool = RemoteFetchPool::default();
        let clone = pool.clone();
        let mut permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
            .map(|_| pool.try_acquire().unwrap())
            .collect();
        assert!(clone.try_acquire().is_none());
        permits.pop();
        let replacement = clone.try_acquire().unwrap();
        assert!(pool.try_acquire().is_none());
        drop(replacement);
        drop(permits);
        assert_eq!(pool.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn concurrent_admission_never_exceeds_the_shared_limit() {
        let pool = RemoteFetchPool::default();
        let start = Arc::new(Barrier::new(17));
        let admitted = Arc::new(Barrier::new(17));
        let release = Arc::new(Barrier::new(17));
        thread::scope(|scope| {
            for _ in 0..16 {
                let pool = pool.clone();
                let start = Arc::clone(&start);
                let admitted = Arc::clone(&admitted);
                let release = Arc::clone(&release);
                scope.spawn(move || {
                    start.wait();
                    let permit = pool.try_acquire();
                    admitted.wait();
                    release.wait();
                    drop(permit);
                });
            }
            start.wait();
            admitted.wait();
            assert_eq!(
                pool.active.load(Ordering::Acquire),
                MAX_CONCURRENT_REMOTE_FETCHES
            );
            release.wait();
        });
        assert_eq!(pool.active.load(Ordering::Acquire), 0);
    }
}

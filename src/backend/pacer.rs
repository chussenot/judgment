//! A bound on calls in flight, and one pause every call honours.
//!
//! A program that makes many calls at once, a batch over thousands of
//! cases, has two things the retry loop cannot give it. It has no way to
//! say "at most eight at a time", and when the server answers 429 with a
//! wait, only the one call that got the answer waits: the others, already
//! in flight or queued, go on sending and are refused in their turn, each
//! spending an attempt. A [`Pacer`] is both: a semaphore for the first,
//! and a deadline that [`Pacer::pause_for`] only ever moves later for the
//! second. It is cheap to clone, and every clone is the same pacer.
//!
//! [`Paced`] wraps any [`SystemOne`] with one, so the bound works over a
//! [`Fake`](crate::Fake) in a test as over the client. A client given the
//! same pacer ([`ClientBuilder::pacer`](crate::client::ClientBuilder::pacer))
//! also feeds the pause: a retryable 429 or 529 that names a wait pauses the
//! pacer for it, so every call on that pacer holds its next attempt until
//! the wait is over. Billed responses are never retried, and the wait is
//! the one the retry policy already capped
//! ([`RetryPolicy::retry_after_max`](crate::http::RetryPolicy)).

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::time::Instant;

use super::{BoxFuture, SystemOne};
use crate::answer::Response;
use crate::asset::Assets;
use crate::error::Result;
use crate::question::Questions;

/// The longest a single pause is taken for: a day. Nothing here asks for
/// more (the retry loop's wait is capped at 30 s by default), and a clamp
/// keeps a pause from overflowing the clock.
const LONGEST_PAUSE: Duration = Duration::from_secs(24 * 60 * 60);

/// Permits for calls in flight and a deadline before which no call sends
/// (module docs). Clone it to share it.
#[derive(Debug, Clone)]
pub struct Pacer {
    permits: Arc<Semaphore>,
    until: Arc<watch::Sender<Instant>>,
}

impl Pacer {
    /// A pacer allowing `max_in_flight` calls at once and no pause. Zero is
    /// read as one: a pacer that admits nothing would hang every call. More
    /// than the semaphore can hold ([`Semaphore::MAX_PERMITS`]) is read as
    /// that many, which is no bound in practice.
    #[must_use]
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(
                max_in_flight.clamp(1, Semaphore::MAX_PERMITS),
            )),
            until: Arc::new(watch::channel(Instant::now()).0),
        }
    }

    /// Hold every call on this pacer for `wait` from now. Monotonic: a
    /// pause that would end before the one already running changes
    /// nothing, so two calls reporting different waits leave the longer one
    /// in force. A wait over a day is taken as a day.
    pub fn pause_for(&self, wait: Duration) {
        let until = Instant::now() + wait.min(LONGEST_PAUSE);
        self.until.send_if_modified(|current| {
            let later = until > *current;
            if later {
                *current = until;
            }
            later
        });
    }

    /// Return once no pause is running. A pause extended while this sleeps
    /// is waited out too.
    pub(crate) async fn wait(&self) {
        loop {
            let until = *self.until.borrow();
            if until <= Instant::now() {
                return;
            }
            tokio::time::sleep_until(until).await;
        }
    }

    /// A place among the calls in flight, given back when it is dropped, so
    /// a call that is cancelled or fails frees it. The semaphore is never
    /// closed, so there is always one; the `Option` is only what tokio's
    /// signature asks for.
    async fn admit(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.permits).acquire_owned().await.ok()
    }

    /// When the pause now running ends, for the tests.
    #[cfg(test)]
    fn until(&self) -> Instant {
        *self.until.borrow()
    }
}

/// A [`SystemOne`] that makes at most the pacer's number of calls at once
/// on the backend it wraps, each waiting out the pacer's pause first.
///
/// The place is held for the whole call, retries included, and given back
/// when the call's future is dropped, so cancelling a call frees it.
#[derive(Debug)]
pub struct Paced<B> {
    inner: B,
    pacer: Pacer,
}

impl<B: SystemOne> Paced<B> {
    /// Bound `inner`'s calls by `pacer`. Give the same pacer to a client
    /// ([`ClientBuilder::pacer`](crate::client::ClientBuilder::pacer)) so
    /// its 429s pause these calls too.
    pub const fn new(inner: B, pacer: Pacer) -> Self {
        Self { inner, pacer }
    }

    /// The wrapped backend.
    pub const fn inner(&self) -> &B {
        &self.inner
    }

    /// The pacer, to clone it into another [`Paced`] or a client.
    pub const fn pacer(&self) -> &Pacer {
        &self.pacer
    }
}

impl<B: SystemOne> SystemOne for Paced<B> {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let _place = self.pacer.admit().await;
            self.pacer.wait().await;
            self.inner.answer(state, model, questions).await
        })
    }

    fn answer_with_assets<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
        assets: &'a Assets,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let _place = self.pacer.admit().await;
            self.pacer.wait().await;
            self.inner
                .answer_with_assets(state, model, questions, assets)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_pause_is_never_shortened() {
        let pacer = Pacer::new(1);
        pacer.pause_for(Duration::from_secs(10));
        let long = pacer.until();
        pacer.pause_for(Duration::from_secs(1));
        assert_eq!(pacer.until(), long, "a shorter pause changed the deadline");
        pacer.pause_for(Duration::from_secs(20));
        assert!(pacer.until() > long, "a longer pause did not extend it");
        // Time passing does not shorten it either: the deadline is absolute.
        tokio::time::advance(Duration::from_secs(5)).await;
        pacer.pause_for(Duration::ZERO);
        assert!(pacer.until() > long);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_returns_when_the_pause_ends_and_not_before() {
        let pacer = Pacer::new(1);
        let started = Instant::now();
        pacer.wait().await;
        assert_eq!(started.elapsed(), Duration::ZERO, "no pause, no wait");

        pacer.pause_for(Duration::from_secs(3));
        pacer.wait().await;
        assert_eq!(started.elapsed(), Duration::from_secs(3));

        // A pause extended while a call waits is waited out too.
        pacer.pause_for(Duration::from_secs(2));
        let extender = pacer.clone();
        let extend = async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            extender.pause_for(Duration::from_secs(4));
        };
        tokio::join!(pacer.wait(), extend);
        assert_eq!(started.elapsed(), Duration::from_secs(8));
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_over_a_day_is_a_day_and_zero_places_is_one() {
        let pacer = Pacer::new(0);
        assert_eq!(pacer.permits.available_permits(), 1);
        let started = Instant::now();
        pacer.pause_for(Duration::MAX);
        assert_eq!(pacer.until() - started, LONGEST_PAUSE);
    }

    #[test]
    fn more_places_than_the_semaphore_holds_is_the_most_it_holds() {
        let pacer = Pacer::new(usize::MAX);
        assert_eq!(pacer.permits.available_permits(), Semaphore::MAX_PERMITS);
        // A bound inside the range is taken as given.
        assert_eq!(Pacer::new(8).permits.available_permits(), 8);
    }
}

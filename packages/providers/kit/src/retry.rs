//! A request asked again when the answer says a moment's wait may mend it: at most [`ATTEMPTS`]
//! times, the pause doubling between them. Which answers those are is the caller's to say
//! ([`TRANSIENT`] for the statuses every host means so); a request that went out and got no whole
//! answer (`HttpError::Unanswered`) may have been acted on and billed, and is not asked again
//! unless the caller says so.

use std::time::Duration;

/// Attempts at most, the first among them.
pub const ATTEMPTS: u32 = 3;
/// The first pause; each next one is twice as long.
pub const PAUSE: Duration = Duration::from_millis(800);
/// Statuses a moment's wait may mend on any host: a timeout, an upstream gone or overloaded.
pub const TRANSIENT: [u16; 6] = [408, 500, 502, 503, 504, 529];

/// What one attempt came to.
pub enum Attempt<T, E> {
    /// The answer, or a failure no wait mends.
    Done(Result<T, E>),
    /// A failure another attempt may mend.
    Again(E),
}

/// `attempt` until it is done, or its last failure after [`ATTEMPTS`]; `pause` before the second.
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff between attempts, which no event ends sooner"
)]
pub fn retried<T, E>(pause: Duration, mut attempt: impl FnMut() -> Attempt<T, E>) -> Result<T, E> {
    let mut wait = pause;
    for _ in 1..ATTEMPTS {
        match attempt() {
            Attempt::Done(result) => return result,
            Attempt::Again(_) => {
                std::thread::sleep(wait);
                wait = wait.saturating_mul(2);
            }
        }
    }
    match attempt() {
        Attempt::Done(result) => result,
        Attempt::Again(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;

//! Translation: the socket, the worker, and what comes back to the panel.
//!
//! ## Why the HTTP lives here and not in `hub-core`
//!
//! `hub_core::translation` owns the prompt, the body, and the mapping from a status
//! code to a sentence. It owns no socket on purpose -- those cases are testable
//! without a server -- and the architecture guard bans `reqwest` below `app`, so
//! the one place a request can be made is the composition root. That is not a
//! limitation to work around; it is the layering saying where the impurity is.
//!
//! ## Why a thread per request and not a worker pool
//!
//! A `blocking` client cannot be aborted mid-flight. With one long-lived worker, a
//! request the user has already replaced would hold the queue for up to the full
//! budget, and the *next* translation would inherit the wait. One thread per press
//! means an abandoned request is just an abandoned thread: it finishes, its answer
//! is dropped by id (see [`Engine::poll`]), and the panel never waited for it.
//!
//! Requests are user-initiated and serial in practice, so the thread cost is one
//! thread per keystroke at worst -- not a pool worth building.
//!
//! ## Modelled on the shipped implementations
//!
//! The base-URL rules, the 60 s budget, the connect timeout and the decision to
//! report "timeout" separately from "connection failed" come from
//! `easyflowhub-app/src/features/translation/index.ts` (the adapter) and from the
//! shipped app's Rust transcription module (the same rules, already in Rust and
//! already using `reqwest::blocking`). Neither file name is repeated here whole: the
//! layering guard bans the old shell's name from this crate's source, comments
//! included, and that is a rule worth keeping rather than routing around.

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hub_core::translation::{
    self, Problem, Reply, Request, COMPLETION_PATH, REQUEST_TIMEOUT_SECS,
};

/// How long the connection may take before Relay counts as unreachable.
///
/// Separate from the request budget because the two failures mean different things
/// to the user -- "Relay 没启动" versus "Provider 卡住了" -- and telling them apart
/// after 60 s of silence is too late to be useful.
const CONNECT_TIMEOUT_SECS: u64 = 15;

/// A Relay answer, before anything has been decided about it.
///
/// The status code is kept as a number rather than a `StatusCode` so the mapping
/// stays in `hub-core` (which does not know what a `reqwest` type is) and so the
/// tests can produce any code without a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
}

/// The seam between "decide what to ask" and "ask it".
///
/// Exists so the interesting half -- which status becomes which sentence, what a
/// 200 with no `choices` means -- is testable without binding a port.
pub trait Transport: Send + Sync {
    fn send(&self, url: &str, body: &str) -> Result<Answer, Problem>;
}

/// The real one: a blocking POST through the local Relay.
pub struct Relay {
    client: reqwest::blocking::Client,
}

impl Relay {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            // Falling back to the default client keeps the feature working with a
            // longer connect timeout rather than refusing to start the panel.
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client }
    }
}

impl Default for Relay {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for Relay {
    fn send(&self, url: &str, body: &str) -> Result<Answer, Problem> {
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            // No `Authorization`: Relay owns the provider keys and the routing
            // (ADR-0008). A header here would be a second credential store.
            .body(body.to_string())
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    Problem::Timeout
                } else {
                    Problem::Offline
                }
            })?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|_| Problem::Malformed)?;
        Ok(Answer { status, body })
    }
}

/// Normalise a configured base onto `…/v1/chat/completions`.
///
/// Accepts a bare origin, an origin with a trailing slash, one already ending in
/// `/v1`, and a prefixed deployment ending in `/relay/v1` -- the same four shapes
/// `transcription_endpoint` accepts. Credentials in the URL, a query and a fragment
/// are rejected rather than dropped: a `?token=…` that the user typed and this
/// silently discarded would be a setting that looks applied and is not.
pub fn endpoint(base: &str) -> Result<String, Problem> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(Problem::InvalidUrl);
    }
    let mut url = reqwest::Url::parse(trimmed).map_err(|_| Problem::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Problem::InvalidUrl);
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Problem::InvalidUrl);
    }
    // Strip a trailing `/v1` before appending, because `COMPLETION_PATH` already
    // begins with it: `/relay/v1` must not become `/relay/v1/v1/chat/completions`.
    let path = url.path().trim_end_matches('/');
    let root = path.strip_suffix("/v1").unwrap_or(path);
    url.set_path(&format!("{root}{COMPLETION_PATH}"));
    Ok(url.to_string())
}

/// Ask and interpret, in that order. Pure with respect to the network: everything
/// that touches a socket is behind `transport`.
pub fn run(transport: &dyn Transport, relay_url: &str, request: &Request) -> Result<Reply, Problem> {
    // Built before the URL so an empty selection fails without touching the
    // configuration -- "nothing was selected" is the answer regardless of where
    // Relay lives, and a user who has not configured Relay yet should still be
    // told the more specific thing.
    let body = translation::body(request)?;
    let url = endpoint(relay_url)?;
    let answer = transport.send(&url, &body)?;
    if !(200..300).contains(&answer.status) {
        return Err(translation::status_problem(answer.status, &answer.body));
    }
    translation::parse(&answer.body, &request.target_language, &request.model)
}

/// One finished job: the answer and how long it took.
///
/// The duration is measured around the transport call only, so the log line's `ms`
/// is the network's cost and not the app's.
#[derive(Debug, Clone)]
pub struct Finished {
    pub result: Result<Reply, Problem>,
    pub elapsed: Duration,
}

/// Hands jobs to threads and collects the answers the panel still wants.
///
/// ## Why every answer carries an id
///
/// The panel is a transient window: the user presses the key, asks, and can press
/// Esc before the answer arrives. `accept` is the id of the one job whose answer is
/// still wanted, so a late answer from a closed reveal is dropped here rather than
/// shown on top of a different selection. Ids are never reused (`next` only grows),
/// because a reused id would make a stale answer indistinguishable from a fresh
/// one -- the exact bug this is here to prevent.
pub struct Engine {
    transport: Arc<dyn Transport>,
    tx: Sender<(u64, Finished)>,
    rx: Receiver<(u64, Finished)>,
    next: u64,
    accept: u64,
}

impl Engine {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        let (tx, rx) = channel();
        Self {
            transport,
            tx,
            rx,
            next: 0,
            accept: 0,
        }
    }

    /// Start a request on its own thread and return the id it will answer under.
    pub fn start(&mut self, relay_url: &str, request: Request) -> u64 {
        self.next += 1;
        let id = self.next;
        self.accept = id;
        let transport = self.transport.clone();
        let tx = self.tx.clone();
        let relay_url = relay_url.to_string();
        // A failed spawn would leave the panel spinning forever, so it is reported
        // as the same answer a refused connection gives: the user's action is the
        // same (try again), and there is no thread to reap.
        let spawned = std::thread::Builder::new()
            .name("translate".to_string())
            .spawn(move || {
                let at = Instant::now();
                let result = run(transport.as_ref(), &relay_url, &request);
                let finished = Finished {
                    result,
                    elapsed: at.elapsed(),
                };
                // Nobody listening means the process is going away; the send failing
                // is the normal way this thread ends.
                let _ = tx.send((id, finished));
            });
        if spawned.is_err() {
            let _ = self.tx.send((
                id,
                Finished {
                    result: Err(Problem::Offline),
                    elapsed: Duration::ZERO,
                },
            ));
        }
        id
    }

    /// Stop wanting the answer in flight.
    ///
    /// Called when the panel closes: the request itself keeps running (a blocking
    /// client cannot be cancelled), and its answer is dropped on arrival.
    pub fn cancel(&mut self) {
        self.accept = 0;
    }

    /// Take the newest answer, if one has arrived.
    ///
    /// Everything else in the channel is drained and dropped: answers to superseded
    /// jobs, and answers that arrived after the panel was closed. `self.tx` is held
    /// for the life of the engine, so the channel is never disconnected and
    /// `Disconnected` here means "nothing more to read".
    pub fn poll(&mut self) -> Option<Finished> {
        let mut newest = None;
        loop {
            match self.rx.try_recv() {
                Ok((id, finished)) => {
                    if id != 0 && id == self.accept {
                        newest = Some(finished);
                    }
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        newest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn request() -> Request {
        Request {
            model: "gpt-4o-mini".to_string(),
            target_language: "zh-CN".to_string(),
            text: "Hello, world.".to_string(),
        }
    }

    /// Answers with whatever it was built with, and records what it was asked.
    struct Fake {
        answer: Result<Answer, Problem>,
        seen: Mutex<Vec<(String, String)>>,
    }

    impl Fake {
        fn replying(status: u16, body: &str) -> Arc<Self> {
            Arc::new(Self {
                answer: Ok(Answer {
                    status,
                    body: body.to_string(),
                }),
                seen: Mutex::new(Vec::new()),
            })
        }

        fn failing(problem: Problem) -> Arc<Self> {
            Arc::new(Self {
                answer: Err(problem),
                seen: Mutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> usize {
            self.seen.lock().unwrap().len()
        }
    }

    impl Transport for Fake {
        fn send(&self, url: &str, body: &str) -> Result<Answer, Problem> {
            self.seen
                .lock()
                .unwrap()
                .push((url.to_string(), body.to_string()));
            self.answer.clone()
        }
    }

    const OK: &str = r#"{"choices":[{"message":{"role":"assistant","content":"你好，世界。"}}]}"#;

    #[test]
    fn the_base_url_is_normalised_onto_the_completion_path() {
        for (base, expected) in [
            ("http://127.0.0.1:8787", "http://127.0.0.1:8787/v1/chat/completions"),
            ("http://127.0.0.1:8787/", "http://127.0.0.1:8787/v1/chat/completions"),
            ("http://127.0.0.1:8787/v1", "http://127.0.0.1:8787/v1/chat/completions"),
            ("http://127.0.0.1:8787/v1/", "http://127.0.0.1:8787/v1/chat/completions"),
            ("https://relay.example.com/relay/v1", "https://relay.example.com/relay/v1/chat/completions"),
            ("https://relay.example.com/relay", "https://relay.example.com/relay/v1/chat/completions"),
            // Whitespace around a pasted value is a typo, not a different host.
            ("  http://127.0.0.1:8787  ", "http://127.0.0.1:8787/v1/chat/completions"),
        ] {
            assert_eq!(endpoint(base).as_deref(), Ok(expected), "base={base}");
        }
    }

    #[test]
    fn a_base_that_is_not_a_usable_http_url_is_refused_by_name() {
        for base in [
            "",
            "   ",
            "127.0.0.1:8787",          // parses as a scheme-less relative reference
            "ftp://127.0.0.1/v1",
            "http://",
            "http://user:pass@127.0.0.1:8787",
            "http://127.0.0.1:8787/v1?token=abc",
        ] {
            assert_eq!(endpoint(base), Err(Problem::InvalidUrl), "base={base}");
        }
    }

    #[test]
    fn a_good_answer_becomes_the_translation() {
        let fake = Fake::replying(200, OK);
        let reply = run(fake.as_ref(), "http://127.0.0.1:8787", &request()).unwrap();
        assert_eq!(reply.text, "你好，世界。");
        assert_eq!(reply.model, "gpt-4o-mini");

        let seen = fake.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "http://127.0.0.1:8787/v1/chat/completions");
        let body: serde_json::Value = serde_json::from_str(&seen[0].1).unwrap();
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][1]["content"], "Hello, world.");
    }

    #[test]
    fn a_refusal_is_reported_as_the_problem_it_represents() {
        for (status, body, problem) in [
            (401, "{}", Problem::Auth),
            (429, "{}", Problem::RateLimited),
            (404, "", Problem::Missing),
            (400, r#"{"error":{"message":"no relay route matched gpt-4o-mini"}}"#, Problem::NoRoute),
            // An upstream body can carry a key; it must not reach the sentence.
            (502, "gateway says: sk-live-abc", Problem::Unavailable),
        ] {
            let fake = Fake::replying(status, body);
            let problem_out = run(fake.as_ref(), "http://127.0.0.1:8787", &request()).unwrap_err();
            assert_eq!(problem_out, problem, "status={status}");
            assert!(!problem_out.message().contains("sk-live"));
        }
    }

    #[test]
    fn a_200_that_is_not_a_completion_is_not_reported_as_a_translation() {
        let fake = Fake::replying(200, r#"{"choices":[]}"#);
        assert_eq!(
            run(fake.as_ref(), "http://127.0.0.1:8787", &request()).unwrap_err(),
            Problem::Malformed
        );
    }

    #[test]
    fn a_transport_failure_passes_through_without_a_second_request() {
        let fake = Fake::failing(Problem::Timeout);
        assert_eq!(
            run(fake.as_ref(), "http://127.0.0.1:8787", &request()).unwrap_err(),
            Problem::Timeout
        );
        assert_eq!(fake.calls(), 1, "a timeout must not be retried on its own");
    }

    #[test]
    fn an_empty_selection_never_reaches_the_network() {
        let fake = Fake::replying(200, OK);
        let mut empty = request();
        empty.text = "  ".to_string();
        assert_eq!(
            run(fake.as_ref(), "http://127.0.0.1:8787", &empty).unwrap_err(),
            Problem::EmptyInput
        );
        assert_eq!(fake.calls(), 0, "nothing was asked, so nothing may be sent");
    }

    #[test]
    fn a_missing_model_stops_before_the_url_is_even_parsed() {
        // Deliberate order: "tell them to pick a model" is more useful than
        // "your Relay URL is broken" when both are wrong.
        let fake = Fake::replying(200, OK);
        let mut no_model = request();
        no_model.model = String::new();
        assert_eq!(
            run(fake.as_ref(), "not a url", &no_model).unwrap_err(),
            Problem::NoModel
        );
        assert_eq!(fake.calls(), 0);
    }

    /// Answers with the text it was asked to translate, so two jobs in flight can
    /// never be confused for one another by their bodies.
    struct Echo;

    impl Transport for Echo {
        fn send(&self, _url: &str, body: &str) -> Result<Answer, Problem> {
            let value: serde_json::Value = serde_json::from_str(body).unwrap();
            let text = value["messages"][1]["content"].as_str().unwrap();
            Ok(Answer {
                status: 200,
                body: format!(
                    r#"{{"choices":[{{"message":{{"role":"assistant","content":"译:{text}"}}}}]}}"#
                ),
            })
        }
    }

    fn asked(text: &str) -> Request {
        Request {
            text: text.to_string(),
            ..request()
        }
    }

    fn wait_for(engine: &mut Engine) -> Option<Finished> {
        // The worker is a real thread; a bounded spin is honest about that without
        // importing a timer abstraction for one test.
        for _ in 0..200 {
            if let Some(finished) = engine.poll() {
                return Some(finished);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn a_superseded_answer_is_dropped_rather_than_shown_over_the_newer_one() {
        let mut engine = Engine::new(Arc::new(Echo));
        engine.start("http://127.0.0.1:8787", asked("第一段"));
        engine.start("http://127.0.0.1:8787", asked("第二段"));
        let finished = wait_for(&mut engine).expect("the second job answers");
        let reply = finished.result.expect("a 200 with a completion is a reply");
        // Which body arrives first is up to the scheduler; only the one belonging
        // to the accepted id may come back.
        assert_eq!(reply.text, "译:第二段");
    }

    #[test]
    fn cancelling_drops_even_the_answer_to_the_newest_job() {
        let mut engine = Engine::new(Arc::new(Echo));
        engine.start("http://127.0.0.1:8787", asked("一段话"));
        engine.cancel();
        assert!(
            wait_for(&mut engine).is_none(),
            "a closed panel must not receive a translation it can no longer show"
        );
    }
}

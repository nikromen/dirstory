//! One persistent matcher thread, with obsolete queries canceled by generation.
use nucleo::{
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
    Config, Matcher, Utf32String,
};
use std::{
    cmp::Reverse,
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc,
    },
    thread::{self, JoinHandle},
};

struct Request {
    generation: u64,
    query: String,
}

struct ResultSet {
    generation: u64,
    visits: Vec<usize>,
}

pub(super) struct Search {
    requests: Option<Sender<Request>>,
    results: Receiver<ResultSet>,
    generation: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}

impl Search {
    pub(super) fn new(texts: Arc<Vec<String>>) -> io::Result<Self> {
        let (requests, incoming) = mpsc::channel::<Request>();
        let (outgoing, results) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let latest = generation.clone();

        let worker = thread::Builder::new()
            .name("dirstory-search".into())
            .spawn(move || {
                let candidates: Vec<Utf32String> =
                    texts.iter().map(|text| text.as_str().into()).collect();
                let mut matcher = Matcher::new(Config::DEFAULT.match_paths());

                while let Ok(mut request) = incoming.recv() {
                    // Coalesce queued edits before scoring the immutable candidates.
                    for newer in incoming.try_iter() {
                        request = newer;
                    }

                    let atom = Atom::new(
                        &request.query,
                        CaseMatching::Smart,
                        Normalization::Smart,
                        AtomKind::Fuzzy,
                        false,
                    );
                    let mut scored = Vec::new();

                    for (visit, candidate) in candidates.iter().enumerate() {
                        if latest.load(Ordering::Relaxed) != request.generation {
                            break;
                        }

                        if let Some(score) = atom.score(candidate.slice(..), &mut matcher) {
                            scored.push((Reverse(score), visit));
                        }
                    }

                    if latest.load(Ordering::Relaxed) != request.generation {
                        continue;
                    }

                    // Explicit chronological tie-breaking, independent of path length.
                    scored.sort_unstable();
                    let visits = scored.into_iter().map(|(_, visit)| visit).collect();
                    if outgoing
                        .send(ResultSet {
                            generation: request.generation,
                            visits,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;

        Ok(Self {
            requests: Some(requests),
            results,
            generation,
            worker: Some(worker),
        })
    }

    pub(super) fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn query(&self, query: &str) -> io::Result<()> {
        self.cancel();
        let generation = self.generation.load(Ordering::Relaxed);

        self.requests
            .as_ref()
            .unwrap()
            .send(Request {
                generation,
                query: query.into(),
            })
            .map_err(|_| io::Error::other("Search worker stopped"))
    }

    pub(super) fn receive(&self) -> io::Result<Option<Vec<usize>>> {
        loop {
            match self.results.try_recv() {
                Ok(result) if result.generation == self.generation.load(Ordering::Relaxed) => {
                    return Ok(Some(result.visits));
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::other("Search worker stopped"))
                }
            }
        }
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.cancel();
        self.requests.take();

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

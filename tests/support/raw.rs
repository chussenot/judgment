//! A raw HTTP/1.1 server driven by a script per connection, for what
//! wiremock cannot do: cut a body, stall mid-response, reset a connection
//! after the request arrived. A thread over a std listener, no dependency.
//!
//! Each accepted connection waits until the whole request (head and
//! `content-length` body) is in its receive buffer, records it, then runs
//! its script: connection `i` takes `scripts[i]`, the last script repeats.
//! A script that runs out closes cleanly (end of stream); [`Step::Reset`]
//! closes with the request still unread, which makes the kernel answer with
//! a TCP reset (the std way to get one without `SO_LINGER`).
//!
//! The accept loop and every stall poll a stop flag, set by
//! [`RawServer::stop`] or on drop, so a regression fails the test instead
//! of hanging it.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One thing a connection does after its request arrived.
#[derive(Debug, Clone)]
pub enum Step {
    /// Write these bytes, as they are.
    Write(Vec<u8>),
    /// Do nothing for this long, or until the server stops;
    /// `Duration::MAX` hangs until then.
    Stall(Duration),
    /// Close at once with the request unread: a TCP reset. Later steps are
    /// not run.
    Reset,
}

/// What one accepted connection received.
#[derive(Debug, Clone, Default)]
pub struct Connection {
    /// The request's bytes as far as they arrived.
    pub request: Vec<u8>,
    /// Whether the head and the whole declared body arrived.
    pub complete: bool,
}

pub struct RawServer {
    /// `http://127.0.0.1:PORT`.
    pub url: String,
    connections: Arc<Mutex<Vec<Connection>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RawServer {
    /// Serve `scripts`, one per connection in accept order, the last one
    /// repeated.
    pub fn start(scripts: Vec<Vec<Step>>) -> Self {
        assert!(!scripts.is_empty(), "a raw server needs a script");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let connections = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, done) = (Arc::clone(&connections), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            let mut handlers = Vec::new();
            while !done.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let index = {
                            let mut seen = seen.lock().unwrap();
                            seen.push(Connection::default());
                            seen.len() - 1
                        };
                        let script = scripts[index.min(scripts.len() - 1)].clone();
                        let (seen, done) = (Arc::clone(&seen), Arc::clone(&done));
                        handlers.push(std::thread::spawn(move || {
                            serve(stream, &script, &seen, index, &done);
                        }));
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        Self {
            url,
            connections,
            stop,
            thread: Some(thread),
        }
    }

    /// Every connection accepted so far, in order.
    pub fn connections(&self) -> Vec<Connection> {
        self.connections.lock().unwrap().clone()
    }

    /// Stop accepting, end every stall, wait for the threads, and return
    /// every connection accepted.
    pub fn stop(mut self) -> Vec<Connection> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.connections()
    }
}

impl Drop for RawServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn serve(
    stream: TcpStream,
    script: &[Step],
    seen: &Mutex<Vec<Connection>>,
    index: usize,
    stop: &AtomicBool,
) {
    let mut stream = stream;
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let (request, complete) = peek_request(&stream, stop);
    let length = request.len();
    seen.lock().unwrap()[index] = Connection { request, complete };
    for step in script {
        match step {
            Step::Write(bytes) => {
                if stream
                    .write_all(bytes)
                    .and_then(|()| stream.flush())
                    .is_err()
                {
                    return;
                }
            }
            Step::Stall(wait) => {
                let until = Instant::now().checked_add(*wait);
                while until.is_none_or(|t| Instant::now() < t) && !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            // Dropping a socket with unread bytes in its receive buffer
            // sends a reset rather than a FIN.
            Step::Reset => return,
        }
    }
    // Consume what was peeked, so the close below is a clean end of stream.
    let _ = stream.read_exact(&mut vec![0; length]);
    let _ = stream.shutdown(Shutdown::Both);
}

/// The request as it sits in the receive buffer, read with `peek` so it
/// stays there for [`Step::Reset`], and whether it is whole. Gives up when
/// the peer closes or the server stops.
fn peek_request(stream: &TcpStream, stop: &AtomicBool) -> (Vec<u8>, bool) {
    let mut buf = vec![0; 1 << 20];
    let mut seen = 0;
    while !stop.load(Ordering::SeqCst) {
        match stream.peek(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                seen = n;
                if let Some(total) = request_length(&buf[..n])
                    && n >= total
                {
                    return (buf[..total].to_vec(), true);
                }
                // `peek` returns at once while bytes are waiting.
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    (buf[..seen].to_vec(), false)
}

/// The head's length plus its `content-length`, once the head is complete.
fn request_length(buf: &[u8]) -> Option<usize> {
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
    let body = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    Some(end + 4 + body)
}

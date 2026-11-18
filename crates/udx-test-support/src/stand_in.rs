//! The recording HTTP stand-in and the test certificate (A 3.14).
//!
//! T7 fills these in. Until then they follow A 3's rule "Stubs never panic":
//! a stand-in made here binds no socket, serves nothing and records nothing,
//! and no key material exists.

/// What a route of the stand-in answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandInResponse {
    Body {
        status: u16,
        content_type: String,
        body: Vec<u8>,
    },
    Redirect {
        status: u16,
        location: String,
    },
    Slow {
        bytes_per_s: u32,
        body: Vec<u8>,
    },
    Gzip {
        body: Vec<u8>,
    },
    Huge {
        bytes: u64,
    },
}

/// One path and its scripted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandInRoute {
    pub path: String,
    pub response: StandInResponse,
}

/// A request the stand-in received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A recording HTTP/1.1 stand-in on 127.0.0.1 (stub until T7: inert).
#[derive(Debug)]
pub struct HttpStandIn {
    tls: bool,
}

impl HttpStandIn {
    /// A stand-in that serves and records nothing (T7 builds the real one).
    pub async fn start(routes: Vec<StandInRoute>) -> HttpStandIn {
        let _ = routes;
        HttpStandIn { tls: false }
    }

    /// The same over TLS (T7). Inert until then.
    pub async fn start_tls(routes: Vec<StandInRoute>, cert: &TestCert) -> HttpStandIn {
        let _ = (routes, cert);
        HttpStandIn { tls: true }
    }

    /// A loopback URL that no listener serves (port 0 accepts no connection).
    pub fn url(&self, path: &str) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://127.0.0.1:0/{}", path.trim_start_matches('/'))
    }

    /// Every request received: none, until T7.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        Vec::new()
    }
}

/// A test certificate authority and leaf (stub until T7: inert, no key material).
#[derive(Debug)]
pub struct TestCert {
    _inert: (),
}

impl TestCert {
    /// An inert value; T7 generates the real certificates.
    pub fn generate() -> TestCert {
        TestCert { _inert: () }
    }

    /// A fixed placeholder naming T7: not a certificate.
    pub fn ca_pem(&self) -> String {
        "placeholder: the test certificate authority is built by T7".to_string()
    }
}

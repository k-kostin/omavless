// SPDX-License-Identifier: MIT

//! Bounded credential-private HTTP transport for subscription feeds.
//!
//! The request method and headers are fixed. Every initial and redirected URL
//! passes the canonical subscription validator. Redirects are followed
//! manually so HTTPS can never silently downgrade to non-loopback HTTP and no
//! authorization/cookie state can cross origins. Ambient proxy credentials are
//! deliberately ignored until an explicit semantic proxy contract exists.

use omavless_domain::import::valid_subscription_url;
use omavless_domain::subscription_feed::{
    MAX_SUBSCRIPTION_FEED_BYTES, PrivateSubscriptionBody, SubscriptionFeedError,
};
use omavless_domain::subscription_metadata::{
    SubscriptionUsage, parse_final_subscription_userinfo,
};
use std::fmt;
use std::time::{Duration, Instant};
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;

pub const SUBSCRIPTION_TIMEOUT: Duration = Duration::from_secs(25);
pub const MAX_SUBSCRIPTION_REDIRECTS: u32 = 5;
pub const MAX_SUBSCRIPTION_RESPONSE_HEADER_BYTES: usize = 32 * 1024;
const USER_AGENT: &str = concat!("OmaVLESS/", env!("CARGO_PKG_VERSION"));
const ACCEPT: &str = "text/plain, application/octet-stream;q=0.9, */*;q=0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionTransportError {
    InvalidUrl,
    Redirect,
    TooManyRedirects,
    Timeout,
    Tls,
    HttpStatus,
    TooLarge,
    Unavailable,
}

impl SubscriptionTransportError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidUrl => "subscription_transport_invalid_url",
            Self::Redirect => "subscription_transport_redirect_invalid",
            Self::TooManyRedirects => "subscription_transport_redirect_limit",
            Self::Timeout => "subscription_transport_timeout",
            Self::Tls => "subscription_transport_tls",
            Self::HttpStatus => "subscription_transport_http_status",
            Self::TooLarge => "subscription_feed_too_large",
            Self::Unavailable => "subscription_transport_unavailable",
        }
    }
}

impl fmt::Display for SubscriptionTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUrl => "Subscription URL is invalid",
            Self::Redirect => "Subscription redirect is invalid",
            Self::TooManyRedirects => "Subscription redirect limit was exceeded",
            Self::Timeout => "Subscription request timed out",
            Self::Tls => "Subscription TLS verification failed",
            Self::HttpStatus => "Subscription provider rejected the request",
            Self::TooLarge => "Subscription response is too large",
            Self::Unavailable => "Subscription transport is unavailable",
        })
    }
}

impl std::error::Error for SubscriptionTransportError {}

pub trait SubscriptionTransport {
    fn fetch(&self, url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError>;
}

/// Private optional provider metadata from the final accepted response only.
/// No Debug/Serialize: usage values may identify an account. This result is
/// not consumed by ordinary refresh/store; only explicit private usage reads.
pub struct FetchedSubscription {
    pub body: PrivateSubscriptionBody,
    pub usage: Option<SubscriptionUsage>,
}

pub struct HttpsSubscriptionTransport {
    agent: Agent,
    timeout: Duration,
    max_body_bytes: usize,
    max_redirects: u32,
}

impl HttpsSubscriptionTransport {
    #[must_use]
    pub fn new() -> Self {
        Self::with_bounds(
            SUBSCRIPTION_TIMEOUT,
            MAX_SUBSCRIPTION_FEED_BYTES,
            MAX_SUBSCRIPTION_REDIRECTS,
        )
    }

    fn with_bounds(timeout: Duration, max_body_bytes: usize, max_redirects: u32) -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .max_response_header_size(MAX_SUBSCRIPTION_RESPONSE_HEADER_BYTES)
            .user_agent(USER_AGENT)
            .accept(ACCEPT)
            .accept_encoding("identity")
            .proxy(None)
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        Self {
            agent: config.new_agent(),
            timeout,
            max_body_bytes,
            max_redirects,
        }
    }

    fn parse_url(value: &str) -> Result<Url, SubscriptionTransportError> {
        if !valid_subscription_url(value) {
            return Err(SubscriptionTransportError::InvalidUrl);
        }
        let parsed = Url::parse(value).map_err(|_| SubscriptionTransportError::InvalidUrl)?;
        if parsed.as_str().len() > omavless_domain::import::MAX_SUBSCRIPTION_URL_BYTES {
            return Err(SubscriptionTransportError::InvalidUrl);
        }
        Ok(parsed)
    }

    fn redirect_url(current: &Url, location: &str) -> Result<Url, SubscriptionTransportError> {
        let redirected = current
            .join(location)
            .map_err(|_| SubscriptionTransportError::Redirect)?;
        if !valid_subscription_url(redirected.as_str()) {
            return Err(SubscriptionTransportError::Redirect);
        }
        Ok(redirected)
    }

    fn map_transport_error(error: ureq::Error) -> SubscriptionTransportError {
        match error {
            ureq::Error::Timeout(_) => SubscriptionTransportError::Timeout,
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::Pem(_) => {
                SubscriptionTransportError::Tls
            }
            ureq::Error::BodyExceedsLimit(_) => SubscriptionTransportError::TooLarge,
            _ => SubscriptionTransportError::Unavailable,
        }
    }

    fn fetch_url(
        &self,
        initial: &str,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.fetch_with_budget(initial, self.timeout)
    }

    /// Use the smaller of the provider limit and the enclosing job's remaining
    /// time. The budget covers redirects and body consumption together. It is
    /// trusted runtime input, never a caller-selectable IPC timeout.
    pub fn fetch_with_budget(
        &self,
        initial: &str,
        remaining_job_time: Duration,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.fetch_with_usage_with_budget(initial, remaining_job_time)
            .map(|fetched| fetched.body)
    }

    /// Private read seam. Current live refresh still consumes only
    /// the body through `fetch_with_budget` and never persists usage metadata.
    pub fn fetch_with_usage_with_budget(
        &self,
        initial: &str,
        remaining_job_time: Duration,
    ) -> Result<FetchedSubscription, SubscriptionTransportError> {
        let started = Instant::now();
        self.fetch_with_usage_with_elapsed(initial, remaining_job_time, || started.elapsed())
    }

    #[cfg(test)]
    fn fetch_with_elapsed<F>(
        &self,
        initial: &str,
        remaining_job_time: Duration,
        mut elapsed: F,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError>
    where
        F: FnMut() -> Duration,
    {
        self.fetch_with_usage_with_elapsed(initial, remaining_job_time, &mut elapsed)
            .map(|fetched| fetched.body)
    }

    fn fetch_with_usage_with_elapsed<F>(
        &self,
        initial: &str,
        remaining_job_time: Duration,
        mut elapsed: F,
    ) -> Result<FetchedSubscription, SubscriptionTransportError>
    where
        F: FnMut() -> Duration,
    {
        let budget = self.timeout.min(remaining_job_time);
        let remaining = |elapsed| {
            budget
                .checked_sub(elapsed)
                .filter(|left| !left.is_zero())
                .ok_or(SubscriptionTransportError::Timeout)
        };
        remaining(elapsed())?;
        let mut current = Self::parse_url(initial)?;
        for redirect_count in 0..=self.max_redirects {
            let request_budget = remaining(elapsed())?;
            let mut response = self
                .agent
                .get(current.as_str())
                .config()
                .timeout_global(Some(request_budget))
                .build()
                .call()
                .map_err(Self::map_transport_error)?;
            remaining(elapsed())?;
            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                if redirect_count == self.max_redirects {
                    return Err(SubscriptionTransportError::TooManyRedirects);
                }
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok())
                    .ok_or(SubscriptionTransportError::Redirect)?;
                current = Self::redirect_url(&current, location)?;
                continue;
            }
            if !(200..300).contains(&status) {
                return Err(SubscriptionTransportError::HttpStatus);
            }
            if response
                .headers()
                .get("content-length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|length| length > self.max_body_bytes as u64)
            {
                return Err(SubscriptionTransportError::TooLarge);
            }
            // Invalid/ambiguous optional provider metadata never breaks a
            // usable feed. An invalid UTF-8 usage value is not silently
            // omitted when another duplicate value is present.
            let usage = if response
                .headers()
                .get_all("subscription-userinfo")
                .iter()
                .any(|value| value.to_str().is_err())
            {
                None
            } else {
                parse_final_subscription_userinfo(
                    status,
                    response.headers().iter().filter_map(|(name, value)| {
                        value.to_str().ok().map(|text| (name.as_str(), text))
                    }),
                )
                .ok()
                .flatten()
            };
            let bytes = response
                .body_mut()
                .with_config()
                .limit(u64::try_from(self.max_body_bytes.saturating_add(1)).unwrap_or(u64::MAX))
                .read_to_vec()
                .map_err(Self::map_transport_error)?;
            remaining(elapsed())?;
            if bytes.len() > self.max_body_bytes {
                return Err(SubscriptionTransportError::TooLarge);
            }
            let body = PrivateSubscriptionBody::from_bytes(bytes).map_err(|error| match error {
                SubscriptionFeedError::TooLarge => SubscriptionTransportError::TooLarge,
                _ => SubscriptionTransportError::Unavailable,
            })?;
            return Ok(FetchedSubscription { body, usage });
        }
        Err(SubscriptionTransportError::TooManyRedirects)
    }
}

impl Default for HttpsSubscriptionTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionTransport for HttpsSubscriptionTransport {
    fn fetch(&self, url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.fetch_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_domain::subscription_feed::decode_subscription_feed;
    use omavless_domain::subscription_metadata::{
        SubscriptionMetadataError, parse_final_subscription_userinfo,
    };
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    const PROFILE: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 512];
        while bytes.len() < 8192 && !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn server(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                stream.write_all(&response).unwrap();
            }
            requests
        });
        (format!("http://{address}/feed"), worker)
    }

    fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut value = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\n{headers}Content-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        value.extend_from_slice(body);
        value
    }

    fn fetch_error(
        result: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
    ) -> SubscriptionTransportError {
        match result {
            Err(error) => error,
            Ok(_) => panic!("subscription fetch unexpectedly succeeded"),
        }
    }

    #[test]
    fn fixed_get_fetches_one_bounded_private_feed() {
        let (url, worker) = server(vec![response("200 OK", "", PROFILE.as_bytes())]);
        let transport = HttpsSubscriptionTransport::with_bounds(
            Duration::from_secs(2),
            MAX_SUBSCRIPTION_FEED_BYTES,
            MAX_SUBSCRIPTION_REDIRECTS,
        );
        let body = transport.fetch(&url).unwrap();
        assert_eq!(decode_subscription_feed(body).unwrap().counts().accepted, 1);
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        let request = requests[0].to_ascii_lowercase();
        assert!(request.starts_with("get /feed http/1.1\r\n"));
        assert!(request.contains("accept-encoding: identity\r\n"));
        assert!(request.contains("user-agent: omavless/"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("cookie:"));
    }

    #[test]
    fn relative_redirect_is_revalidated_and_bounded() {
        let (url, worker) = server(vec![
            response("302 Found", "Location: /next\r\n", b""),
            response("200 OK", "", PROFILE.as_bytes()),
        ]);
        let transport = HttpsSubscriptionTransport::with_bounds(
            Duration::from_secs(2),
            MAX_SUBSCRIPTION_FEED_BYTES,
            1,
        );
        let body = transport.fetch(&url).unwrap();
        assert_eq!(decode_subscription_feed(body).unwrap().counts().accepted, 1);
        let requests = worker.join().unwrap();
        assert!(requests[1].starts_with("GET /next HTTP/1.1\r\n"));
    }

    #[test]
    fn ureq_preserves_duplicate_usage_header_fields_for_future_admission() {
        let (url, worker) = server(vec![response(
            "200 OK",
            "Subscription-Userinfo: upload=1;download=2;total=3\r\nsubscription-userinfo: upload=4;download=5;total=6\r\n",
            PROFILE.as_bytes(),
        )]);
        let transport = HttpsSubscriptionTransport::new();
        let response = transport.agent.get(&url).call().unwrap();
        let values: Vec<_> = response
            .headers()
            .get_all("subscription-userinfo")
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect();
        assert_eq!(
            values.len(),
            2,
            "ureq folded or dropped a duplicate usage header"
        );
        assert_eq!(values[0], "upload=1;download=2;total=3");
        assert_eq!(values[1], "upload=4;download=5;total=6");
        assert!(matches!(
            parse_final_subscription_userinfo(
                response.status().as_u16(),
                response
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.to_str().unwrap()))
            ),
            Err(SubscriptionMetadataError::DuplicateHeader)
        ));
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn only_final_redirect_response_supplies_optional_usage() {
        let (url, worker) = server(vec![
            response(
                "302 Found",
                "Location: /next\r\nSubscription-Userinfo: upload=9;download=9;total=9\r\n",
                b"",
            ),
            response(
                "200 OK",
                "Subscription-Userinfo: upload=1;download=2;total=3\r\n",
                PROFILE.as_bytes(),
            ),
        ]);
        let transport = HttpsSubscriptionTransport::new();
        let redirected = transport.agent.get(&url).call().unwrap();
        assert_eq!(redirected.status().as_u16(), 302);
        assert!(
            parse_final_subscription_userinfo(
                redirected.status().as_u16(),
                redirected
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.to_str().unwrap()))
            )
            .unwrap()
            .is_none()
        );

        let location = redirected
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        let final_url =
            HttpsSubscriptionTransport::redirect_url(&Url::parse(&url).unwrap(), location).unwrap();
        let final_response = transport.agent.get(final_url.as_str()).call().unwrap();
        let final_usage = parse_final_subscription_userinfo(
            final_response.status().as_u16(),
            final_response
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str(), value.to_str().unwrap())),
        )
        .unwrap()
        .unwrap();
        assert_eq!(final_usage.upload_bytes, 1);
        assert_eq!(final_usage.download_bytes, 2);
        assert_eq!(final_usage.total_bytes, 3);
        assert_eq!(worker.join().unwrap().len(), 2);
    }

    #[test]
    fn actual_fetch_extracts_only_final_usage_without_an_extra_request() {
        let (url, worker) = server(vec![
            response(
                "302 Found",
                "Location: /next\r\nSubscription-Userinfo: upload=99;download=99;total=99\r\n",
                b"",
            ),
            response(
                "200 OK",
                "Subscription-Userinfo: upload=1;download=2;total=3\r\n",
                PROFILE.as_bytes(),
            ),
        ]);
        let fetched = HttpsSubscriptionTransport::new()
            .fetch_with_usage_with_budget(&url, Duration::from_secs(3))
            .unwrap();
        assert_eq!(
            decode_subscription_feed(fetched.body)
                .unwrap()
                .counts()
                .accepted,
            1
        );
        let usage = fetched.usage.unwrap();
        assert_eq!(
            (usage.upload_bytes, usage.download_bytes, usage.total_bytes),
            (1, 2, 3)
        );
        assert_eq!(worker.join().unwrap().len(), 2);
    }

    #[test]
    fn missing_invalid_and_duplicate_usage_keep_the_body_usable() {
        for header in [
            "",
            "Subscription-Userinfo: upload=bad;download=2;total=3\r\n",
            "Subscription-Userinfo: upload=1;download=2;total=3\r\nsubscription-userinfo: upload=4;download=5;total=6\r\n",
        ] {
            let (url, worker) = server(vec![response("200 OK", header, PROFILE.as_bytes())]);
            let fetched = HttpsSubscriptionTransport::new()
                .fetch_with_usage_with_budget(&url, Duration::from_secs(3))
                .unwrap();
            assert!(fetched.usage.is_none());
            assert_eq!(
                decode_subscription_feed(fetched.body)
                    .unwrap()
                    .counts()
                    .accepted,
                1
            );
            assert_eq!(worker.join().unwrap().len(), 1);
        }
    }

    #[test]
    fn non_utf8_usage_value_cannot_be_discarded_in_favor_of_valid_duplicate() {
        let mut raw = response(
            "200 OK",
            "Subscription-Userinfo: x\r\nSubscription-Userinfo: upload=1;download=2;total=3\r\n",
            PROFILE.as_bytes(),
        );
        let marker = b"Subscription-Userinfo: x\r\n";
        let index = raw
            .windows(marker.len())
            .position(|part| part == marker)
            .unwrap();
        raw[index + marker.len() - 3] = 0xff;
        let (url, worker) = server(vec![raw]);
        let fetched = HttpsSubscriptionTransport::new()
            .fetch_with_usage_with_budget(&url, Duration::from_secs(3))
            .unwrap();
        assert!(fetched.usage.is_none());
        assert_eq!(
            decode_subscription_feed(fetched.body)
                .unwrap()
                .counts()
                .accepted,
            1
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn body_only_caller_remains_successful_with_valid_usage_header() {
        let (url, worker) = server(vec![response(
            "200 OK",
            "Subscription-Userinfo: upload=1;download=2;total=3\r\n",
            PROFILE.as_bytes(),
        )]);
        let body = HttpsSubscriptionTransport::new()
            .fetch_with_budget(&url, Duration::from_secs(3))
            .unwrap();
        assert_eq!(decode_subscription_feed(body).unwrap().counts().accepted, 1);
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn unusable_optional_usage_does_not_fail_a_usable_feed() {
        let headers = "Subscription-Userinfo: upload=invalid;download=2;total=3\r\n";
        let (url, worker) = server(vec![
            response("200 OK", headers, PROFILE.as_bytes()),
            response("200 OK", headers, PROFILE.as_bytes()),
        ]);
        let transport = HttpsSubscriptionTransport::new();
        let response = transport.agent.get(&url).call().unwrap();
        assert!(matches!(
            parse_final_subscription_userinfo(
                response.status().as_u16(),
                response
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.to_str().unwrap()))
            ),
            Err(SubscriptionMetadataError::InvalidNumber)
        ));
        // Production transport currently ignores optional usage metadata. A
        // future caller must preserve this fail-open feed behavior.
        let body = transport.fetch(&url).unwrap();
        assert_eq!(decode_subscription_feed(body).unwrap().counts().accepted, 1);
        assert_eq!(worker.join().unwrap().len(), 2);
    }

    #[test]
    fn invalid_redirect_is_rejected_without_contacting_the_target() {
        let (url, worker) = server(vec![response(
            "302 Found",
            "Location: http://example.invalid/private\r\n",
            b"",
        )]);
        let error = fetch_error(HttpsSubscriptionTransport::new().fetch(&url));
        assert_eq!(error, SubscriptionTransportError::Redirect);
        worker.join().unwrap();
    }

    #[test]
    fn redirect_and_body_limits_fail_closed() {
        let (url, worker) = server(vec![response("302 Found", "Location: /again\r\n", b"")]);
        assert_eq!(
            fetch_error(
                HttpsSubscriptionTransport::with_bounds(Duration::from_secs(2), 64, 0).fetch(&url)
            ),
            SubscriptionTransportError::TooManyRedirects
        );
        worker.join().unwrap();

        let (url, worker) = server(vec![response("200 OK", "", &[b'x'; 65])]);
        assert_eq!(
            fetch_error(
                HttpsSubscriptionTransport::with_bounds(Duration::from_secs(2), 64, 0).fetch(&url)
            ),
            SubscriptionTransportError::TooLarge
        );
        worker.join().unwrap();
    }

    #[test]
    fn timeout_status_and_private_inputs_map_to_fixed_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/private-token", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            thread::sleep(Duration::from_millis(150));
        });
        let error = fetch_error(
            HttpsSubscriptionTransport::with_bounds(Duration::from_millis(40), 64, 0).fetch(&url),
        );
        assert_eq!(error, SubscriptionTransportError::Timeout);
        worker.join().unwrap();

        let (url, worker) = server(vec![response("403 Forbidden", "", b"private")]);
        let error = fetch_error(HttpsSubscriptionTransport::new().fetch(&url));
        assert_eq!(error, SubscriptionTransportError::HttpStatus);
        worker.join().unwrap();
        let public = format!("{error:?} {error}");
        assert!(!public.contains("private-token"));
        assert!(!public.contains(&url));
    }

    #[test]
    fn redirect_chain_cannot_reset_the_total_budget() {
        let (url, worker) = server(vec![response("302 Found", "Location: /next\r\n", b"")]);
        let transport = HttpsSubscriptionTransport::with_bounds(
            Duration::from_secs(2),
            MAX_SUBSCRIPTION_FEED_BYTES,
            5,
        );
        let mut times = [Duration::ZERO, Duration::ZERO, Duration::from_secs(2)].into_iter();
        let error = fetch_error(
            transport.fetch_with_elapsed(&url, Duration::from_secs(9), || {
                times.next().expect("no work after deadline")
            }),
        );
        assert_eq!(error, SubscriptionTransportError::Timeout);
        assert_eq!(worker.join().unwrap().len(), 1);
        assert!(times.next().is_none());
    }

    #[test]
    fn body_completion_after_budget_is_not_accepted() {
        let (url, worker) = server(vec![response("200 OK", "", PROFILE.as_bytes())]);
        let transport = HttpsSubscriptionTransport::new();
        let mut times = [
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(1),
        ]
        .into_iter();
        assert_eq!(
            fetch_error(
                transport.fetch_with_elapsed(&url, Duration::from_secs(1), || times
                    .next()
                    .expect("no work after body deadline"),)
            ),
            SubscriptionTransportError::Timeout
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn zero_job_budget_performs_no_network_io() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/private", listener.local_addr().unwrap());
        assert_eq!(
            fetch_error(HttpsSubscriptionTransport::new().fetch_with_budget(&url, Duration::ZERO)),
            SubscriptionTransportError::Timeout
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn remaining_job_time_caps_the_actual_request_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/private", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            thread::sleep(Duration::from_millis(150));
            let _ = stream.write_all(&response("200 OK", "", PROFILE.as_bytes()));
        });
        assert_eq!(
            fetch_error(
                HttpsSubscriptionTransport::new()
                    .fetch_with_budget(&url, Duration::from_millis(40),)
            ),
            SubscriptionTransportError::Timeout
        );
        worker.join().unwrap();
    }

    #[test]
    fn non_loopback_http_userinfo_fragments_and_oversize_fail_before_io() {
        for value in [
            "http://example.invalid/feed",
            "https://user:secret@example.invalid/feed",
            "https://example.invalid/feed#private",
        ] {
            assert_eq!(
                fetch_error(HttpsSubscriptionTransport::new().fetch(value)),
                SubscriptionTransportError::InvalidUrl
            );
        }
    }
}

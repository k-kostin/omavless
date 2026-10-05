// SPDX-License-Identifier: MIT
//! Optional fixed client, not installed/registered or native K1 readiness.
//! Requires the admitted canonical user/mount/network namespace and trusted
//! root/package custody. Path/peer equality is not creator or namespace proof.
//! SO_PEERCRED is a connection/listen-time snapshot, NOT process liveness.
//! Read-only builder temporaries may close on failure; only actually returned
//! original socket/assembled graph owners are retained on uncertain cuts.
use crate::protocol::{self, Health, Mode, Protection, Request, Response};
use std::time::{Duration, Instant};

const PATH: &str = "/run/omavless-netguard/control.sock";
const BUDGET: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    UnavailableOrUnknown,
}
type Result<T> = std::result::Result<T, ClientError>;
const REFUSE: ClientError = ClientError::UnavailableOrUnknown;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Peer {
    pid: i32,
    uid: u32,
    gid: u32,
}
enum Connect {
    Complete,
    Pending,
}
enum Progress {
    Bytes(usize),
    WouldBlock,
}
#[derive(Clone, Copy)]
enum Interest {
    Read,
    Write,
}

trait Backend {
    type Endpoint;
    type Socket;
    fn now(&self) -> Instant;
    fn admit(&mut self) -> Result<Self::Endpoint>;
    fn recheck(&mut self, endpoint: &Self::Endpoint) -> Result<()>;
    fn create(&mut self) -> Result<Self::Socket>;
    fn connect(&mut self, socket: &Self::Socket) -> Result<Connect>;
    fn connected(&mut self, socket: &Self::Socket) -> Result<()>;
    fn wait(
        &mut self,
        socket: &Self::Socket,
        interest: Interest,
        remaining: Duration,
    ) -> Result<()>;
    fn peer(&mut self, socket: &Self::Socket) -> Result<Peer>;
    fn write(&mut self, socket: &Self::Socket, bytes: &[u8]) -> Result<Progress>;
    fn read(&mut self, socket: &Self::Socket, bytes: &mut [u8]) -> Result<Progress>;
}
struct Original<B: Backend> {
    backend: B,
    endpoint: Option<B::Endpoint>,
    socket: Option<B::Socket>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Fresh,
    InFlight,
    Complete,
    Poisoned,
}
struct Client<B: Backend> {
    original: Option<Original<B>>,
    phase: Phase,
    peer: Option<Peer>,
}
impl<B: Backend> Drop for Client<B> {
    fn drop(&mut self) {
        if matches!(self.phase, Phase::InFlight | Phase::Poisoned)
            && let Some(original) = self.original.take()
        {
            std::mem::forget(original);
        }
    }
}
fn check<B: Backend>(g: &Original<B>, deadline: Instant) -> Result<()> {
    if g.backend.now() >= deadline {
        Err(REFUSE)
    } else {
        Ok(())
    }
}
fn recheck<B: Backend>(g: &mut Original<B>, deadline: Instant) -> Result<()> {
    check(g, deadline)?;
    g.backend.recheck(g.endpoint.as_ref().ok_or(REFUSE)?)?;
    check(g, deadline)
}
fn peer<B: Backend>(
    g: &mut Original<B>,
    deadline: Instant,
    expected: Option<Peer>,
) -> Result<Peer> {
    check(g, deadline)?;
    let actual = g.backend.peer(g.socket.as_ref().ok_or(REFUSE)?)?;
    check(g, deadline)?;
    // Matches actual supported unit User=root/Group=root, not package ACL GID.
    if actual.uid != 0
        || actual.gid != 0
        || actual.pid <= 0
        || expected.is_some_and(|p| p != actual)
    {
        return Err(REFUSE);
    }
    Ok(actual)
}
fn transfer<B: Backend>(
    g: &mut Original<B>,
    deadline: Instant,
    bytes: &mut [u8],
    writing: bool,
) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        check(g, deadline)?;
        let socket = g.socket.as_ref().ok_or(REFUSE)?;
        let progress = if writing {
            g.backend.write(socket, &bytes[offset..])
        } else {
            g.backend.read(socket, &mut bytes[offset..])
        }?;
        check(g, deadline)?; // positive-late progress never retires the owner
        match progress {
            Progress::Bytes(n) if n > 0 && n <= bytes.len() - offset => offset += n,
            Progress::Bytes(_) => return Err(REFUSE), // EOF/short-zero is not success
            Progress::WouldBlock => {
                let remaining = deadline
                    .checked_duration_since(g.backend.now())
                    .ok_or(REFUSE)?;
                g.backend.wait(
                    socket,
                    if writing {
                        Interest::Write
                    } else {
                        Interest::Read
                    },
                    remaining,
                )?;
                check(g, deadline)?;
            }
        }
    }
    Ok(())
}
fn positive(request: Request, response: Response) -> bool {
    let Response::Status {
        policy_version: protocol::POLICY_VERSION,
        protection,
        health: Health::Verified,
    } = response
    else {
        return false;
    };
    match request {
        Request::Status {} => true,
        Request::Arm {
            generation,
            mode: Mode::Full,
        } => protection == Protection::Armed { generation },
        Request::Disarm { generation } => {
            protection
                == Protection::Disarmed {
                    closed_generation: Some(generation),
                }
        }
    }
}
impl<B: Backend> Client<B> {
    fn new(backend: B) -> Self {
        Self {
            original: Some(Original {
                backend,
                endpoint: None,
                socket: None,
            }),
            phase: Phase::Fresh,
            peer: None,
        }
    }
    fn exchange(&mut self, request: Request) -> Result<Response> {
        if !matches!(self.phase, Phase::Fresh | Phase::Complete) {
            self.phase = Phase::Poisoned;
            return Err(REFUSE);
        }
        if self.peer.is_none() && request != (Request::Status {}) {
            self.phase = Phase::Poisoned;
            return Err(REFUSE);
        }
        self.phase = Phase::InFlight; // consumed BEFORE every external callback
        let result = self.perform(request);
        self.phase = if result.is_ok() {
            Phase::Complete
        } else {
            Phase::Poisoned
        };
        result
    }
    fn perform(&mut self, request: Request) -> Result<Response> {
        let g = self.original.as_mut().ok_or(REFUSE)?;
        // A previous stream retires ONLY after a healthy request-specific reply
        // and ALL postgates completed. No second allocation while it is held.
        drop(g.socket.take());
        let deadline = g.backend.now().checked_add(BUDGET).ok_or(REFUSE)?;
        let body = protocol::encode_request(request).map_err(|_| REFUSE)?;
        check(g, deadline)?;
        if g.endpoint.is_none() {
            g.endpoint = Some(g.backend.admit()?);
            check(g, deadline)?;
        }
        recheck(g, deadline)?;
        check(g, deadline)?;
        // Positive ownership is stored BEFORE the post-create time/check cut.
        g.socket = Some(g.backend.create()?);
        check(g, deadline)?;
        match g.backend.connect(g.socket.as_ref().ok_or(REFUSE)?)? {
            Connect::Complete => {}
            Connect::Pending => {
                check(g, deadline)?;
                let remaining = deadline
                    .checked_duration_since(g.backend.now())
                    .ok_or(REFUSE)?;
                g.backend
                    .wait(g.socket.as_ref().ok_or(REFUSE)?, Interest::Write, remaining)?;
            }
        }
        check(g, deadline)?;
        g.backend.connected(g.socket.as_ref().ok_or(REFUSE)?)?;
        check(g, deadline)?;
        let first_peer = peer(g, deadline, self.peer)?;
        recheck(g, deadline)?;
        let mut wire = (body.len() as u32).to_be_bytes().to_vec();
        wire.extend(body);
        transfer(g, deadline, &mut wire, true)?;
        let mut prefix = [0; 4];
        transfer(g, deadline, &mut prefix, false)?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length == 0 || length > protocol::MAX_FRAME_BYTES {
            return Err(REFUSE);
        }
        let mut bytes = vec![0; length];
        transfer(g, deadline, &mut bytes, false)?;
        check(g, deadline)?;
        let response = protocol::decode_response(&bytes).map_err(|_| REFUSE)?;
        check(g, deadline)?;
        if !positive(request, response) {
            return Err(REFUSE);
        }
        recheck(g, deadline)?;
        peer(g, deadline, Some(first_peer))?;
        check(g, deadline)?;
        self.peer = Some(first_peer);
        // Keep this positively completed socket owned through return. Only the
        // NEXT distinct operation (or normal known-complete Drop) retires it.
        Ok(response)
    }
}

mod linux;
/// Lazy fixed endpoint: constructor performs NO filesystem/network operation.
/// This API is optional SOURCE only; there is no product owner/factory caller.
pub struct FixedClient {
    client: Client<linux::Linux>,
}
impl Default for FixedClient {
    fn default() -> Self {
        Self::new()
    }
}
impl FixedClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(linux::Linux),
        }
    }
    pub fn exchange(&mut self, request: Request) -> Result<Response> {
        self.client.exchange(request)
    }
}
#[cfg(test)]
mod tests;

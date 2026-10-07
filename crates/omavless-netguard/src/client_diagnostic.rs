// SPDX-License-Identifier: MIT
//! Explicit developer-only last-entered source labels, never authority.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cut {
    NotEntered,
    EndpointAdmit,
    EndpointRecheck,
    SocketCreate,
    Connect,
    ConnectWait,
    Connected,
    Peer,
    WriteRequest,
    ReadPrefix,
    FrameLength,
    ReadBody,
    Decode,
    PositiveResponse,
    FinalEndpoint,
    FinalPeer,
    GroupOpen,
    RunOpen,
    ParentOpen,
    LeafOpen,
    GroupValidate,
    RunValidate,
    RunNamed,
    ParentNamed,
    ParentShape,
    LeafNamed,
    LeafShape,
}
impl Cut {
    const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::EndpointAdmit => "endpoint_admit",
            Self::EndpointRecheck => "endpoint_recheck",
            Self::SocketCreate => "socket_create",
            Self::Connect => "connect",
            Self::ConnectWait => "connect_wait",
            Self::Connected => "connected",
            Self::Peer => "peer",
            Self::WriteRequest => "write_request",
            Self::ReadPrefix => "read_prefix",
            Self::FrameLength => "frame_length",
            Self::ReadBody => "read_body",
            Self::Decode => "decode",
            Self::PositiveResponse => "positive_response",
            Self::FinalEndpoint => "final_endpoint",
            Self::FinalPeer => "final_peer",
            Self::GroupOpen => "group_open",
            Self::RunOpen => "run_open",
            Self::ParentOpen => "parent_open",
            Self::LeafOpen => "leaf_open",
            Self::GroupValidate => "group_validate",
            Self::RunValidate => "run_validate",
            Self::RunNamed => "run_named",
            Self::ParentNamed => "parent_named",
            Self::ParentShape => "parent_shape",
            Self::LeafNamed => "leaf_named",
            Self::LeafShape => "leaf_shape",
        }
    }
}
thread_local! {
    static LAST: Cell<Cut> = const { Cell::new(Cut::NotEntered) };
}
pub(crate) fn mark(cut: Cut) {
    LAST.set(cut);
}
pub fn last_token() -> &'static str {
    LAST.get().token()
}
#[cfg(test)]
#[test]
fn diagnostic_is_closed_thread_local_and_not_an_exchange_input() {
    mark(Cut::ReadPrefix);
    let child = std::thread::spawn(|| {
        assert_eq!(last_token(), "not_entered");
        mark(Cut::PositiveResponse);
        assert_eq!(last_token(), "positive_response");
    });
    child.join().unwrap();
    assert_eq!(last_token(), "read_prefix");
    mark(Cut::NotEntered);
}

// SPDX-License-Identifier: MIT
// Included only inside the existing test module with both developer + TUI.
// No production entry, fixture permit, raw-ID mapping or privileged dispatcher.

#[test]
#[ignore = "separately reviewed disposable VM namespace and exact developer pair only"]
fn actual_owner_developer_pair_tui_workspace_in_dev_vm() {
    assert!(std::env::var("OMAVLESS_CLOSE_DEVELOPER_CLIENT_VM").as_deref() == Ok("1"));
    assert_ne!(nix::unistd::getuid().as_raw(), 0);
    composed_core_selected_close_with_client(
        PathBuf::from("/var/lib/omavless-close-development-pair/mihomo"),
        true,
        false,
        true,
        true,
        false,
        false,
        false,
    );
}

fn closed_client_reply_failure(
    method: &str,
    value: &serde_json::Value,
) -> (&'static str, &'static str) {
    let method = match method {
        "system.hello" => "system.hello",
        "capabilities.get" => "capabilities.get",
        "development.connections.snapshot" => "development.connections.snapshot",
        "development.connections.prepare" => "development.connections.prepare",
        "development.connections.confirm" => "development.connections.confirm",
        "development.connections.receipt" => "development.connections.receipt",
        _ => "unrecognized_method",
    };
    let code = value["error"]["code"]
        .as_str()
        .and_then(omavless_control_protocol::StableErrorCode::parse)
        .map_or(
            "unrecognized_error",
            omavless_control_protocol::StableErrorCode::as_str,
        );
    (method, code)
}
fn assert_client_reply(method: &str, value: &serde_json::Value) {
    if value["ok"] != true {
        let (method, code) = closed_client_reply_failure(method, value);
        panic!("client_reply_refused method={method} code={code}");
    }
}

#[test]
fn client_reply_diagnostic_discards_private_values_and_reports_only_closed_enums() {
    let value = json!({"ok":false,"error":{"code":"invalid_argument","message":"private bearer", "details":{"secret":"private"}}});
    assert_eq!(
        closed_client_reply_failure("system.hello", &value),
        ("system.hello", "invalid_argument")
    );
    assert_eq!(
        closed_client_reply_failure(
            "private command",
            &json!({"error":{"code":"private bearer"}})
        ),
        ("unrecognized_method", "unrecognized_error")
    );
}

fn exercise_actual_tui_workspace(
    fixture: &SocketFixture,
    clients: &mut [std::net::TcpStream],
    targets: [u16; 2],
) {
    use omavless_control_protocol::{FrameKind, encode_request, make_request, write_unary_frame};
    use omavless_tui::developer_close::{Call, Input, KeyCode, KeyEvent, KeyModifiers, Workspace};
    use omavless_tui::{i18n::Locale, model::ReadError};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn send(input: Input) -> Call {
        match input {
            Input::Send(call) => call,
            _ => panic!("expected fixed client call"),
        }
    }
    fn echo(client: &mut std::net::TcpStream) {
        client.write_all(b"alive").unwrap();
        let mut bytes = [0; 5];
        client.read_exact(&mut bytes).unwrap();
        assert!(bytes == *b"alive");
    }
    fn refresh(workspace: &mut Workspace, fixture: &SocketFixture, calls: &mut Vec<&'static str>) {
        let mut call = send(workspace.input(key(KeyCode::Char('r')), Instant::now(), true));
        loop {
            calls.push(call.method());
            let value = crate::call(&fixture.paths, call.method(), call.params()).unwrap();
            assert_client_reply(call.method(), &value);
            if call.method() == "development.connections.snapshot" {
                assert!(
                    value["result"]["rows"]
                        .as_array()
                        .is_some_and(|r| r.len() == 2)
                );
            }
            match workspace.accept(Ok(value), Instant::now()) {
                Some(next) => call = next,
                None => break,
            }
        }
    }
    fn prepare_b(
        workspace: &mut Workspace,
        fixture: &SocketFixture,
        target: u16,
        calls: &mut Vec<&'static str>,
    ) {
        // Target comes from the SAME original opaque row's prepare display.
        // Nothing is joined to runtime.connection_rows or a raw controller ID.
        for attempt in 0..2 {
            let call = send(workspace.input(key(KeyCode::Char('x')), Instant::now(), true));
            assert!(call.method() == "development.connections.prepare");
            calls.push(call.method());
            let value = crate::call(&fixture.paths, call.method(), call.params()).unwrap();
            assert_client_reply(call.method(), &value);
            let is_b = value["result"]["display"]["port"] == target;
            assert!(workspace.accept(Ok(value), Instant::now()).is_none());
            if is_b {
                return;
            }
            assert!(attempt == 0);
            assert!(matches!(
                workspace.input(key(KeyCode::Esc), Instant::now(), true),
                Input::None
            ));
            assert!(matches!(
                workspace.input(key(KeyCode::Down), Instant::now(), true),
                Input::None
            ));
        }
        panic!("selected original target unavailable");
    }

    assert!(targets[0] != targets[1] && clients.len() == 2);
    let before = fixture.desired_bytes();
    let mut workspace = Workspace::new(Locale::En);
    let mut calls = Vec::new();
    refresh(&mut workspace, fixture, &mut calls);
    prepare_b(&mut workspace, fixture, targets[1], &mut calls);
    assert!(matches!(
        workspace.input(key(KeyCode::Esc), Instant::now(), true),
        Input::None
    ));
    for client in clients.iter_mut() {
        echo(client);
    }
    prepare_b(&mut workspace, fixture, targets[1], &mut calls);
    assert!(matches!(
        workspace.input(key(KeyCode::Enter), Instant::now(), false),
        Input::None
    ));
    for client in clients.iter_mut() {
        echo(client);
    }
    workspace.input(key(KeyCode::Esc), Instant::now(), true);
    refresh(&mut workspace, fixture, &mut calls);
    prepare_b(&mut workspace, fixture, targets[1], &mut calls);
    // Genuine original lifetime elapses, not a fabricated clock/expired owner.
    std::thread::sleep(CONFIRMATION_LIFETIME + Duration::from_millis(10));
    assert!(matches!(
        workspace.input(key(KeyCode::Enter), Instant::now(), true),
        Input::None
    ));
    for client in clients.iter_mut() {
        echo(client);
    }
    assert!(!calls.contains(&"development.connections.confirm"));
    assert!(fixture.desired_bytes() == before);

    refresh(&mut workspace, fixture, &mut calls);
    prepare_b(&mut workspace, fixture, targets[1], &mut calls);
    let confirm = send(workspace.input(key(KeyCode::Enter), Instant::now(), true));
    assert!(confirm.method() == "development.connections.confirm");
    calls.push(confirm.method());
    // Real complete-frame write to the original same-UID socket; authenticate
    // before writing. Drop WITHOUT reading any confirm reply. This is a real
    // client-response loss, not a fabricated server/effect result.
    let uid = nix::unistd::Uid::current().as_raw();
    crate::validate_client_directory(&fixture.paths.directory, uid).unwrap();
    let socket_metadata = fs::symlink_metadata(&fixture.paths.socket).unwrap();
    assert!(
        socket_metadata.file_type().is_socket()
            && socket_metadata.uid() == uid
            && socket_metadata.mode() & 0o7777 == 0o600
    );
    let mut client = UnixStream::connect(&fixture.paths.socket).unwrap();
    let credentials =
        nix::sys::socket::getsockopt(&client, nix::sys::socket::sockopt::PeerCredentials).unwrap();
    assert!(credentials.uid() == uid);
    client
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request = make_request("tui-lost-confirm", confirm.method(), confirm.params()).unwrap();
    write_unary_frame(
        &mut client,
        &encode_request(&request).unwrap(),
        FrameKind::Request,
    )
    .unwrap();
    client.shutdown(std::net::Shutdown::Write).unwrap();
    drop(client);
    assert!(
        workspace
            .accept(Err(ReadError::Unavailable), Instant::now())
            .is_none()
    );
    for code in [KeyCode::Enter, KeyCode::Char('r'), KeyCode::Char('x')] {
        assert!(matches!(
            workspace.input(key(code), Instant::now(), true),
            Input::None
        ));
    }

    // Establish that this particular lost reply really followed the selected
    // effect BEFORE competing receipt clients can contend with admission.
    let until = Instant::now() + Duration::from_secs(4);
    let mut closed = false;
    while Instant::now() < until {
        let result = clients[1].write_all(b"after").and_then(|()| {
            let mut bytes = [0; 5];
            clients[1].read(&mut bytes)
        });
        match result {
            Ok(0) => {
                closed = true;
                break;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                closed = true;
                break;
            }
            Ok(_) => {}
            Err(_) => panic!("selected closure unproven"),
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(closed);
    echo(&mut clients[0]);
    let receipt_call = send(workspace.input(key(KeyCode::Char('u')), Instant::now(), true));
    assert!(receipt_call.method() == "development.connections.receipt");
    assert!(receipt_call.params()["operationId"] == confirm.params()["operationId"]);
    let until = Instant::now() + Duration::from_secs(4);
    let terminal = loop {
        calls.push(receipt_call.method());
        let response =
            crate::call(&fixture.paths, receipt_call.method(), receipt_call.params()).unwrap();
        assert_client_reply(receipt_call.method(), &response);
        let complete = response["result"]["state"] == "finished";
        assert!(
            workspace
                .accept(Ok(response.clone()), Instant::now())
                .is_none()
        );
        if complete {
            break response;
        }
        assert!(Instant::now() < until);
        let next = send(workspace.input(key(KeyCode::Char('u')), Instant::now(), true));
        assert!(next.method() == receipt_call.method() && next.params() == receipt_call.params());
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(
        terminal["result"]["outcome"] == "closed" && terminal["result"]["receiptRevision"] == 1
    );
    // Repeat the EXACT read already emitted by the real Workspace. No second
    // confirmation, operation ID or authority refresh is sent after the effect.
    let replay = crate::call(&fixture.paths, receipt_call.method(), receipt_call.params()).unwrap();
    assert!(replay["ok"] == true && replay["result"] == terminal["result"]);
    assert!(
        calls
            .iter()
            .filter(|method| **method == "development.connections.confirm")
            .count()
            == 1
    );
    assert!(fixture.desired_bytes() == before);
    echo(&mut clients[0]);
}

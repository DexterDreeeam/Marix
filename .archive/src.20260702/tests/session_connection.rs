use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use marix::agent::frontdoor::Session;
use marix::client::core::ClientSession;
use marix::common::channel::ChannelError;

const SESSION_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(20);
const DISCONNECT_SETTLE_DELAY: Duration = Duration::from_millis(100);

#[test]
fn sessions_connect_repeatedly() {
    for _ in 0..8 {
        let address = unused_loopback_address();
        let agent_rx = spawn_new_agent_accept(address);
        let mut client = connect_client_with_retry(address);
        let mut agent = receive_agent(agent_rx);

        client.close().expect("client close should succeed");
        agent.close().expect("agent close should succeed");
    }
}

#[test]
fn client_connect_without_agent_fails_then_connects_after_agent_starts() {
    let address = unused_loopback_address();

    let result = ClientSession::connect(address);

    assert!(result.is_err(), "client connected without an agent");

    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    client.close().expect("client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn one_agent_rejects_second_active_client() {
    let address = unused_loopback_address();
    let agent = Session::new(address).expect("agent session should be created");
    let agent_rx = spawn_agent_accept(agent);
    let mut first_client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    let second_client = ClientSession::connect(address);

    assert!(
        second_client.is_err(),
        "second client connected while the first client was active"
    );
    first_client
        .close()
        .expect("first client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn one_agent_accepts_repeated_clients_after_disconnect() {
    let address = unused_loopback_address();
    let mut agent = Session::new(address).expect("agent session should be created");

    for _ in 0..6 {
        let agent_rx = spawn_agent_accept(agent);
        let mut client = connect_client_with_retry(address);
        agent = receive_agent(agent_rx);

        client.close().expect("client close should succeed");
        drop(client);
        thread::sleep(DISCONNECT_SETTLE_DELAY);
    }

    agent.close().expect("agent close should succeed");
}

#[test]
fn agent_close_succeeds_after_client_is_dropped() {
    for _ in 0..4 {
        let address = unused_loopback_address();
        let agent_rx = spawn_new_agent_accept(address);
        let client = connect_client_with_retry(address);
        let mut agent = receive_agent(agent_rx);

        drop(client);
        thread::sleep(DISCONNECT_SETTLE_DELAY);

        agent
            .close()
            .expect("agent close should tolerate a dropped client");
    }
}

#[test]
fn client_close_succeeds_after_agent_is_dropped() {
    for _ in 0..4 {
        let address = unused_loopback_address();
        let agent_rx = spawn_new_agent_accept(address);
        let mut client = connect_client_with_retry(address);
        let agent = receive_agent(agent_rx);

        drop(agent);
        thread::sleep(DISCONNECT_SETTLE_DELAY);

        client
            .close()
            .expect("client close should tolerate a dropped agent");
    }
}

fn spawn_new_agent_accept(address: SocketAddr) -> Receiver<Result<Session, ChannelError>> {
    let agent = Session::new(address).expect("agent session should be created");
    spawn_agent_accept(agent)
}

fn spawn_agent_accept(agent: Session) -> Receiver<Result<Session, ChannelError>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = accept_agent(agent);
        let _ = tx.send(result);
    });
    rx
}

fn accept_agent(mut agent: Session) -> Result<Session, ChannelError> {
    agent.accept()?;
    Ok(agent)
}

fn receive_agent(rx: Receiver<Result<Session, ChannelError>>) -> Session {
    rx.recv_timeout(SESSION_TIMEOUT)
        .expect("agent did not accept before timeout")
        .expect("agent accept failed")
}

fn connect_client_with_retry(address: SocketAddr) -> ClientSession {
    let started = Instant::now();
    let mut last_error = None;

    while started.elapsed() < SESSION_TIMEOUT {
        match ClientSession::connect(address) {
            Ok(client) => return client,
            Err(error) => {
                last_error = Some(error);
                thread::sleep(CONNECT_RETRY_DELAY);
            }
        }
    }

    panic!("client did not connect before timeout: {last_error:?}");
}

fn unused_loopback_address() -> SocketAddr {
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("loopback port should be available");
    listener
        .local_addr()
        .expect("loopback listener should expose its address")
}

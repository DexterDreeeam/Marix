use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use marix::agent::frontdoor::Session;
use marix::client::core::ClientSession;
use marix::common::channel::ChannelError;
use marix::common::message::ChatRequest;

const SESSION_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(20);
const DISCONNECT_SETTLE_DELAY: Duration = Duration::from_millis(100);
const TASK_COMPLETION_RACE_DELAY: Duration = Duration::from_millis(120);
const TASK_COUNT: usize = 24;
const TASK_STARTUP_DELAY_MS: [u64; 5] = [0, 1, 5, 20, 50];

#[test]
fn many_pending_tasks_can_cancel() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    let mut tasks = create_tasks(&mut client, TASK_COUNT, "pending");
    for task in &mut tasks {
        task.cancel().expect("client task cancel should succeed");
        task.cancel()
            .expect("client task cancel should be idempotent");
    }

    client.close().expect("client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn tasks_cancel_after_startup_delays() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    for delay_ms in TASK_STARTUP_DELAY_MS {
        let mut task = client
            .create_task(chat_request(&format!("startup-delay-{delay_ms}")))
            .expect("client should create a delayed task");

        sleep_startup_delay(delay_ms);

        task.cancel()
            .expect("client task cancel should tolerate startup delay");
        task.cancel()
            .expect("client task cancel should remain idempotent after startup delay");
    }

    client.close().expect("client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn task_cancels_tolerate_short_completion_window() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    let mut tasks = create_tasks(&mut client, TASK_COUNT, "completion-window");
    thread::sleep(TASK_COMPLETION_RACE_DELAY);

    for task in tasks.iter_mut() {
        task.cancel()
            .expect("client task cancel should tolerate possible completion race");
        task.cancel()
            .expect("client task cancel should remain idempotent after completion race");
    }

    client.close().expect("client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn task_terminal_receive_tolerates_agent_close_after_startup_delays() {
    for delay_ms in TASK_STARTUP_DELAY_MS {
        let address = unused_loopback_address();
        let agent_rx = spawn_new_agent_accept(address);
        let mut client = connect_client_with_retry(address);
        let mut agent = receive_agent(agent_rx);
        let mut task = client
            .create_task(chat_request(&format!("terminal-delay-{delay_ms}")))
            .expect("client should create a terminal-delay task");

        sleep_startup_delay(delay_ms);
        agent.close().expect("agent close should succeed");
        thread::sleep(DISCONNECT_SETTLE_DELAY);

        assert_task_disconnected(task.receive());
        client
            .close()
            .expect("client close should tolerate a closed agent");
    }
}

#[test]
fn pending_tasks_do_not_block_repeated_client_reconnects() {
    let address = unused_loopback_address();
    let mut agent = Session::new(address).expect("agent session should be created");

    for round in 0..4 {
        let agent_rx = spawn_agent_accept(agent);
        let mut client = connect_client_with_retry(address);
        agent = receive_agent(agent_rx);

        let tasks = create_tasks(&mut client, TASK_COUNT, &format!("round-{round}"));

        client.close().expect("client close should succeed");
        drop(tasks);
        drop(client);
        thread::sleep(DISCONNECT_SETTLE_DELAY);
    }

    agent.close().expect("agent close should succeed");
}

#[test]
fn second_client_is_rejected_while_first_has_pending_tasks() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut first_client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);

    let _tasks = create_tasks(&mut first_client, TASK_COUNT, "first-client");

    let second_client = ClientSession::connect(address);

    assert!(
        second_client.is_err(),
        "second client connected while first client had pending tasks"
    );
    first_client
        .close()
        .expect("first client close should succeed");
    agent.close().expect("agent close should succeed");
}

#[test]
fn task_cleanup_tolerates_agent_drop() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let agent = receive_agent(agent_rx);

    let mut tasks = create_tasks(&mut client, TASK_COUNT, "agent-drop");
    drop(agent);
    thread::sleep(DISCONNECT_SETTLE_DELAY);

    for task in tasks.iter_mut() {
        let _ = task.cancel();
        task.cancel()
            .expect("client task cancel should be idempotent");
    }
    client
        .close()
        .expect("client close should tolerate a dropped agent");
}

#[test]
fn task_operations_tolerate_agent_close() {
    let address = unused_loopback_address();
    let agent_rx = spawn_new_agent_accept(address);
    let mut client = connect_client_with_retry(address);
    let mut agent = receive_agent(agent_rx);
    let mut tasks = create_tasks(&mut client, TASK_COUNT, "agent-close");

    agent.close().expect("agent close should succeed");
    thread::sleep(DISCONNECT_SETTLE_DELAY);

    for task in tasks.iter_mut() {
        let _ = task.receive();
        let _ = task.cancel();
        task.cancel()
            .expect("client task cancel should be idempotent");
    }
    client
        .close()
        .expect("client close should tolerate a closed agent");
}

fn create_tasks(
    client: &mut ClientSession,
    count: usize,
    prefix: &str,
) -> Vec<marix::client::core::ClientTask> {
    (0..count)
        .map(|index| {
            client
                .create_task(chat_request(&format!("{prefix}-open-{index}")))
                .expect("client should create a task")
        })
        .collect()
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

fn sleep_startup_delay(delay_ms: u64) {
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }
}

fn assert_task_disconnected(
    result: Result<marix::common::message::ResponseMessageEnvelope, ChannelError>,
) {
    assert!(
        matches!(result, Err(ChannelError::Disconnected)),
        "task should receive a terminal disconnection signal, got {result:?}"
    );
}

fn unused_loopback_address() -> SocketAddr {
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("loopback port should be available");
    listener
        .local_addr()
        .expect("loopback listener should expose its address")
}

fn chat_request(content: &str) -> ChatRequest {
    ChatRequest {
        content: content.to_owned(),
    }
}

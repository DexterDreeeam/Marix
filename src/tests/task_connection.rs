use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use marix::agent::frontdoor::AgentSession;
use marix::client::core::ClientSession;
use marix::common::channel::ChannelError;
use marix::common::message::ChatRequest;

// -- Private -- //

const SESSION_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(20);
const DISCONNECT_SETTLE_DELAY: Duration = Duration::from_millis(100);
const TASK_COUNT: usize = 24;

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
fn pending_tasks_do_not_block_repeated_client_reconnects() {
    let address = unused_loopback_address();
    let mut agent = AgentSession::new(address).expect("agent session should be created");

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

fn spawn_new_agent_accept(address: SocketAddr) -> Receiver<Result<AgentSession, ChannelError>> {
    let agent = AgentSession::new(address).expect("agent session should be created");
    spawn_agent_accept(agent)
}

fn spawn_agent_accept(agent: AgentSession) -> Receiver<Result<AgentSession, ChannelError>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = accept_agent(agent);
        let _ = tx.send(result);
    });
    rx
}

fn accept_agent(mut agent: AgentSession) -> Result<AgentSession, ChannelError> {
    agent.accept()?;
    Ok(agent)
}

fn receive_agent(rx: Receiver<Result<AgentSession, ChannelError>>) -> AgentSession {
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

fn chat_request(content: &str) -> ChatRequest {
    ChatRequest {
        content: content.to_owned(),
    }
}

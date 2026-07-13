//! Integration-style unit tests for the transport channel handshake.
//!
//! These tests bind real localhost TCP ports and drive the full
//! `accept_channel` / `connect_channel` handshake. They rely on the
//! process-global config cache installed by [`Config::mock`], so they
//! MUST run serialized: every test locks [`TEST_GUARD`] for its whole
//! body, and each test uses a distinct port to avoid rebind collisions.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::structure::{
    ChannelEndpoint, ChannelError, NetReceiver, accept_channel, connect_channel,
};

/// Serializes the tests: `Config::mock` writes a global config, so
/// concurrent tests would clobber each other's token/port.
static TEST_GUARD: Mutex<()> = Mutex::new(());
/// Ensures the base config fixture is written and `MARIX_CONFIG` is set
/// exactly once, under the guard, so no other thread races the env
/// mutation.
static BASE_CONFIG: OnceLock<()> = OnceLock::new();

const BASE_CONFIG_TOML: &str = r#"
name = "marix-channel-test"

[runtime]
environment = "test"
mode = "network"
marix_path = "."

[client]
interactive = false
request_timeout_ms = 1000

[server]
enabled = true
ip = "127.0.0.1"
auth_token = "base-token"
client_port = 39000
host_port = 39001
telemetry_port = 39002
telemetry_http_port = 39003
max_turns = 8

[model]
backend = "deepseek"

[model.deepseek]
endpoint = "https://example.invalid/chat"
model = "deepseek-chat"
api_key = "test-key"

[tool]
directory = "tool"
"#;

fn ensure_base_config() {
    BASE_CONFIG.get_or_init(|| {
        let path = std::env::temp_dir().join("marix_channel_test_config.toml");
        std::fs::write(&path, BASE_CONFIG_TOML).expect("write base test config");
        // SAFETY: only ever called while holding TEST_GUARD, so no other
        // test thread is reading the environment concurrently.
        unsafe {
            std::env::set_var("MARIX_CONFIG", &path);
        }
    });
}

/// Installs a config whose server section uses `token` and binds the
/// client endpoint to `client_port`, and returns nothing. Subsequent
/// `Config::load()` calls (inside `accept_channel`/`connect_channel`)
/// observe this config.
fn install_config(token: &str, client_port: u16) {
    install_config_with_ip(token, client_port, "127.0.0.1");
}

fn install_config_with_ip(token: &str, client_port: u16, ip: &str) {
    ensure_base_config();
    let overlay =
        format!("[server]\nip = \"{ip}\"\nauth_token = \"{token}\"\nclient_port = {client_port}");
    Config::mock(&[overlay.as_str()]).expect("install mock config");
}

/// Receives one message on `rx`, driving a dedicated current-thread
/// runtime, giving up after `timeout`.
fn recv_message<T>(rx: &mut NetReceiver<T>, timeout: Duration) -> Option<T>
where
    T: remoc::RemoteSend,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build test receive runtime");
    runtime.block_on(async {
        match tokio::time::timeout(timeout, rx.recv()).await {
            Ok(Ok(message)) => message,
            _ => None,
        }
    })
}

#[test]
fn golden_path_round_trip() {
    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config("golden-token", 34110);

    let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    // Give the accept thread time to bind before the single-shot connect.
    std::thread::sleep(Duration::from_millis(300));

    let (client_tx, mut client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("connect");
    let (server_tx, mut server_rx) = accept
        .join()
        .expect("accept thread panicked")
        .expect("accept succeeds");

    // connecter -> server
    client_tx.try_send("ping".to_owned()).expect("client send");
    assert_eq!(
        recv_message(&mut server_rx, Duration::from_secs(5)).as_deref(),
        Some("ping"),
    );

    // server -> connecter
    server_tx.try_send("pong".to_owned()).expect("server send");
    assert_eq!(
        recv_message(&mut client_rx, Duration::from_secs(5)).as_deref(),
        Some("pong"),
    );
}

#[test]
fn server_listener_ignores_configured_server_ip() {
    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config_with_ip("bind-token", 34115, "192.0.2.1");

    let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    // Let the server load the remote, non-local IP before changing the
    // client configuration to the local address used for the connection.
    std::thread::sleep(Duration::from_millis(300));

    install_config("bind-token", 34115);
    let (client_tx, _client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("connect");
    let (_server_tx, mut server_rx) = accept
        .join()
        .expect("accept thread panicked")
        .expect("accept succeeds on wildcard bind");

    client_tx
        .try_send("wildcard".to_owned())
        .expect("client send");
    assert_eq!(
        recv_message(&mut server_rx, Duration::from_secs(5)).as_deref(),
        Some("wildcard"),
    );
}

#[test]
fn wrong_token_rejected_on_connecter() {
    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config("correct", 34120);

    let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    // Let the accept thread read the "correct" token and bind before the
    // config is re-mocked out from under it.
    std::thread::sleep(Duration::from_millis(400));

    // The connecter now presents the wrong token.
    install_config("wrong", 34120);
    let result = connect_channel::<String>(ChannelEndpoint::Client);
    assert!(
        matches!(result, Err(ChannelError::Auth(_))),
        "expected Auth error, got {result:?}",
    );

    // accept_channel keeps looping on a rejected connection; it must not
    // have returned.
    assert!(
        !accept.is_finished(),
        "accept returned on a rejected-token connection",
    );

    // Unblock the accept thread with a valid connection so the test ends.
    install_config("correct", 34120);
    let (_client_tx, _client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("valid connect");
    let _ = accept
        .join()
        .expect("accept thread panicked")
        .expect("accept succeeds after a valid connect");
}

#[test]
fn handshake_timeout_drops_connection() {
    use std::io::{ErrorKind, Read};

    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config("timeout-token", 34130);

    let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    std::thread::sleep(Duration::from_millis(400));

    // A raw TCP connection that never runs the remoc/token handshake.
    let mut raw = std::net::TcpStream::connect("127.0.0.1:34130").expect("raw tcp connect");
    raw.set_read_timeout(Some(Duration::from_secs(8)))
        .expect("set read timeout");

    let start = Instant::now();
    let mut buffer = [0_u8; 64];
    let mut server_closed = false;
    loop {
        match raw.read(&mut buffer) {
            // Clean EOF: the server timed out and closed the connection.
            Ok(0) => {
                server_closed = true;
                break;
            }
            // May receive remoc hello bytes before the timeout fires.
            Ok(_) => {
                if start.elapsed() > Duration::from_secs(8) {
                    break;
                }
            }
            Err(error) => {
                match error.kind() {
                    // Our own read deadline elapsed: server did NOT close.
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                        server_closed = false;
                    }
                    // Reset/abort: the server tore the connection down.
                    _ => {
                        server_closed = true;
                    }
                }
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    assert!(server_closed, "server never closed the idle connection");
    assert!(
        elapsed >= Duration::from_secs(4),
        "connection closed too early ({elapsed:?}); expected the ~5s handshake timeout",
    );
    assert!(
        elapsed <= Duration::from_secs(8),
        "connection closed too late ({elapsed:?})",
    );

    // The server keeps blocking for a real connection.
    assert!(
        !accept.is_finished(),
        "accept returned on a timed-out connection",
    );

    drop(raw);

    // Clean up: a real connection lets accept succeed and the thread end.
    let (_client_tx, _client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("valid connect");
    let _ = accept
        .join()
        .expect("accept thread panicked")
        .expect("accept succeeds after a valid connect");
}

#[test]
fn multiple_connections_same_endpoint() {
    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config("multi-token", 34140);

    // Connection A.
    let accept_a = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    std::thread::sleep(Duration::from_millis(300));
    let (a_client_tx, a_client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("connect A");
    let (a_server_tx, mut a_server_rx) = accept_a
        .join()
        .expect("accept A thread panicked")
        .expect("accept A succeeds");

    // Accepting A dropped its listener; let the port fully free up.
    std::thread::sleep(Duration::from_millis(200));

    // Connection B, same endpoint/port.
    let accept_b = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    std::thread::sleep(Duration::from_millis(300));
    let (b_client_tx, mut b_client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("connect B");
    let (b_server_tx, mut b_server_rx) = accept_b
        .join()
        .expect("accept B thread panicked")
        .expect("accept B succeeds");

    // A message on connection A arrives.
    a_client_tx.try_send("a-ping".to_owned()).expect("A send");
    assert_eq!(
        recv_message(&mut a_server_rx, Duration::from_secs(5)).as_deref(),
        Some("a-ping"),
    );

    // Drop connection A entirely.
    drop(a_client_tx);
    drop(a_client_rx);
    drop(a_server_tx);
    drop(a_server_rx);

    // Connection B still works in both directions, independent of A.
    b_client_tx.try_send("b-ping".to_owned()).expect("B send");
    assert_eq!(
        recv_message(&mut b_server_rx, Duration::from_secs(5)).as_deref(),
        Some("b-ping"),
    );
    b_server_tx
        .try_send("b-pong".to_owned())
        .expect("B send back");
    assert_eq!(
        recv_message(&mut b_client_rx, Duration::from_secs(5)).as_deref(),
        Some("b-pong"),
    );

    drop(b_client_tx);
    drop(b_client_rx);
    drop(b_server_tx);
    drop(b_server_rx);
}

#[test]
fn repeated_connect_disconnect_cycles() {
    let _guard = TEST_GUARD.lock().unwrap_or_else(|error| error.into_inner());
    install_config("stress-token", 34150);

    for iteration in 0..10 {
        let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
        std::thread::sleep(Duration::from_millis(250));
        let (client_tx, client_rx) = connect_channel::<String>(ChannelEndpoint::Client)
            .unwrap_or_else(|error| panic!("connect on iteration {iteration}: {error:?}"));
        let (server_tx, mut server_rx) = accept
            .join()
            .expect("accept thread panicked")
            .unwrap_or_else(|error| panic!("accept on iteration {iteration}: {error:?}"));

        let payload = format!("msg-{iteration}");
        client_tx.try_send(payload.clone()).expect("send");
        assert_eq!(
            recv_message(&mut server_rx, Duration::from_secs(5)),
            Some(payload),
            "iteration {iteration}",
        );

        // Disconnect both ends.
        drop(client_tx);
        drop(client_rx);
        drop(server_tx);
        drop(server_rx);
        std::thread::sleep(Duration::from_millis(80));
    }

    // One more connection after all cycles proves no port/state leakage.
    let accept = std::thread::spawn(|| accept_channel::<String>(ChannelEndpoint::Client));
    std::thread::sleep(Duration::from_millis(250));
    let (client_tx, _client_rx) =
        connect_channel::<String>(ChannelEndpoint::Client).expect("final connect");
    let (_server_tx, mut server_rx) = accept
        .join()
        .expect("final accept thread panicked")
        .expect("final accept succeeds");
    client_tx.try_send("final".to_owned()).expect("final send");
    assert_eq!(
        recv_message(&mut server_rx, Duration::from_secs(5)).as_deref(),
        Some("final"),
    );
}

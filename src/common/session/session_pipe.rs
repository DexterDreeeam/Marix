use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{UserInput, UserOutput};

use super::SessionConfig;

pub struct SessionPipe;

impl SessionPipe {
    pub fn integrate_core() -> (CliSessionPipe, CoreSessionPipe) {
        let (input_tx, input_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();
        (
            CliSessionPipe::Local {
                input_tx,
                output_rx,
            },
            CoreSessionPipe::Local {
                input_rx,
                output_tx,
            },
        )
    }

    pub fn connect_core(config: &SessionConfig) -> io::Result<CliSessionPipe> {
        TcpStream::connect(config.bind_address()).map(CliSessionPipe::Tcp)
    }

    pub fn listen_core(config: &SessionConfig) -> io::Result<CoreSessionListener> {
        TcpListener::bind(config.bind_address()).map(|listener| CoreSessionListener { listener })
    }
}

pub enum CliSessionPipe {
    Local {
        input_tx: Sender<UserInput>,
        output_rx: Receiver<UserOutput>,
    },
    Tcp(TcpStream),
}

impl CliSessionPipe {
    pub fn send_input(&mut self, input: UserInput) -> io::Result<()> {
        match self {
            Self::Local { input_tx, .. } => input_tx
                .send(input)
                .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error)),
            Self::Tcp(stream) => write_json_line(stream, &input),
        }
    }

    pub fn receive_output(&mut self) -> io::Result<UserOutput> {
        match self {
            Self::Local { output_rx, .. } => output_rx
                .recv()
                .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error)),
            Self::Tcp(stream) => read_json_line(stream),
        }
    }

    pub fn request(&mut self, input: UserInput) -> io::Result<UserOutput> {
        self.send_input(input)?;
        self.receive_output()
    }
}

pub enum CoreSessionPipe {
    Local {
        input_rx: Receiver<UserInput>,
        output_tx: Sender<UserOutput>,
    },
    Tcp(TcpStream),
}

impl CoreSessionPipe {
    pub fn receive_input(&mut self) -> io::Result<UserInput> {
        match self {
            Self::Local { input_rx, .. } => input_rx
                .recv()
                .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error)),
            Self::Tcp(stream) => read_json_line(stream),
        }
    }

    pub fn send_output(&mut self, output: UserOutput) -> io::Result<()> {
        match self {
            Self::Local { output_tx, .. } => output_tx
                .send(output)
                .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error)),
            Self::Tcp(stream) => write_json_line(stream, &output),
        }
    }
}

pub struct CoreSessionListener {
    listener: TcpListener,
}

impl CoreSessionListener {
    pub fn accept(&self) -> io::Result<CoreSessionPipe> {
        self.listener
            .accept()
            .map(|(stream, _)| CoreSessionPipe::Tcp(stream))
    }
}

fn write_json_line<T: Serialize>(stream: &mut TcpStream, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn read_json_line<T: DeserializeOwned>(stream: &mut TcpStream) -> io::Result<T> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream.try_clone()?);
    reader.read_line(&mut line)?;
    serde_json::from_str(&line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

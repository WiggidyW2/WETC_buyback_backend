use serde_json;
use firestore;
use tonic;

#[derive(Debug)]
pub enum Error {
    EnvError(std::env::VarError),
    EnvParseError(std::net::AddrParseError),
    ListenerBindError(std::io::Error),
    ListenerAcceptError(std::io::Error),
    GRPCConnectionError(tonic::transport::Error),
    GRPCStatus(tonic::Status),
    SerializationError(serde_json::Error),
    DeserializationError(serde_json::Error),
    StdinError(std::io::Error),
    StdoutError(std::io::Error),
    FirestoreConnectionError(firestore::errors::FirestoreError),
    FirestoreInsertError(firestore::errors::FirestoreError),
    FirestoreSelectError(firestore::errors::FirestoreError),
    ParserSpawnError(std::io::Error),
    ParserPipeError(std::io::Error),
    ParserRuntimeError(String),
    ParserDeserializationError(serde_json::Error),
}

impl From<std::net::AddrParseError> for Error {
    fn from(err: std::net::AddrParseError) -> Self {
        Error::EnvParseError(err)        
    }
}

impl From<std::env::VarError> for Error {
    fn from(err: std::env::VarError) -> Self {
        Error::EnvError(err)        
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(err: tonic::transport::Error) -> Self {
        Error::GRPCConnectionError(err)
    }
}

impl From<tonic::Status> for Error {
    fn from(err: tonic::Status) -> Self {
        Error::GRPCStatus(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::StdinError(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

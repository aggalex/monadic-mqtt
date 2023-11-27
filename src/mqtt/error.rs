use std::str::Utf8Error;
use serde::{Serialize, Serializer};
use rumqttc::v5::ClientError;

#[derive(Serialize)]
struct SerializableError {
    code: String,
    description: String
}

impl SerializableError {
    pub fn new(code: &str, description: impl Serialize + Sized) -> Self {
        Self { code: code.to_string(), description: serde_json::to_string(&description).unwrap_or("Error serializing error".to_string()) }
    }
}

#[derive(PartialEq, Debug)]
pub enum EventParseError {
    Parsing(Utf8Error),
    Deserialization(String),
    Other
}

impl Serialize for EventParseError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match self {
            EventParseError::Parsing(_) => SerializableError::new("UTF8", "Payload is not valid UTF-8"),
            EventParseError::Deserialization(_) => SerializableError::new("DESERIALIZATION", "Payload not valid JSON"),
            EventParseError::Other => SerializableError::new("UNKNOWN", "Unknown error"),
        }.serialize(serializer)
    }
}

#[derive(Debug)]
pub enum PublishError {
    ClientError(ClientError),
    Serialization(serde_json::Error),
    EmptyPayload,
    Runtime(String)
}

impl Serialize for PublishError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match self {
            PublishError::ClientError(_) => SerializableError::new("CLIENT", "Client failed receiving event"),
            PublishError::Serialization(_) => SerializableError::new("SERIALIZATION", "Client failed serializing response"),
            PublishError::EmptyPayload => SerializableError::new("EMPTY_PAYLOAD", "Client responded with an empty payload"),
            PublishError::Runtime(err) => SerializableError::new("RUNTIME", err.clone())
        }.serialize(serializer)
    }
}

impl From<ClientError> for PublishError {
    fn from(value: ClientError) -> Self {
        PublishError::ClientError(value)
    }
}

impl From<serde_json::Error> for PublishError {
    fn from(value: serde_json::Error) -> Self {
        PublishError::Serialization(value)
    }
}

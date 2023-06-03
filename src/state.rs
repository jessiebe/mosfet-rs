use crate::api::ApiClientError;
use crate::opamp::Channel;

#[derive(Clone, Debug)]
pub enum State {
    Disconnected(String),
    Connecting(String),
    Connected(String),
    Polling(String),
    Sending(String),
    Waiting(String),
}

pub enum StateResponse {
    Reply(String),
    Error(String),
    None,
}

#[macro_export]
macro_rules! nullstr {
    () => {
        "".to_string()
    };
}

#[macro_export]
macro_rules! state_log {
    ($token:literal) => {
        ($token).to_string()
    };
}

impl State {
    pub async fn evaluate(self, client: &mut dyn Channel) -> Result<State, ApiClientError> {
        log::debug!("In state {:?}", self);
        match self {
            State::Disconnected(_) => Ok(State::Connecting(nullstr!())),

            State::Connecting(_) => match client.connect().await {
                Ok(StateResponse::Reply(data)) => Ok(State::Connected(data)),
                Ok(StateResponse::None) => Ok(State::Connected(nullstr!())),
                Ok(StateResponse::Error(e)) => Ok(State::Disconnected(e)),
                Err(e) => Ok(State::Disconnected(e.to_string())),
            },

            State::Connected(_) => match client.handshake().await {
                Ok(StateResponse::Reply(data)) => Ok(State::Sending(data)),
                Ok(StateResponse::None) => Ok(State::Polling(nullstr!())),
                Ok(StateResponse::Error(e)) => Ok(State::Disconnected(e)),
                Err(e) => Ok(State::Disconnected(e.to_string())),
            },

            State::Polling(_) => match client.poll().await {
                Ok(StateResponse::Reply(data)) => Ok(State::Sending(data)),
                Ok(StateResponse::None) => Ok(self),
                Ok(StateResponse::Error(e)) => Ok(State::Disconnected(e)),
                Err(e) => Ok(State::Connecting(e.to_string())),
            },

            State::Sending(_) => match client.send().await {
                Ok(StateResponse::Reply(data)) => Ok(State::Waiting(data)),
                Ok(StateResponse::None) => Ok(State::Polling(nullstr!())),
                Ok(StateResponse::Error(e)) => Ok(State::Polling(e)),
                Err(e) => Ok(State::Connecting(e.to_string())),
            },

            State::Waiting(_) => match client.wait().await {
                Ok(StateResponse::Reply(data)) => Ok(State::Polling(data)),
                Ok(StateResponse::None) => Ok(self),
                Ok(StateResponse::Error(e)) => Ok(State::Polling(e)),
                Err(e) => Ok(State::Connecting(e.to_string())),
            },
        }
    }
}

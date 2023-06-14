use crate::api::{ApiCallbacks, ApiClientError, ConnectionSettings};
use crate::{get_time_nanos, nullstr, state_log};
use crate::{
    opamp::*,
    opamp::{spec::*, Channel},
    state::*,
};
use async_trait::async_trait;
use libdeflater::{CompressionLvl, Compressor};
use prost::Message as ProstMessage;
use reqwest::{Client as ReqwestClient, Response};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// This defines a number in seconds of being idle before we generate a heartbeat to the server
const SERVER_POLL_DELAY: u128 = std::time::Duration::from_secs(30).as_nanos();

/// The `HttpClient` struct represents an HTTP client with various fields and methods for communication
/// with an OpAMP server.
///
/// Properties:
///
/// * `settings`: The `settings` property is of type `ConnectionSettings` and is used to store the
/// settings for the HTTP connection. It may include things like the timeout duration, maximum number of
/// redirects, and other connection-related settings.
/// * `address`: The `address` property is of type `url::Url` and represents the URL of the server that
/// the `HttpClient` is connecting to.
/// * `client`: `client` is an instance of the `ReqwestClient` struct, which is a HTTP client for making
/// requests to a server. It is used by the `HttpClient` struct to send HTTP requests to the server
/// specified by the `address` property.
/// * `backoff`: The `backoff` property is an unsigned 32-bit integer that represents the amount of time
/// (in milliseconds) that the client should wait before attempting to send another request to the
/// server in case of a failure or error. The value of `backoff` is typically increased exponentially
/// with each failed attempt
/// * `seqno`: seqno is a property of type u64. It is a sequence number of messages sent to the server
/// * `last_sent_timestamp`: `last_sent_timestamp` is a property of the `HttpClient` struct that stores
/// the timestamp of the last message sent by the client to the server. This property is used to detect
/// idle state and send the server a heartbeat message
/// * `agent_state`: `agent_state` is a `RefCell` that holds an optional `AgentToServer` struct. This
/// struct represents the internal synchronized state of the agent.
/// * `callback`: `callback` is a property of type `Arc<Mutex<Box<dyn ApiCallbacks + Send + Sync +
/// 'a>>>`.
/// * `inbox`: `inbox` is a vector that holds messages received from the server. It is of type
/// `Vec<ServerToAgent>`.
/// * `outbox`: The `outbox` property is a vector that holds messages to be sent from the client to the
/// server. It is of type `Vec<AgentToServer>`, where `AgentToServer` is a custom enum type that
/// represents the different types of messages that can be sent from the client to
/// * `state`: The `state` property is a variable of type `State` that represents the current state of
/// the `Client` instance. The FSM can change its state and this field indicates current state.
pub struct HttpClient<'a> {
    settings: ConnectionSettings,
    address: url::Url,
    client: ReqwestClient,
    backoff: u32,
    seqno: u64,
    last_sent_timestamp: u128,
    agent_state: RefCell<Option<AgentToServer>>,
    callback: Arc<Mutex<Box<dyn ApiCallbacks + Send + Sync + 'a>>>,
    inbox: Vec<ServerToAgent>,
    outbox: Vec<AgentToServer>,
    state: State,
}

impl HttpClient<'_> {
    pub fn new(
        settings: ConnectionSettings,
        cb: Box<dyn ApiCallbacks + Send + Sync + '_>,
    ) -> HttpClient {
        let path = settings.server_endpoint.clone() + settings.listen_path.as_str();
        let address = url::Url::parse(&path).unwrap();
        let client = ReqwestClient::new();

        HttpClient {
            settings,
            address,
            client,
            backoff: 0,
            seqno: 0,
            last_sent_timestamp: 0,
            agent_state: RefCell::new(None),
            callback: Arc::new(Mutex::new(cb)),
            inbox: vec![],
            outbox: vec![],
            state: State::Disconnected("".to_string()),
        }
    }

    /// This function sends a message to a server, receives a response, and handles compression if
    /// necessary.
    ///
    /// Arguments:
    ///
    /// * `message`: The message to be sent from the agent to the server.
    /// * `timeout`: `timeout` is a `Duration` parameter that specifies the maximum amount of time to
    /// wait for a response from the server before timing out.
    /// * `compress`: A boolean flag indicating whether the payload should be compressed before sending
    /// it to the server. If set to true, the payload will be compressed using gzip compression.
    ///
    /// Returns:
    ///
    /// a `Result` containing a `ServerToAgent` message if the request is successful, or an
    /// `ApiClientError` if there is an error.
    pub async fn send_and_receive(
        &mut self,
        message: &mut AgentToServer,
        timeout: Duration,
        compress: bool,
    ) -> Result<ServerToAgent, ApiClientError> {
        self.seqno += 1;
        message.sequence_num = self.seqno;
        self.last_sent_timestamp = crate::get_time_nanos!();
        if let Some(state) = self.agent_state.borrow().as_ref() {
            message.capabilities = state.capabilities.clone();
            message.flags = state.flags.clone();
        } else {
            log::warn!("Missing persistent agent state");
        }
        log::debug!("Sending \n: {:#?}", &message);

        let request_body = message.encode_to_vec();

        log::debug!(
            "Sending a post to [{}] with key [{}]",
            &self.address,
            &self.settings.api_key
        );

        let mut request = self
            .client
            .post(self.address.clone())
            .header("Content-Type", "application/x-protobuf")
            .header("api-key", format!("{}", &self.settings.api_key));

        if compress {
            log::debug!("Sending a compressed payload");
            let mut compressor = Compressor::new(CompressionLvl::fastest());
            let mut compressed_data: Vec<u8> =
                Vec::with_capacity(compressor.gzip_compress_bound(request_body.len()));
            compressor
                .gzip_compress(&request_body, compressed_data.as_mut_slice())
                .unwrap();

            request = request
                .header("Content-Encoding", "gzip")
                .header("Accept-Encoding", "gzip")
                .body(compressed_data);
        } else {
            log::debug!("Sending a standard (uncompressed) payload");
            request = request.body(request_body);
        }

        let response: Response = match request.timeout(timeout).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    log::debug!("Request successful");
                } else {
                    log::warn!("Request failure: {}", resp.status().as_str());
                    return Err(ApiClientError::new(line!(), resp.status().as_str()));
                }
                resp
            }
            Err(e) => {
                log::warn!("Request send failure: {}", e.to_string());
                return Err(ApiClientError::new(line!(), e.to_string().as_str()));
            }
        };

        let headers = response.headers().clone();

        let response_body = match response.bytes().await {
            Ok(body) => body.to_vec(),
            Err(e) => {
                return Err(ApiClientError::new(line!(), e.to_string().as_str()));
            }
        };

        // Check for compressed response and decompress if necessary
        let server_message = if let Some(encoding) = headers.get("Content-Encoding") {
            if encoding == "gzip" {
                let mut decompressor = libdeflater::Decompressor::new();
                const INBOUND_LENGTH: usize = 4096; // TODO: Find this more reliably

                let mut decompressed_data: Vec<u8> = Vec::with_capacity(INBOUND_LENGTH);
                decompressor
                    .gzip_decompress(&response_body, decompressed_data.as_mut_slice())
                    .unwrap();
                ServerToAgent::decode(&decompressed_data[1..]).unwrap()
            } else {
                log::debug!("{:#?}", &response_body);
                ServerToAgent::decode(&response_body[1..]).unwrap()
            }
        } else {
            log::debug!("{:#?}", &response_body);
            ServerToAgent::decode(&response_body[..]).unwrap()
        };
        Ok(server_message)
    }

    fn set_health(&mut self, healthy: bool) {
        let mut state = self.get_status().unwrap();

        if let Some(health) = &mut state.health {
            health.healthy = healthy;
        }

        *self.agent_state.borrow_mut() = Some(state);
    }

    fn get_health(&self) -> Option<AgentHealth> {
        let mut h = self
            .agent_state
            .borrow()
            .as_ref()
            .unwrap()
            .health
            .clone()
            .unwrap();
        h.start_time_unix_nano = get_time_nanos!() as u64;
        Some(h)
    }

    pub fn get_status(&mut self) -> Result<AgentToServer, ApiClientError> {
        // Populate an initial state if it doesnt yet exist
        if self.agent_state.borrow().is_none() {
            // Get our client configuration data
            let mut func = self.callback.lock().unwrap();
            let config_map = match func.get_configuration() {
                Ok(reply) => reply,
                Err(e) => {
                    log::warn!("API callback error: {}", e);
                    None
                }
            };

            // Get agent capabilities
            let (capabilities, flags) = func.get_features();

            *self.agent_state.borrow_mut() = Some(AgentToServer {
                instance_uid: self.settings.instance_id.clone(),
                sequence_num: 0, // Populated on send
                capabilities,
                flags,

                agent_description: Some(defaults::agent_description(
                    self.settings.name.as_str(),
                    self.settings.version.as_str(),
                )),
                health: Some(defaults::agent_health()),
                effective_config: Some(EffectiveConfig { config_map }),
                remote_config_status: Some(defaults::remote_config_status()),
                package_statuses: Some(defaults::package_statuses()),
                agent_disconnect: None,
            });
        }

        Ok(self.agent_state.borrow().as_ref().unwrap().clone())
    }
}

#[async_trait]
impl Channel for HttpClient<'_> {
    fn get_instance_id(&self) -> &String {
        &self.settings.instance_id
    }

    async fn connect(&mut self) -> Result<StateResponse, ApiClientError> {
        match self.client.head(self.address.clone()).send().await {
            Ok(response) => {
                // TODO: Check instead for a specific status page or API endpoint
                // that will always return a 200 for this to be a more reliable check

                if response.status().as_u16() != 404 {
                    return Ok(StateResponse::Reply(state_log!("remote server present")));
                }
                Ok(StateResponse::Error(response.status().to_string()))

                // if response.status().is_success() {
                //     return Ok(StateResponse::Reply(
                //         state_log!("remote server listening"),
                //     ));
                // } else {
                //     return Ok(StateResponse::Error(response.status().to_string()));
                // }
            }
            Err(e) => {
                // Backoff sleep
                self.backoff += 1;

                let connect_retries =
                    std::env::var("OPAMP_CONNECT_RETRIES").unwrap_or("10".to_string());
                if self.backoff > connect_retries.parse::<u32>().unwrap() {
                    log::error!("Failed to connect after {} retries", connect_retries);
                    return Err(ApiClientError {
                        code: line!(),
                        details: format!("Failed to connect to endpoint: {}", e),
                    });
                }
                let idle_sec = std::time::Duration::from_secs(2_u64.pow(self.backoff));
                std::thread::sleep(idle_sec);
                Ok(StateResponse::Error(state_log!(
                    "endpoint not responding: retrying .."
                )))
            }
        }
    }

    async fn handshake(&mut self) -> Result<StateResponse, ApiClientError> {
        match self.get_status() {
            Ok(self_status) => {
                self.outbox.push(self_status);
                Ok(StateResponse::Reply("Handshake enqueued".to_string()))
            }
            Err(e) => Ok(StateResponse::Error(format!(
                "State reporting failed: {}",
                e
            ))),
        }
    }

    async fn poll(&mut self) -> Result<StateResponse, ApiClientError> {
        let _ = self.get_health().is_some_and(|mut x| {
            if !x.healthy {
                self.set_health(true);
                x.healthy = true;
                // Report our own health as healthy (we're polling obviously!)
                log::debug!("Enqueued healthy message");
                self.outbox.push(AgentToServer {
                    instance_uid: self.get_instance_id().clone(),
                    health: Some(x),
                    ..AgentToServer::default()
                });
            }
            true
        });

        if !self.outbox.is_empty() {
            return Ok(StateResponse::Reply(state_log!("flushing queue")));
        }

        // Queue up a poll request if there is nothing pending to send and more than
        // 30 seconds have passed since the last message to the server
        if crate::get_time_nanos!() >= (self.last_sent_timestamp + SERVER_POLL_DELAY) {
            self.outbox.push(AgentToServer {
                instance_uid: self.settings.instance_id.clone(),
                ..AgentToServer::default()
            });
            return Ok(StateResponse::Reply(state_log!("server poll")));
        }

        if self.inbox.is_empty() {
            // Nothing to do
            return Ok(StateResponse::None);
        }

        // Check the inbox for messages to process
        if let Some(msg) = self.inbox.pop() {
            log::debug!("Received a binary message");
            log::debug!("[ServerToAgent]\n{:#?}", &msg);
            if let Some(_command) = &msg.command {
                let mut func = self.callback.lock().unwrap();
                match func.on_command(&msg) {
                    Ok(Some(reply)) => self.outbox.push(reply),
                    Ok(None) => {}
                    Err(e) => {
                        log::warn!("API callback error: {}", e);
                    }
                };
            }

            // Relay upstream errors to the client
            if msg.error_response.is_some() {
                let mut func = self.callback.lock().unwrap();
                func.on_error(&msg);
            }

            // Check and report full state
            if msg.flags & (ServerToAgentFlags::ReportFullState as u64) != 0 {
                // Check our health (matches with our instance_id)
                if msg.instance_uid == self.settings.instance_id {
                    match self.get_status() {
                        Ok(state) => self.outbox.push(state),
                        Err(e) => {
                            return Ok(StateResponse::Error(format!(
                                "State reporting failed: {}",
                                e
                            )));
                        }
                    }
                } else {
                    // The instance_uid isnt us. Must be one of our children
                    let mut func = self.callback.lock().unwrap();
                    match func.on_health_check(&msg) {
                        Ok(Some(reply)) => {
                            self.outbox.push(reply);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("API callback error: {}", e);
                        }
                    };
                }
            }

            if let Some(agent_rc) = &msg.remote_config {
                log::debug!("Received a remote config: {:?}", agent_rc);
                let mut func = self.callback.lock().unwrap();
                match func.on_agent_remote_config(&msg) {
                    Ok(Some(reply)) => self.outbox.push(reply),
                    Ok(None) => {}
                    Err(e) => {
                        log::warn!("API callback error: {}", e);
                    }
                };
            }

            // TODO: Check our agent capabilities if it supports any of these
            // else ignore them harmlessly
            if let Some(_connection_settings_offers) = &msg.connection_settings {
                if let Some(_owm_metrics) = &_connection_settings_offers.own_metrics {
                    let mut func = self.callback.lock().unwrap();
                    // TODO: Send specific type of this callback as an enum
                    match func.on_connection_settings_offers(&msg) {
                        Ok(Some(reply)) => self.outbox.push(reply),
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("API callback error: {}", e);
                        }
                    };
                }
                if let Some(_owm_traces) = &_connection_settings_offers.own_traces {
                    let mut func = self.callback.lock().unwrap();
                    // TODO: Send specific type of this callback as an enum
                    match func.on_connection_settings_offers(&msg) {
                        Ok(Some(reply)) => self.outbox.push(reply),
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("API callback error: {}", e);
                        }
                    };
                }
                if let Some(_owm_logs) = &_connection_settings_offers.own_logs {
                    let mut func = self.callback.lock().unwrap();
                    // TODO: Send specific type of this callback as an enum
                    match func.on_connection_settings_offers(&msg) {
                        Ok(Some(reply)) => self.outbox.push(reply),
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("API callback error: {}", e);
                        }
                    };
                }
            }

            if let Some(_packages_available) = &msg.packages_available {
                let mut func = self.callback.lock().unwrap();
                match func.on_packages_available(&msg) {
                    Ok(Some(reply)) => self.outbox.push(reply),
                    Ok(None) => {}
                    Err(e) => {
                        log::warn!("API callback error: {}", e);
                    }
                };
            }
        }

        // Call the on_loop for the client to communicate any state to the server
        {
            let mut func = self.callback.lock().unwrap();
            match func.on_loop() {
                Ok(Some(reply)) => self.outbox.push(reply),
                Ok(None) => {}
                Err(e) => {
                    log::warn!("API on_loop error: {}", e);
                }
            };
        }

        if self.outbox.is_empty() {
            return Ok(StateResponse::None);
        } else {
            return Ok(StateResponse::Reply(state_log!("messages pending")));
        }
    }

    async fn send(&mut self) -> Result<StateResponse, ApiClientError> {
        // self.flush().await.unwrap();
        let pending = std::mem::take(&mut self.outbox);

        for mut msg in pending {
            match self
                .send_and_receive(&mut msg, Duration::from_secs(10), false)
                .await
            {
                Ok(message) => self.inbox.push(message),
                Err(e) => {
                    return Ok(StateResponse::Error(e.to_string()));
                }
            }
        }
        Ok(StateResponse::Reply(state_log!("sent")))
    }

    async fn wait(&mut self) -> Result<StateResponse, ApiClientError> {
        Ok(StateResponse::Reply(nullstr!()))
    }

    /// Triggers state transitions on the client
    async fn trigger(&mut self) {
        self.state = match State::evaluate(self.state.clone(), self).await {
            Ok(s) => s,
            Err(_) => State::Disconnected(state_log!("invalid")),
        };
    }
}

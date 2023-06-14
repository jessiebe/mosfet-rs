use crate::api::{ApiCallbacks, ApiClientError, ConnectionSettings};
use crate::{nullstr, state_log};
use crate::{
    opamp::*,
    opamp::{spec::*, Channel},
    state::*,
};
use async_trait::async_trait;
use futures_util::SinkExt;
use futures_util::StreamExt;
use prost::Message as ProstMessage;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

pub struct WsClient<'a> {
    settings: ConnectionSettings,
    address: url::Url,
    backoff: u32,
    seqno: u64,
    agent_state: RefCell<Option<AgentToServer>>,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    callback: Arc<Mutex<Box<dyn ApiCallbacks + Send + Sync + 'a>>>,
    outbox: Vec<AgentToServer>,
    state: State,
}

impl WsClient<'_> {
    pub fn new(
        settings: ConnectionSettings,
        cb: Box<dyn ApiCallbacks + Send + Sync + '_>,
    ) -> WsClient {
        let path = settings.server_endpoint.clone() + settings.listen_path.as_str();
        let address = url::Url::parse(&path).unwrap();

        WsClient {
            settings,
            address,
            backoff: 0,
            seqno: 0,
            agent_state: RefCell::new(None),
            stream: None,
            callback: Arc::new(Mutex::new(cb)),
            outbox: vec![],
            state: State::Disconnected("".to_string()),
        }
    }

    pub async fn to_sink(&mut self, buf: Message) -> Result<(), ApiClientError> {
        let (mut s, _) = self.stream.as_mut().unwrap().split();
        match s.send(buf).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiClientError::new(
                line!(),
                format!("Websocket send error {}", e).as_str(),
            )),
        }
    }

    async fn flush(&mut self) -> Result<(), ApiClientError> {
        let pending = std::mem::replace(&mut self.outbox, Vec::new());
        let mut capabilities = 0;
        let mut flags = 0;

        if let Some(state) = self.agent_state.borrow().as_ref() {
            capabilities = state.capabilities.clone();
            flags = state.flags.clone();
        } else {
            log::warn!("Missing persistent agent state");
        }

        for mut msg in pending {
            msg.capabilities = capabilities;
            msg.flags = flags;
            self.seqno += 1;
            msg.sequence_num = self.seqno;
            log::trace!("Sending \n: {:#?}", &msg);
            let mut buf = Vec::new();
            buf.reserve(msg.encoded_len());
            msg.encode(&mut buf).unwrap();
            self.to_sink(Message::Binary(buf))
                .await
                .expect("Send failure");
        }
        Ok(())
    }

    pub async fn receive(&mut self) -> Result<Option<Message>, ApiClientError> {
        let (_, mut r) = self.stream.as_mut().unwrap().split();
        match r.next().await {
            Some(Ok(message)) => Ok(Some(message)),
            Some(Err(e)) => Err(ApiClientError {
                code: line!(),
                details: format!("WebSocket receive error {}", e),
            }),
            None => Ok(None),
        }
    }

    fn set_health(&mut self, healthy: bool) {
        // We're now polling. Set our state to healthy
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
        h.start_time_unix_nano = crate::get_time_nanos!() as u64;
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
impl Channel for WsClient<'_> {
    fn get_instance_id(&self) -> &String {
        &self.settings.instance_id
    }

    async fn connect(&mut self) -> Result<StateResponse, ApiClientError> {
        self.set_health(false);
        self.stream = match connect_async(self.address.clone()).await {
            Ok(s) => {
                let (strm, _) = s;
                Some(strm)
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
                        details: format!("Failed to connect to websocket: {}", e),
                    });
                }
                let idle_sec = std::time::Duration::from_secs(2_u64.pow(self.backoff));
                std::thread::sleep(idle_sec);
                return Ok(StateResponse::None);
            }
        };

        log::info!("Websocket connection to server successful");
        Ok(StateResponse::Reply(state_log!("connected")))
    }

    async fn handshake(&mut self) -> Result<StateResponse, ApiClientError> {
        self.set_health(false);
        match self.get_status() {
            Ok(firstmsg) => {
                self.outbox.push(firstmsg);
                Ok(StateResponse::Reply(state_log!("handshake enqueued")))
            }
            Err(e) => Ok(StateResponse::Error(format!(
                "State reporting failed: {}",
                e
            ))),
        }
    }

    async fn poll(&mut self) -> Result<StateResponse, ApiClientError> {
        let _ = self.get_health().is_some_and(|mut x| {
            if x.healthy == false {
                self.set_health(true);
                x.healthy = true;
                // Enqueue one healthy message since we're polling
                self.outbox.push(AgentToServer {
                    instance_uid: self.get_instance_id().clone(),
                    health: Some(x),
                    ..AgentToServer::default()
                });
            }
            true
        });

        // Check if theres anything pending first
        if !self.outbox.is_empty() {
            return Ok(StateResponse::Reply(state_log!("flushing queue")));
        }

        // Check the websocket inbound
        if let Ok(Some(Message::Binary(bytes))) = self.receive().await {
            log::debug!("Received a binary websocket message");
            // NOTE: OpAMP currently has an 8 byte zero header. Skip it to parse the message
            if let Ok(msg) = ServerToAgent::decode(&mut std::io::Cursor::new(&bytes[1..])) {
                log::trace!("Received a ServerToAgent message");
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
                if let Some(_) = &msg.error_response {
                    let mut func = self.callback.lock().unwrap();
                    func.on_error(&msg);
                }

                // Check and report full state
                if msg.flags & (ServerToAgentFlags::ReportFullState as u64) != 0 {
                    // Check our health (match it to the instance_id)
                    if &msg.instance_uid == &self.settings.instance_id {
                        // Report our own health as healthy (we're heartbeating obviously!)
                        self.set_health(true);
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
                    log::trace!("Received a remote config: {:?}", agent_rc);
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
        self.flush().await.unwrap();
        Ok(StateResponse::Reply(state_log!("messages sent")))
    }

    async fn wait(&mut self) -> Result<StateResponse, ApiClientError> {
        Ok(StateResponse::Reply(nullstr!()))
    }

    /// Triggers state transitions on the client
    async fn trigger(&mut self) {
        self.state = match State::evaluate(self.state.clone(), self).await {
            Ok(s) => s,
            Err(_) => State::Disconnected(state_log!("invalid state transition!")),
        };
    }
}

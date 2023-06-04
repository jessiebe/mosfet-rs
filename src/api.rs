#[cfg(feature = "http")]
use crate::httpclient::HttpClient;
use crate::opamp::{spec::*, util::*, Channel};
#[cfg(feature = "websocket")]
use crate::wsclient::WsClient;
use std::{error::Error, fmt};

/// `pub trait ApiCallbacks` is defining a trait that must be implemented by OpAMP clients. It
/// defines a set of methods that an implementing type must provide, which will be called by the `Api`
/// struct during its operation. This allows for customization and extension of the behavior of the
/// `Api` struct without modifying its core implementation.
pub trait ApiCallbacks {
    /// Request the client to report its current configuration
    fn get_configuration(&mut self) -> Result<Option<AgentConfigMap>, ApiClientError>;
    /// Asks the client to report a tuple of (capabilities, flags) for OpAMP
    fn get_features(&mut self) -> (u64, u64);
    /// Primary execution loop of the OpAMP client
    fn on_loop(&mut self) -> Result<Option<AgentToServer>, ApiClientError>;
    /// Reverse reported errors
    fn on_error(&mut self, inbound: &ServerToAgent);
    /// Health check callback for the supervisor to report its (and subagent) health
    fn on_health_check(
        &mut self,
        inbound: &ServerToAgent,
    ) -> Result<Option<AgentToServer>, ApiClientError>;
    fn on_command(
        &mut self,
        inbound: &ServerToAgent,
    ) -> Result<Option<AgentToServer>, ApiClientError>;
    /// Callback that is invoked when the OpAMP server deploys a new config to this node
    fn on_agent_remote_config(
        &mut self,
        inbound: &ServerToAgent,
    ) -> Result<Option<AgentToServer>, ApiClientError>;
    /// Callback for suggesting/altering different connection parameters to the supervisor
    fn on_connection_settings_offers(
        &mut self,
        inbound: &ServerToAgent,
    ) -> Result<Option<AgentToServer>, ApiClientError>;
    /// Reports on packages that are available for the supervisor to download and deploy
    fn on_packages_available(
        &mut self,
        inbound: &ServerToAgent,
    ) -> Result<Option<AgentToServer>, ApiClientError>;
}

/// The above code defines a struct called ConnectionSettings with several fields for server connection
/// and debugging settings.
///
/// Properties:
///
/// * `server_endpoint`: The URL or IP address of the server that the connection will be established
/// with.
/// * `api_key`: The `api_key` property is a string that represents an authentication key used to access
/// a server or API. It is typically used to identify and authorize a user or application to access
/// specific resources or perform certain actions.
/// * `listen_path`: The `listen_path` property is a string that represents the path where the server
/// will listen for incoming requests. This is typically a URL path that is used to route requests to
/// the appropriate endpoint.
/// * `name`: The name property is a string that represents the name of the connection. It
/// could be used to identify the specific connection settings object or to provide a name for the
/// connection settings that is meaningful to the user.
/// * `version`: The `version` property is a string that represents the version of the application or
/// service that is using these connection settings. It can be used to identify which version of the
/// application is running when troubleshooting or debugging issues.
/// * `instance_id`: The `instance_id` property is a ULID for the instance of the
/// application or service that is using these connection settings. It can be used to differentiate
/// between multiple instances running on the same or different machines or environments.
/// * `debugmode`: `debugmode` is a property of type `log::LevelFilter` which is used to specify the
/// level of logging that should be enabled for the connection.
pub struct ConnectionSettings {
    pub server_endpoint: String,
    pub api_key: String,
    pub listen_path: String,
    pub name: String,
    pub version: String,
    pub instance_id: String,
    pub debugmode: log::LevelFilter,
}

#[derive(Debug)]
/// The ApiClientError reports clients to and from the API.
///
/// Properties:
///
/// * `code`: The `code` property is a public unsigned 32-bit integer that represents an error code
/// associated with an `ApiClientError` instance. This code can be used to identify the specific type of
/// error that occurred.
/// * `details`: The `details` property is a `String` that provides additional information about the
/// error that occurred in an `ApiClient`. It can be used to provide more context to the error code and
/// help with debugging.
pub struct ApiClientError {
    pub code: u32,
    pub details: String,
}

impl ApiClientError {
    pub fn new(code: u32, msg: &str) -> ApiClientError {
        ApiClientError {
            code,
            details: format!("Service Error <{}> : {}", &code, &msg),
        }
    }

    pub fn code(&self) -> u32 {
        self.code
    }
}

impl fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ApiClientError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl Default for ConnectionSettings {
    fn default() -> ConnectionSettings {
        ConnectionSettings {
            server_endpoint: "ws://127.0.0.1:4320".to_string(),
            api_key: "".to_string(),
            listen_path: "/v1/opamp".to_string(),
            name: "io.opentelemetry.collector".to_string(),
            version: std::env::var("CARGO_PKG_VERSION").unwrap_or("0.0.1".to_string()),
            instance_id: generate_ulid().to_string(),
            debugmode: log::LevelFilter::Info,
        }
    }
}

pub struct Api<'a> {
    pub client: Box<dyn Channel + 'a>,
}

impl Api<'_> {
    #[cfg(feature = "websocket")]
    pub fn websocket_client(
        settings: ConnectionSettings,
        cb: Box<dyn ApiCallbacks + Send + Sync + '_>,
    ) -> Api {
        Api {
            client: Box::new(WsClient::new(settings, cb)),
        }
    }

    #[cfg(feature = "http")]
    pub fn http_client(
        settings: ConnectionSettings,
        cb: Box<dyn ApiCallbacks + Send + Sync + '_>,
    ) -> Api {
        Api {
            client: Box::new(HttpClient::new(settings, cb)),
        }
    }

    #[cfg(not(feature = "http"))]
    pub fn http_client(_: ConnectionSettings, _: Box<dyn ApiCallbacks + Send + Sync + '_>) -> Api {
        unimplemented!("Requires http feature")
    }

    #[cfg(not(feature = "websocket"))]
    pub fn websocket_client(
        _: ConnectionSettings,
        _: Box<dyn ApiCallbacks + Send + Sync + '_>,
    ) -> Api {
        unimplemented!("Requires websocket feature")
    }

    pub fn new(settings: ConnectionSettings, cb: Box<dyn ApiCallbacks + Send + Sync + '_>) -> Api {
        if settings.server_endpoint.starts_with("http") {
            Self::http_client(settings, cb)
        } else {
            Self::websocket_client(settings, cb)
        }
    }

    pub async fn poll(&mut self) {
        self.client.trigger().await;
    }
}

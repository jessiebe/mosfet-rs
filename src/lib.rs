//! # OpenTelemetry OpAMP Protocol Client Library
//!
//! A standards compliant OpAMP client library for writing reliable observability infrastructure.
//!
//! Otel-opamp-rs is an event-loop based, non-blocking api for writing high performance
//! OpAMP capable supervisors and agents. At a high level, it provides a few major capabilities:
//!
//! * Standards compliant OpAMP client with HTTP and Websocket capabilities
//! * Extra source level components to simplify building supervisors and agents
//! * Supported on Windows and Linux platforms
//! * Thread-safe: Flexibility to operate in a single threaded or multi-threaded mode
//! * Feature flags that let you choose functionality to suit your use case
//!  
//! ## Not supported
//! The following will *not* be supported by this library
//! ​
//! * Tertiary agent functionality (metrics collection, lifecycle management, etc)
//! * Communication mechanism/protocol strictly between the supervisor and agent processes (i.e. not involving OpAMP protocol integration)
//! * Package management (may have a limited pilot implementation in _extras_)
//! * Any scripts/configs supporting the deployment of the supervisor or agent
//! * Deployment options for end clients
//! * Persistence of state in external storage
//! * Authorization or access control of any kind at a protocol level.
//!  
//! # Integrating otel-opamp-rs
//!
//! The easiest way to get started is to include and enable the following features in Cargo.toml
//!
//! ```toml
//! otel-opamp-rs = { version = "0.0.8", features = ["http", "websocket", "extras"] }
//! ```
//!
//! Interfacing code needs to implement the following trait and its callbacks
//!
//! ```
//! pub trait ApiCallbacks {
//!     fn get_configuration(&mut self) -> Result<Option<AgentConfigMap>, ApiClientError>;
//!     fn get_features(&mut self) -> (u64, u64);
//!     fn on_loop(&mut self) -> Result<Option<AgentToServer>, ApiClientError>;
//!     fn on_error(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     );
//!     fn on_health_check(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     ) -> Result<Option<AgentToServer>, ApiClientError>;
//!     fn on_command(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     ) -> Result<Option<AgentToServer>, ApiClientError>;
//!     fn on_agent_remote_config(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     ) -> Result<Option<AgentToServer>, ApiClientError>;
//!     fn on_connection_settings_offers(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     ) -> Result<Option<AgentToServer>, ApiClientError>;
//!     fn on_packages_available(
//!         &mut self,
//!         inbound: &ServerToAgent,
//!     ) -> Result<Option<AgentToServer>, ApiClientError>;
//! }
//! ```
//!
//! To kick-start the API and poll it for data, you can go about it like so:
//!
//! ```
//! pub struct Supervisor {
//! }
//!
//! impl ApiCallbacks for &mut Supervisor {
//!     // .. callback functions
//! }
//!
//! impl Supervisor {
//!     pub fn run() {
//!         // Do your Supervisor/client initialization here
//!
//!         // Get an API handle
//!         let mut handle = Api::new(
//!            ConnectionSettings {
//!                server_endpoint: server.endpoint,
//!                api_key: server.api_key.clone(),
//!                debugmode: args.options.debugmode,
//!                ..Default::default()
//!                }, Box::new(self),
//!             );
//!
//!         // Execution loop
//!         loop {
//!             handle.poll().await;
//!
//!             // Perform other supervisor specific tasks
//!         }
//!     }
//! }
//! ```
//! The API does not enforce any specific polling interval but OpAMP however recommends per 30 seconds
//! Note however that this code performs all OpAMP specific tasks via pure async processing and does
//! not spawn a thread to perform these tasks (primarily because it isnt required). That said, the
//! recommended approach is to do all OpAMP specific processing in the callbacks and defer long running
//! operations to the main run function loop.
//!
//! The API also auto generates a poll message every 60 seconds to the server as required by OpAMP
//!
//! # Under the hood
//!
//! This crate consists of a number of modules that provide a range of functionality
//! essential for implementing OpAMP capable clients. In this section, we will take a brief look at these,
//! summarizing the major APIs and their uses.
//!
//! ### Finite State Machine
//!
//! The library makes use of an FSM to establish various states of operation in the process
//! of connecting, handshaking and processing messages from the server. This mechanism
//! gives it a high degree of predictability and robustness and simplifies debugging immensely.
//!
//! Running the code in debug mode shows state transition messages in detail to give you an
//! insight of what its doing.
//!
//! ## Channel support
//!
//! The FSM requires supported network channels to implement the Channel trait
//!
//! ```
//! pub trait Channel: Send {
//!     fn get_instance_id(&self) -> &String;
//!     // State transition handlers
//!     async fn trigger(&mut self);
//!     async fn connect(&mut self) -> Result<StateResponse, ApiClientError>;
//!     async fn handshake(&mut self) -> Result<StateResponse, ApiClientError>;
//!     async fn poll(&mut self) -> Result<StateResponse, ApiClientError>;
//!     async fn send(&mut self) -> Result<StateResponse, ApiClientError>;
//!     async fn wait(&mut self) -> Result<StateResponse, ApiClientError>;
//! }
//! ```
//!
//! Implementations for HTTP and Websocket are already in place, but e.g. nothing prevents OpAMP over
//! a message queue like MQTT or Kafka turning up in future by implementing this interface for those mechanisms
//!
//! ### HTTP
//!
//! The HTTP mechanism as defined by OpAMP is a half duplex connection. It requires that we poll
//! the server repeatedly and pick up messages on the responses. This also requires us to receive
//! and respond to each incoming message before processing the next one (hence half-duplex).
//!
//! ### Websocket
//!
//! The Websocket connection is stream oriented full duplex. Messages can be pumped into the server
//! serially and responses can be processed similarly. This makes websocket based setups naturally
//! more performant.
//!
//! ## Design philosophy
//!
//! The API was designed on the premise of using threads sparingly (only for on demand lengthy processing)
//! and all other operations are performed asynchronously. This reduces the process footprint to fewer core
//! and allows the API to function more efficiently on devices with more modest processing capabilities but
//! does not sacrifice performance at higher end hardware either.
//!
//! The one caveat is, the API is intended to co-exist around other operations in the run loop that keep the
//! process busy with other tasks and not only busy spinning on the API handle. This gives users a choice
//! on how agressive the polling needs to be (as long as its called at least once every 30 seconds).
//!
//! ## Supported platforms
//!
//! otel-opamp-rs currently guarantees support for the following platforms:
//!
//!  * Linux
//!  * Windows
//!  * MacOS
//!  * BSD and variants
//!
//! Support for running on routers and other small form factor devices have not yet been tested but is on the roadmap.
//!

pub mod api;
pub mod extras;
#[cfg(feature = "http")]
pub mod httpclient;
pub mod opamp;
pub mod state;
#[cfg(feature = "websocket")]
pub mod wsclient;

use crate::api::ApiClientError;
use crate::state::StateResponse;
use async_trait::async_trait;

pub mod spec {
    include!(concat!(env!("OUT_DIR"), "/opamp.proto.rs"));
}

#[async_trait]
/// The `Channel` trait is what different transports would implement to support OpAMP
pub trait Channel: Send {
    fn get_instance_id(&self) -> &String;
    // State transition handlers
    async fn trigger(&mut self);
    async fn connect(&mut self) -> Result<StateResponse, ApiClientError>;
    async fn handshake(&mut self) -> Result<StateResponse, ApiClientError>;
    async fn poll(&mut self) -> Result<StateResponse, ApiClientError>;
    async fn send(&mut self) -> Result<StateResponse, ApiClientError>;
    async fn wait(&mut self) -> Result<StateResponse, ApiClientError>;
}

#[macro_export]
macro_rules! get_time_nanos {
    () => {
        $crate::opamp::util::get_time_nanos()
    };
}

pub mod util {
    use rand::RngCore;
    use ulid::Generator;

    pub fn get_time_nanos() -> u128 {
        let start = std::time::SystemTime::now();
        let since_the_epoch = start
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time error");
        since_the_epoch.as_nanos()
    }

    pub fn generate_ulid() -> ulid::Ulid {
        let dt = std::time::SystemTime::now();
        let mut entropy = [0; 10];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut entropy[6..]);
        let mut gen = Generator::new();
        gen.generate_from_datetime_with_source(dt, &mut rng)
            .unwrap()
    }
}

pub mod defaults {
    use super::spec::*;
    use sysinfo::{System, SystemExt};

    pub fn effective_config() -> EffectiveConfig {
        EffectiveConfig { config_map: None }
    }

    pub fn remote_config_status() -> RemoteConfigStatus {
        RemoteConfigStatus {
            last_remote_config_hash: vec![],
            status: RemoteConfigStatuses::Unset.into(),
            error_message: "".to_string(),
        }
    }

    pub fn package_statuses() -> PackageStatuses {
        PackageStatuses {
            packages: std::collections::HashMap::new(),
            server_provided_all_packages_hash: vec![],
            error_message: "".to_string(),
        }
    }

    pub fn agent_health() -> AgentHealth {
        AgentHealth {
            healthy: false,
            start_time_unix_nano: get_time_nanos!() as u64,
            last_error: "".to_string(),
        }
    }

    pub fn agent_description(name: &str, version: &str) -> AgentDescription {
        let mut identifying_attributes = Vec::new();
        let mut non_identifying_attributes = Vec::new();
        // Identifying attributes
        identifying_attributes.push(KeyValue {
            key: "service.name".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(name.to_string())),
            }),
        });
        identifying_attributes.push(KeyValue {
            key: "service.version".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(version.to_string())),
            }),
        });
        identifying_attributes.push(KeyValue {
            key: "service.instance.id".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(
                    super::util::generate_ulid().to_string(),
                )),
            }),
        });
        // Non-identifying attributes
        let sys = System::new_all();
        non_identifying_attributes.push(KeyValue {
            key: "os.type".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(
                    std::env::consts::OS.to_string(),
                )),
            }),
        });
        non_identifying_attributes.push(KeyValue {
            key: "os.version".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(sys.kernel_version().unwrap())),
            }),
        });
        non_identifying_attributes.push(KeyValue {
            key: "host.name".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(sys.host_name().unwrap())),
            }),
        });

        AgentDescription {
            identifying_attributes,
            non_identifying_attributes,
        }
    }
}

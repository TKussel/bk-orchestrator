use std::{time::Duration, fmt::Display};

use reqwest::{header::{HeaderMap, HeaderValue}, StatusCode};
use serde::{de, Deserializer, Deserialize, Serialize, Serializer};
use tokio::time::sleep;
use uuid::Uuid;

use crate::*;
use crate::error::*;

type BrokerId = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyId {
    proxy: String,
    broker: BrokerId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppId {
    app: String,
    rest: ProxyId,
}

impl ProxyId {
    pub fn get_proxy_id(&self) -> String {
        format!("{}.{}", &self.proxy, &self.broker)
    }
    pub fn get_broker_id(&self) -> String {
        self.broker.clone()
    }
    pub fn new(full: String) -> Result<Self, ExecutorError> {
        let mut components: Vec<String> = full.split(".").map(|x| x.to_string()).collect();
        let rest = components.split_off(1).join(".");
        Ok(ProxyId {
            proxy: components
                .first()
                .cloned()
                .ok_or_else(|| ExecutorError::InvalidBeamId(format!("Invalid ProxyId: {}", full)))?,
            broker: rest,
        })
    }
}

impl AppId {
    pub fn get_app_id(&self) -> String {
        format!("{}.{}", &self.app, &self.rest.get_proxy_id())
    }
    pub fn get_proxy_id(&self) -> String {
        self.rest.get_proxy_id()
    }
    pub fn new(full: String) -> Result<Self, ExecutorError> {
        let mut components: Vec<String> = full.split(".").map(|x| x.to_string()).collect();
        let rest = components.split_off(1).join(".");
        Ok(AppId {
            app: components
                .first()
                .cloned()
                .ok_or_else(|| ExecutorError::InvalidBeamId(format!("Invalid ProxyId: {}", full)))?,
            rest: ProxyId::new(rest)?,
        })
    }
}
impl Display for ProxyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.proxy, self.broker)
    }
}
impl Display for AppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.app, self.rest)
    }
}

impl<'de> serde::Deserialize<'de> for AppId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        AppId::new(s).map_err(de::Error::custom)
    }
}

impl Serialize for AppId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
              {
                  let mut state = String::serialize(&self.to_string(), serializer)?;
                  Ok(state)
              }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BeamTask {
    pub id: Uuid,
    pub from: AppId,
    pub to: Vec<AppId>,
    pub metadata: String,
    pub body: String,
    pub ttl: usize,
    pub failure_strategy: FailureStrategy,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FailureStrategy {
    Retry(Retry),
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Retry {
    pub backoff_millisecs: usize,
    pub max_tries: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BeamResult {
    pub from: AppId,
    pub to: Vec<AppId>,
    pub task: Uuid,
    pub status: Status,
    pub metadata: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Claimed,
    Succeeded,
    TempFailed,
    PermFailed,
}

impl BeamResult {
    pub fn claimed(from: AppId, to: Vec<AppId>, task: Uuid) -> Self {
        Self {
            from,
            to,
            task,
            status: Status::Claimed,
            metadata: "unused".to_owned(),
            body: "unused".to_owned(),
        }
    }
    pub fn succeeded(from: AppId, to: Vec<AppId>, task: Uuid, body: String) -> Self {
        Self {
            from,
            to,
            task,
            status: Status::Succeeded,
            metadata: "unused".to_owned(),
            body,
        }
    }

    pub fn perm_failed(from: AppId, to: Vec<AppId>, task: Uuid, body: String) -> Self {
        Self {
            from,
            to,
            task,
            status: Status::PermFailed,
            metadata: "unused".to_owned(),
            body,
        }
    }
}

pub async fn check_availability(config: &BeamConfig) {
    let mut attempt: usize = 0;

    println!("Check Beam availability...");

    loop {
        let resp = match config.client
            .get(format!("{}v1/health", config.beam_proxy_url))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                println!("Error making request: {:?}", e);
                continue;
            }
        };

        let status_code = resp.status();
        let status_text = status_code.as_str();

        if resp.status().is_success() {
            println!("Beam is available now.");
            break;
        } else if attempt == 10 {
            println!(
                "Beam still not available after {} attempts.",
                10
            );
            break;
        } else {
            println!("Beam still not available, retrying in 3 seconds...");
            sleep(Duration::from_secs(3)).await;
            attempt += 1;
        }
    }
}

pub async fn retrieve_tasks(config: &BeamConfig) -> Result<Vec<BeamTask>, ExecutorError> {
    println!("Retrieve tasks...");

    let mut tasks: Vec<BeamTask> = Vec::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("ApiKey {} {}", config.app_id, config.app_key))
            .map_err(|e| {
                ExecutorError::ConfigurationError(format!(
                    "Cannot assemble authorization header: {}",
                    e
                ))
            })?,
    );

    let url = format!(
        "{}v1/tasks?filter=todo&wait_count=1&wait_time=100",
        config.beam_proxy_url
    );
    let resp = config.client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| ExecutorError::UnableToRetrieveTasks(e))?;

    let status_code = resp.status();
    let status_text = status_code.as_str();
    println!("{status_text}");

    match status_code {
        StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
            tasks = resp
                .json::<Vec<BeamTask>>()
                .await
                .map_err(|e| ExecutorError::UnableToParseTasks(e))?;
        }
        _ => {
            println!("Unable to retrieve tasks: {}", status_code);
            //return error
        }
    }
    Ok(tasks)
}

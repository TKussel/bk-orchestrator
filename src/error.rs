use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Unable to retrieve tasks from Beam")]
    UnableToRetrieveTasks(reqwest::Error),
    #[error("Unable to parse tasks from Beam")]
    UnableToParseTasks(reqwest::Error),
    #[error("Unable to parse workload")]
    UnableToParseWorkload(serde_json::Error),
    #[error("Unable to answer task")]
    UnableToAnswerTask(reqwest::Error),
    #[error("Unable to set proxy settings")]
    InvalidProxyConfig(reqwest::Error),
    #[error("Configuration error")]
    ConfigurationError(String),
    #[error("Invalid BeamID")]
    InvalidBeamId(String),
    #[error("Parsing error")]
    ParsingError(String),
    #[error("Docker API error")]
    DockerError(String),
}
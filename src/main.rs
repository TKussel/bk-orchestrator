mod beam;
mod error;
mod executor;

use std::time::Duration;

use beam::BeamTask;
use error::ExecutorError;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::{Receiver, Sender, self}, time::sleep, io::AsyncWriteExt};

use futures_util::StreamExt;

use reqwest::header::AUTHORIZATION;
use bollard::{Docker, container::{CreateContainerOptions, AttachContainerResults, AttachContainerOptions, RemoveContainerOptions}};
use uuid::Uuid;


#[derive(Debug, Copy, Clone, Hash, Deserialize)]
enum Executor {
    BKExecutor
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WorkflowSteps {
    name: String,
    image: String,
    env: Option<Vec<String>>,
    input: Option<Vec<String>>,
    output: String
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Workload {
    output: Vec<String>,
    steps: Vec<WorkflowSteps>
}

#[derive(Debug, Clone, Deserialize)]
struct ExecutionTask {
    executor: Executor,
    workload: Workload
}

impl TryFrom<BeamTask> for ExecutionTask {
    type Error = ExecutorError;

    fn try_from(value: BeamTask) -> Result<Self, Self::Error> {
        let result: Result<Self, Self::Error> = serde_json::from_str(&value.body).map_err(|e| ExecutorError::ParsingError(e.to_string()));
        result
        // Todo: Get TaskId and put it in the ExecutionTask
    }
}

#[derive(Debug, Clone)]
pub struct BeamConfig {
    app_id: String,
    app_key: String,
    beam_proxy_url: String,
    client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    println!("Starting Executor...");
    let config = BeamConfig {app_id: "executor.tobias-dev.broker".into(), app_key: "Secret".into(), beam_proxy_url: "http://127.0.0.1:8081".into(), client: reqwest::Client::new()};
    let docker =  Docker::connect_with_local_defaults().expect("Cannot initialize docker");
    docker.version().await.expect("Cannot connect to docker");

    let (tx, rx): (Sender<ExecutionTask>, Receiver<ExecutionTask>) = mpsc::channel(1024); 
    let beam_tx = tx.clone();
    let _beam_fetcher = tokio::spawn( async move { fetch_beam_tasks(beam_tx, config)});
    let executor = tokio::spawn(async move { handle_tasks(rx, docker)});
    println!("Started  up, waiting for tasks");
    _ = executor.await;
    println!("This should not be reached");
}

async fn fetch_beam_tasks(tx: Sender<ExecutionTask>, config: BeamConfig) {
    println!("Beam-Connector started");
    loop {
        beam::check_availability(&config).await;
        let Ok(tasks) = beam::retrieve_tasks(&config).await else {
            println!("Cannot retreive Tasks");
            sleep(Duration::from_secs(10)).await;
            continue;
        };
        for task in tasks {
            let Ok(task) = ExecutionTask::try_from(task.clone()) else {
                println!("Error with task {:?}", task);
                continue;
            };
            _ = tx.send(task).await.or_else(|e|{println!("Error: Could not send task to execution handler: {e}");Err(ExecutorError::ParsingError("()".into()))});
        }
    }
}

async fn handle_tasks(mut rx: Receiver<ExecutionTask>, docker: Docker) {
    println!("Executor Handler started");
    loop {
    let task = rx.recv().await;
    if let Some(task) = task {
        let local_docker_handler = docker.clone();
        // Todo: Match on Orchestrator
        tokio::spawn(async move {executor::execute_bk_orchestrator(local_docker_handler, task.workload, uuid::Uuid::new_v4())});
    } else {
        sleep(Duration::from_millis(50)).await;
    };

    }
}



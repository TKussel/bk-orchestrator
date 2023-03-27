mod beam;
mod error;

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
enum Orchestrator {
    BKOrchestrator
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
    orchestrator: Orchestrator,
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
struct Config {
    app_id: String,
    app_key: String,
    beam_proxy_url: String,
    client: reqwest::Client,
    docker: Docker,
}

#[tokio::main]
async fn main() {
    println!("Starting Executor...");
    let config = Config {app_id: "executor.tobias-dev.broker".into(), app_key: "Secret".into(), beam_proxy_url: "http://127.0.0.1:8081".into(), client: reqwest::Client::new(), docker: Docker::connect_with_local_defaults().expect("Cannot initialize docker")};
    config.docker.version().await.expect("Cannot connect to docker");

    let (tx, rx): (Sender<ExecutionTask>, Receiver<ExecutionTask>) = mpsc::channel(1024); 
    let beam_tx = tx.clone();
    let beam_fetcher = tokio::spawn( move || fetch_beam_tasks(beam_tx, &config)());
    // Execute Task
    // Do something with results
}

async fn fetch_beam_tasks(tx: Sender<ExecutionTask>, config: &Config) {
    loop {
        beam::check_availability(config).await;
        let Ok(tasks) = beam::retrieve_tasks(config).await else {
            println!("Cannot retreive Tasks");
            sleep(Duration::from_secs(10));
            continue;
        };
        for task in tasks {
            let Ok(task) = ExecutionTask::try_from(task.clone()) else {
                println!("Error with task {:?}", task);
                continue;
            };
            tx.send(task);
        }
    }
}

async fn handle_tasks(rx: Receiver<ExecutionTask>, config: &Config) {
    loop {
    let task = rx.recv().await;
    if let Some(task) = task {
        match task.orchestrator {
            Orchestrator::BKOrchestrator => tokio::spawn(async move {execute_bk_orchestrator(config, task.workload, uuid::Uuid::new_v4())}),
            _ => Err(ExecutorError::ConfigurationError("Invalid Orchestrator".into()))
        };
    } else {
        sleep(Duration::from_millis(50));
    };
    
    // Wait on rx
    // switch, based on orchestrator
    // spawn orchestrator_wrapper
    }
}

async fn execute_bk_orchestrator(config: &Config, workload: Workload, id: Uuid) -> Result<(), ExecutorError> {
    let container_name = format!("BKOrchestrator-{id}");
    let container_options = CreateContainerOptions {name: &container_name, platform: None};
    let start_options = bollard::container::Config {
        image: Some("samply/bridgehead-executor"),
        attach_stdin: Some(true),
        attach_stderr: Some(true),
        attach_stdout: Some(true),
        tty: Some(true),
        open_stdin: Some(true),
        ..Default::default()
    };

    let id = config.docker.create_container(Some(container_options), start_options).await.map_err(|e|ExecutorError::DockerError(format!("Cannot create container {container_name}: {e}")))?.id;
    config.docker.start_container::<String>(&id, None).await.map_err(|e| ExecutorError::DockerError(format!("Cannot start container: {e}")))?;

    let attach_options = AttachContainerOptions::<String> {
        stdout: Some(true),
        stderr: Some(true),
        stdin: Some(true),
        stream: Some(true),
        ..Default::default()
    };
    let AttachContainerResults { mut output, mut input} =
        config.docker.attach_container(&id, Some(attach_options)).await.map_err(|e|ExecutorError::DockerError(format!("Cannot attach to container {id}: {e}")))?;

    let input_instruction = serde_json::to_string(&workload).map_err(|e|ExecutorError::UnableToParseWorkload(e))?;

    input.write_all(input_instruction.as_bytes()).await.map_err(|e|ExecutorError::DockerError(format!("Cannot write to stdin of container {id}: {e}")))?;
    input.flush();


    while let Some(Ok(msg)) = output.next().await {
        print!("{msg}");
    }

    config.docker.remove_container(&id, Some(RemoveContainerOptions {force: true, ..Default::default()})).await.map_err(|e|ExecutorError::DockerError(format!("Cannot remove container {id}: {e}")));

    Ok(())
}

mod beam;
mod error;
mod docker_executor;
mod workflow;
mod config;
mod banner;
mod logger;

use std::{time::Duration, process::exit};

use config::BeamConfig;
use error::ExecutorError;
use tokio::{sync::mpsc::{Receiver, Sender, self}, time::sleep};

use reqwest::header::AUTHORIZATION;
use bollard::Docker;

use crate::workflow::{ExecutionTask, Executor};
use tracing::{debug, error, warn, info};

#[tokio::main]
async fn main() -> Result<(), ExecutorError> {
    color_eyre::install().map_err(|e|ExecutorError::ConfigurationError(format!("Cannot initialize error printer: {e}")))?;
    if let Err(e) = logger::init_logger() {
        error!("Cannot initalize logger: {}", e);
        exit(1);
    };
    banner::print_banner();

    let config = config::BeamConfig::load()?;

    let (tx, rx): (Sender<ExecutionTask>, Receiver<ExecutionTask>) = mpsc::channel(1024); 
    let beam_tx = tx.clone();
    let _beam_fetcher = tokio::spawn( async move { fetch_beam_tasks(beam_tx, config).await});
    let executor = tokio::spawn(async move { handle_tasks(rx).await});
    _ = executor.await;
    error!("This should not be reached");
    Ok(())
}

async fn fetch_beam_tasks(tx: Sender<ExecutionTask>, config: BeamConfig) {
    debug!("Beam-Connector started");
    loop {
        beam::check_availability(&config).await;
        let Ok(tasks) = beam::retrieve_tasks(&config).await else {
            warn!("Cannot retreive Tasks");
            sleep(Duration::from_secs(10)).await;
            continue;
        };
        for task in tasks {
            let answer = beam::answer_task_success(&task, &config).await;
            if answer.is_err() {
                warn!("Error answering task {:?}", task);
                continue;
            }
            let task = ExecutionTask::try_from(task.clone());
            if task.is_err() {
                warn!("Error in task {:?}", task);
                continue;
            };
            _ = tx.send(task.unwrap()).await.or_else(|e|{error!("Error: Could not send task to execution handler: {e}");Err(ExecutorError::ParsingError("()".into()))});
        }
    }
}

async fn handle_tasks(mut rx: Receiver<ExecutionTask>) {
    debug!("Executor Handler started");
    loop {
    let task = rx.recv().await;
    if let Some(task) = task {
        info!("Got task {:?} in executor", task);
        tokio::spawn(async move {run_orchestrator(task).await});
    } else {
        sleep(Duration::from_millis(50)).await;
    };

    }
}
async fn run_orchestrator(task: ExecutionTask) {
    match task.executor.name {
        Executor::DockerExecutor => {
            debug!("Initializing Docker engine");
            let docker =  Docker::connect_with_local_defaults().expect("Cannot initialize docker");
            let version = docker.version().await.expect("Cannot connect to docker");
            println!("Version: {:?}", version);
            debug!("Starting Docker Job");
            if let Err(err) = docker_executor::execute_docker_orchestrator(docker, task.workflow, uuid::Uuid::new_v4()).await {
                warn!("Error executing task: {}", err);
            }
        },
        _ => {
            warn!("Executor {:?} not implemented", task.executor.name)
        }
    };
}



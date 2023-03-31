mod beam;
mod error;
mod executor;
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

use crate::workflow::ExecutionTask;
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
    let docker =  Docker::connect_with_local_defaults().expect("Cannot initialize docker");
    docker.version().await.expect("Cannot connect to docker");

    let (tx, rx): (Sender<ExecutionTask>, Receiver<ExecutionTask>) = mpsc::channel(1024); 
    let beam_tx = tx.clone();
    let _beam_fetcher = tokio::spawn( async move { fetch_beam_tasks(beam_tx, config)});
    let executor = tokio::spawn(async move { handle_tasks(rx, docker).await});
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
            let Ok(task) = ExecutionTask::try_from(task.clone()) else {
                warn!("Error in task {:?}", task);
                continue;
            };
            _ = tx.send(task).await.or_else(|e|{error!("Error: Could not send task to execution handler: {e}");Err(ExecutorError::ParsingError("()".into()))});
        }
    }
}

async fn handle_tasks(mut rx: Receiver<ExecutionTask>, docker: Docker) {
    debug!("Executor Handler started");
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



use bollard::{Docker, container::{CreateContainerOptions, AttachContainerOptions, AttachContainerResults, RemoveContainerOptions}};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use tracing::debug;

use crate::{error::ExecutorError, workflow::Workflow};

pub(crate) async fn execute_docker_orchestrator(docker: Docker, workflow: Workflow, id: Uuid) -> Result<(), ExecutorError> {
    let container_name = format!("DockerOrchestrator-{id}");
    let container_options = CreateContainerOptions {name: &container_name, platform: None};
    let start_options = bollard::container::Config {
        image: Some("orchestrator-tester:local"),
        attach_stdin: Some(true),
        attach_stderr: Some(true),
        attach_stdout: Some(true),
        tty: Some(true),
        open_stdin: Some(true),
        ..Default::default()
    };

    let id = docker.create_container(Some(container_options), start_options).await.map_err(|e|ExecutorError::DockerError(format!("Cannot create container {container_name}: {e}")))?.id;
    docker.start_container::<String>(&id, None).await.map_err(|e| ExecutorError::DockerError(format!("Cannot start container: {e}")))?;
    debug!("Created container: {:?}", id);

    let attach_options = AttachContainerOptions::<String> {
        stdout: Some(true),
        // stderr: Some(true),
        stdin: Some(true),
        stream: Some(true),
        ..Default::default()
    };
    let AttachContainerResults { mut output, mut input }=
        docker.attach_container(&id, Some(attach_options)).await.map_err(|e|ExecutorError::DockerError(format!("Cannot attach to container {id}: {e}")))?;
    debug!("Attached to container {:?}", id);

    let input_instruction = serde_json::to_string(&workflow).map_err(|e|ExecutorError::UnableToParseWorkload(e))?;
    debug!("Attempting to send to stdin: {}", input_instruction);
    input.write_all(input_instruction.as_bytes()).await.map_err(|e|ExecutorError::DockerError(format!("Cannot write to stdin of container {id}: {e}")))?;
    input.write("\n".as_bytes()).await.map_err(|e|ExecutorError::DockerError(format!("Cannot write newline to stdin of container {id}: {e}")))?;
    input.flush().await.map_err(|e|ExecutorError::DockerError(format!("Cannot flush stdin: {}", e)))?;
    input.shutdown().await.map_err(|e|ExecutorError::DockerError(format!("Cannot close stdin: {}", e)))?;
    debug!("Closed stream, written to stdin.");

    debug!("Attempting to read from stdout");
    while let Some(Ok(msg)) = output.next().await {
        print!("{msg}");
    }
    
    debug!("EOS from stdout, removing container");
    docker.remove_container(&id, Some(RemoveContainerOptions {force: true, ..Default::default()})).await.map_err(|e|ExecutorError::DockerError(format!("Cannot remove container {id}: {e}")))?;
    debug!("Container removed");

    Ok(())
}

use bollard::{Docker, container::{CreateContainerOptions, AttachContainerOptions, AttachContainerResults, RemoveContainerOptions}, image::ListImagesOptions};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use tracing::{debug};

use crate::{error::ExecutorError, workflow::Workflow};

pub(crate) async fn execute_docker_orchestrator(docker: Docker, workflow: Workflow, id: Uuid) -> Result<(), ExecutorError> {
    let container_name = format!("DockerOrchestrator-{id}");
    let container_options = CreateContainerOptions {name: &container_name, platform: None};
    let start_options = bollard::container::Config {
        image: Some("hello-world"),
        attach_stdin: Some(true),
        attach_stderr: Some(true),
        attach_stdout: Some(true),
        tty: Some(true),
        open_stdin: Some(true),
        ..Default::default()
    };

    debug!("Before creation." );
    let images = &docker.list_images(Some(ListImagesOptions::<String> {all: true, ..Default::default()})).await.unwrap();
    for image in images {
        debug!("-> {:?}", image);
    }
    let id = docker.create_container(Some(container_options), start_options).await.map_err(|e|ExecutorError::DockerError(format!("Cannot create container {container_name}: {e}")))?.id;
    docker.start_container::<String>(&id, None).await.map_err(|e| ExecutorError::DockerError(format!("Cannot start container: {e}")))?;
    debug!("After creation. Id: {:?}", id);

    let attach_options = AttachContainerOptions::<String> {
        stdout: Some(true),
        stderr: Some(true),
        stdin: Some(true),
        stream: Some(true),
        ..Default::default()
    };
    let AttachContainerResults { mut output, mut input} =
        docker.attach_container(&id, Some(attach_options)).await.map_err(|e|ExecutorError::DockerError(format!("Cannot attach to container {id}: {e}")))?;
    debug!("After Attach");

    let input_instruction = serde_json::to_string(&workflow).map_err(|e|ExecutorError::UnableToParseWorkload(e))?;

    input.write_all(input_instruction.as_bytes()).await.map_err(|e|ExecutorError::DockerError(format!("Cannot write to stdin of container {id}: {e}")))?;
    let _ = input.flush().await;
     debug!("After input");


    while let Some(Ok(msg)) = output.next().await {
        print!("{msg}");
    }

    docker.remove_container(&id, Some(RemoveContainerOptions {force: true, ..Default::default()})).await.map_err(|e|ExecutorError::DockerError(format!("Cannot remove container {id}: {e}")))?;
    debug!("After remove");

    Ok(())
}

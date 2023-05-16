use serde::{Deserialize, Serialize};

use crate::{beam::BeamTask, error::ExecutorError};


#[derive(Debug, Copy, Clone, Hash, Deserialize)]
pub(crate) enum Executor {
    DockerExecutor,
    HPCExecutor

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkflowSteps {
    name: String,
    image: String,
    env: Option<Vec<String>>,
    input: Option<Vec<String>>,
    output: String
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Workflow {
    output: Vec<String>,
    steps: Vec<WorkflowSteps>
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExecutorInfo{
    pub name: Executor,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExecutionTask {
    pub executor: ExecutorInfo,
    pub workflow: Workflow
}

impl TryFrom<BeamTask> for ExecutionTask {
    type Error = ExecutorError;

    fn try_from(value: BeamTask) -> Result<Self, Self::Error> {
        let result: Result<Self, Self::Error> = serde_json::from_str(&value.body).map_err(|e| ExecutorError::ParsingError(e.to_string()));
        result
        // Todo: Get TaskId and put it in the ExecutionTask
    }
}

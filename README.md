# Bridgehead Orchestrator

This is an early prototype for a workflow orchestrator. It queries Samply.Beam for workflows and uses Docker to execute the chosen workflow executor. Then, it sends the workflow steps to the container's stdin and reads from its stdout.

This is very early undocumented, not for public use.

# Context

[Context](context.md) defines the working vocabulary for `mpsc-log`.

## Terms

| Term                         | Definition                                                                                                                                                           |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active log                   | The JSON Lines file named on the command line. Successful invocations append new records to this file unless rotation runs first.                                    |
| Agent workflow               | A workflow that launches one or more autonomous agents or helper processes that can invoke command-line tools.                                                       |
| Audit yield                  | The distribution of audit findings by severity, source, and remediation lane.                                                                                        |
| CodeRabbit attempt           | One attempt to run CodeRabbit review, including its start time, result, wait time, retry status, and whether the review was deferred.                                |
| Defect escape                | A defect discovered after merge by later work, dogfooding, Continuous Integration (CI), or users.                                                                    |
| Journal                      | A shared JSON Lines file used as an append-only record of workflow events.                                                                                           |
| Lock timeout                 | The maximum time an invocation waits for the journal lock before failing with a timeout diagnostic.                                                                  |
| Object path                  | A `jo`-inspired field path that writes a value into a nested JSON object rather than into a literal top-level key.                                                   |
| Open Dynamic Workflows (ODW) | The workflow runtime used by `df12-build` to coordinate multi-agent roadmap execution.                                                                               |
| Record                       | One JSON object serialized as one JSON Lines value and terminated with `\n`.                                                                                         |
| Remediation lane             | The route assigned to a review or audit follow-up, such as addendum, step task, later roadmap step, or dropped.                                                      |
| Review round                 | One pass through a review loop, including any blocking findings and subsequent fix attempt.                                                                          |
| Rotated log                  | A previous active log generation retained under a numbered or scheduled filename, optionally compressed with gzip.                                                   |
| Scheduled break              | A configured UTC time boundary, either hourly, daily, or weekly, that forces the current active log segment to rotate on the next invocation.                        |
| Sidecar configuration        | The TOML configuration file derived from the active log path by replacing the filename extension with `.toml`.                                                       |
| Type coercion                | Conversion of a CLI value to a JSON string, number, boolean, null, object, or array according to explicit flags, sidecar schema, or default `jo`-inspired inference. |
| Workflow sidecar directory   | The caller-owned directory where a workflow stores run artefacts, including the `mpsc-log` journal chosen by that workflow.                                          |

## Naming

- Use "journal" for the shared JSON Lines log written by `mpsc-log`.
- Use "sidecar configuration" for the `.toml` file next to the journal.
- Use "workflow sidecar directory" for the df12-build ODW artefact directory.

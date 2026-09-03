# Operations

## Safe execution

Run `process-diagnostics run -- ./command --flag` when the operator intends to
execute that exact program. The arguments are passed directly to the child.
There is no implicit shell expansion or pipeline handling. Use
`--clear-env` and repeated `--env KEY=VALUE` when a minimal explicit
environment is required.

Set `--timeout-ms` for a bounded invocation. The CLI caps it at ten minutes.
The default new-process-group policy and terminate-then-kill cancellation
policy are intended to limit children started by the invocation on Linux.

## Reports and retention

JSON is the default output. Save it to a file and run
`process-diagnostics replay transcript.json` to inspect the recorded facts
without rerunning the command. Text output reports command, outcome, duration,
stream sizes, truncation, exit code, signal, and spawn error fields.

Transcripts can contain command arguments, explicit environment values, local
paths, and child output. Store them under the local access and retention
policy.

## Troubleshooting

- `spawn_error` means the operating system could not start the supplied
  program.
- `nonzero` records a normal process exit with a nonzero code.
- `signaled` records a Unix signal when the exit status exposes one.
- `timeout` means the deadline won before normal completion and the configured
  stop policy ran.
- `cancelled` means the supplied cancellation flag or CLI timer won before
  normal completion.
- `stdout_truncated` or `stderr_truncated` means the stream exceeded its
  retained bound. The reader continued draining the pipe.

## Recovery

The tool does not reverse child side effects. If a timeout leaves an external
daemon or detached descendant, use the operating system's own process tools
under the operator's change policy. Replay is read-only and does not execute
the saved command.

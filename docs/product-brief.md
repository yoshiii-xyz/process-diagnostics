# Product brief

process-diagnostics records enough local process state to explain output,
cancellation, process groups, signals, timeouts, and exit status without
pretending to be a task runner.

Target users are maintainers of command-line tools and build diagnostics who
need a bounded, binary-safe transcript of one exact invocation.

The first commands are:

```text
process-diagnostics run -- ./command --flag value
process-diagnostics replay transcript.json
```

The switching wedge is a versioned transcript that preserves the command
boundary, environment policy, both output streams, control policy, and exit
classification in one inspectable artifact.

Evidence and inference are separate. Rust process API documentation linked in
[`docs/research.md`](research.md) supports the documented command, child,
status, and pre-exec APIs. The transcript fields, bounds, and Linux process
group behavior are implementation choices and release-tested observations,
not claims about every operating system.

Non-goals are shell interpretation, task scheduling, pipelines, PTYs,
cross-platform process-tree guarantees, sandboxing, and hosted execution.

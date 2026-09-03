# Limits and non-goals

The 0.1.0 MVP has these limits:

- Linux is the only platform covered by release evidence.
- The tool runs one exact program and argument vector. It does not provide a
  shell, task runner, command pipeline, scheduler, or supervisor API.
- stdout and stderr are each capped at 8 MiB. Transcript JSON is capped at
  24 MiB.
- The default process group policy is implemented on Unix. Windows job
  objects, macOS-specific group behavior, PTYs, and cross-platform signal
  semantics are extension points without release claims.
- A process can create grandchildren that escape a process group, daemonize,
  or change its own signal behavior. The tool does not promise perfect tree
  accounting or cleanup of arbitrary external descendants.
- Timeout and duration measurements use local wall-clock monotonic intervals
  and are affected by scheduling and system load.
- Inherited environment values are intentionally not copied into transcripts.
  Explicit values supplied in `ProcessSpec` or with `--env` can be sensitive.
- The tool does not sandbox the child, inspect its filesystem, or undo side
  effects.

The project is not a build system, command orchestrator, shell replacement,
container runtime, or security sandbox.

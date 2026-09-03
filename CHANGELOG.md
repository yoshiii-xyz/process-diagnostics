# Changelog

## 0.1.0

- Added exact argv execution without an implicit shell or task runner.
- Added bounded concurrent stdout and stderr capture with binary-safe JSON.
- Added process-group timeout and cancellation policies, exit and signal
  classification, explicit environment handling, and replayable transcripts.
- Added Linux-first tests and fuzz coverage for transcript decoding.

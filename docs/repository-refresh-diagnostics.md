# Diagnosing long-running repository work

Logs are appended to `data/repository-refresh-YYYY-MM-DD.log` beside the running
executable. These events do not depend on the temporary branch-switch trace window.

Remote operation records include `pid`, `task_id`, quoted `root`, `kind`, `stage`,
`outcome` and `total_ms`. Match a task by **pid + task_id**, not by the global
`schedule.update blocked` count. Concurrent repositories keep separate identities.

Background fetch stages:

1. `operation.queued`: the UI accepted the task.
2. `scheduler_wait`: time waiting for exclusive repository access.
3. `git_fetch`: the actual Git fetch call, excluding queueing and later probes.
4. `local_probe` and `tag_probe`: local post-fetch checks.
5. `core_snapshot`: a full local reload, only if those checks detect a change.
   Reuses the status/configuration probe and tag list obtained in steps 4, rather
   than repeating those reads.
6. `operation.worker.finished`: the worker is ready to send its result.
7. `operation.received`: the UI consumed it; the gap from worker completion
   identifies delayed result polling.
8. `operation.ui.complete` and `operation.finished`: completion handling is done.

Manual remote operations use the same sequence with `git_action` for the Git call.
Stages log a start before executing and an end with their own `stage_ms`. An error
or panic records `error` or `aborted`; a lost receiver records `disconnected`.
An unfinished stage has a start without an end if the task is still running or
the process was terminated. Stages taking at least five seconds have `slow=true`.

Snapshot refreshes log their task identity and dispatch/queue duration, then
`local_probe` and/or `core_snapshot`. An unchanged probe finishes without sending
a new snapshot. Full snapshot delivery is followed by root-scoped
`snapshot.ui.received` and `snapshot.ui.complete` records for UI handling duration.
Coalesced or cancelled queued requests are explicitly marked, rather than left
looking like pending workers.

`blocked=1` only means one repository is protected; it is not a fetch stopwatch.
Use the stage times to distinguish scheduler contention, Git work, local checks
and UI handling. A long `git_fetch` stage alone still does not distinguish server
latency from connection setup or local Git processing.

Task events are queued for a background writer, retaining their original timestamps.
The bounded queue never waits for disk I/O; overflow is reported as
`operation.logs.dropped`. A process exit may lose the last queued events.
Only task boundaries produce these records. There is no per-frame logging, remote
URL, command stderr, token or file-content logging in this instrumentation.

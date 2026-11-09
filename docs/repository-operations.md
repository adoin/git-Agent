# Repository task ownership

Repository operations live in `RepositoryOperationState`, keyed by
`repo_state_key`. Receivers, busy flags, checkout targets, credential retries,
post-pull push plans, pending recovery prompts and remote refresh deadlines move
together. Switching tabs must preserve this state until the operation and its
required reload finish.

`RepositoryOperations.current` is a projection for the UI or one repository's
completion handlers. `select` parks the previous state without dropping its
receivers. `poll_repository_operations` visits only states with pending tasks and
restores the selected repository afterward. A completion handler returning early
must not skip other repositories' polling or the projection restoration.

UI busy checks use `active_repository_operation`; callbacks and reload completion
use the operation's explicit root. A root-explicit chained action must enter its
owner's state and restore the previous projection without changing the active tab.
Background completion errors stay with their owner until it is viewed, and must
not replace another repository's errors or toasts. Read-only UI tasks may be
invalidated on navigation; mutation receivers must remain owned.

Only repositories activated in this session enter the automatic refresh schedule.
The active repository uses the active interval; visited background repositories
use the slower, staggered interval. Unchanged fingerprint probes avoid full reads.
Closing a tab or disabling automatic refresh removes queued automatic reads, while
explicit reads and running operations retain their receivers. Opening a repository
does not itself initiate a pull or push. Background fetch keeps the existing
global and per-repository opt-in settings and a separate deadline per repository.

The scheduler allows at most three exclusive repository operations and two reads;
only one background read runs at a time. Admission waits belong to the queued
operation's repository. A slow network operation does not globally disable other
repositories. A long duration alone is not evidence that a valid fetch or push
should be terminated.

Disabled toolbar controls keep an opaque `noninteractive.weak_bg_fill`: egui uses
it as the painter's fade target and interprets transparent as invisible. Disabling
a button must preserve its caption and icon while preventing activation.

Regression coverage must include independent receivers after navigation, failure
or disconnect ownership, root-specific continuations, final reload gate release,
Windows path spellings, bounded concurrency and disabled-control rendering. Keep
the title-bar dragging constraints in `top-bar-window-drag.md` intact.

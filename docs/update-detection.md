# Update detection

Update checks run in the existing owned background update task. First request
the latest release through the anonymous GitHub REST endpoint. Only if that check
fails, try the locally installed GitHub CLI:

1. Run `gh auth status --active --hostname github.com` to check CLI availability
   and its active GitHub.com login. Allow up to 10 seconds.
2. If authenticated, run `gh api --hostname github.com --method GET
   repos/adoin/git-Agent/releases/latest`. Allow up to 20 seconds.
3. Parse either source with the same version comparison and platform selection.

An available CLI without a valid login does not trigger an anonymous CLI request.
No CLI subprocess runs after a successful REST check. Commands are noninteractive,
with closed stdin and hidden Windows consoles. Tokens remain managed by `gh`;
the updater never requests or stores them. Do not forward raw CLI account output
or debug diagnostics into the application error dialog.

CLI stdout is drained concurrently and capped at 2 MiB. Timeout paths terminate
and reap the owned process. If both sources fail, preserve the original REST error
and add the CLI fallback failure reason. This fallback can help when anonymous
IP-based API limits are exhausted; it does not supply an alternate network route
or bypass authenticated-account limits.

Installer downloads continue through the existing official GitHub release asset
validation and user-triggered installation flow. This change does not add a CLI
download fallback, login flow or alternate file host.

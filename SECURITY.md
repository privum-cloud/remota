# Security Policy

Remota is a remote-access tool: it holds credentials and brokers connections to machines that
matter. Security reports are welcome and taken seriously.

## Reporting a vulnerability

**Please report privately, through GitHub's private vulnerability reporting:**
[github.com/privum-cloud/remota/security/advisories/new](https://github.com/privum-cloud/remota/security/advisories/new)
(the **Security** tab → *Report a vulnerability*). That channel is private to the maintainers and
lets us credit you and publish an advisory once a fix ships.

Please do **not** open a public issue for an unfixed vulnerability.

A useful report includes:

- the component (desktop app, `server/relay`, `server/agent`) and the affected file/function;
- the commit or release you looked at;
- what an attacker can actually do, and what they need first (network position, a valid token, a
  local account, an unlocked vault…);
- a proof of concept if you have one — and if you don't, say so plainly. A well-scoped source
  review is welcome; please don't present an unverified inference as a demonstrated exploit.

We aim to acknowledge a report within a few days. Remota is maintained by a small team, so please
allow reasonable time for a fix before public disclosure. We'll credit you by name in the release
notes and the advisory unless you'd rather stay anonymous.

Remota is open source (AGPL-3.0-only) — if you'd like to fix what you found, a pull request is very
welcome. For an unfixed vulnerability, report it privately first and we'll coordinate the PR.

## Supported versions

Fixes land on `main` and go out in the next release. Only the latest release is supported; there
are no backports to older 0.1.x versions.

## Scope

In scope: the desktop app (encrypted vault, local WebSocket gateway, SSH/RDP/VNC/Telnet
transports), the self-hosted relay (`server/relay`), the agent (`server/agent`), and the
deployment material under `server/deploy/`.

## Known limitations — please don't report these as new

These are documented, deliberate properties of the current release, not undiscovered bugs. They
are on the roadmap; a report that only restates one of them will be closed as known. What *is*
useful is a concrete attack that these limitations enable in a **documented, recommended**
configuration.

- **`POST /session` and `GET /agents` on the relay have no authentication of their own.** They are
  gated at the reverse proxy by source IP (the shipped `Caddyfile` fails closed). Per-user
  authentication is planned.
- **No per-device authorization.** An operator who can reach `POST /session` can broker a tunnel to
  any registered agent's target port; knowing the device ID is enough. A per-device password /
  accept prompt is planned.
- **SSH host keys are accepted without verification** (no `known_hosts`) — in the app and on the
  jump-host leg. This is a known gap, not a design goal.
- **The RDP gateway does not validate the Windows host's certificate.** It forwards the server
  certificate chain to the `ironrdp-web` client, which performs CredSSP channel binding.
- **`export_connections` writes plaintext JSON**, including passwords. It is an explicit, operator
  initiated export.
- **Vault credentials are held as plain `String` in memory** and are not zeroized on lock.
- **A brokered relay session whose two legs never pair is kept until the relay restarts.** Its
  single-use token stays valid, and the session map grows. Reaching it still requires the
  session's UUIDv4 id, which is only sent to the app and the agent.
- Attacks that require an attacker who already has local code execution as the user, or a vault
  that is already unlocked, are out of scope.

## Deployment hardening

If you self-host the relay, read `server/deploy/README.md` before exposing it. In particular: set a
strong `REMOTA_ENROLL_TOKEN` (the relay refuses to start without one) and edit the operator IP
allowlist in `Caddyfile` — as shipped, the brokering endpoints answer `403` to everyone.

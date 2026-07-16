<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="Remota logo — a red R over the Italian tricolore">

# Remota

**Open-source remote connection manager for Linux — SSH, RDP, VNC and Telnet in one fast, organized, encrypted app.**

A free, self-hosted alternative to mRemoteNG (built for Linux) and to cloud remote‑access tools — keep all your servers, PCs and devices in one place, with no third‑party cloud.

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](./LICENSE)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-333.svg)
![Built with Tauri + Rust + React](https://img.shields.io/badge/Built%20with-Tauri%20·%20Rust%20·%20React-24c8db.svg)

Made with ❤️ in Italy by **[Privum Cloud »](https://privum.cloud)**

</div>

---

Remota is a native Linux desktop app to **manage and open remote sessions** across many
protocols from a single window. Organize hundreds of hosts into folders, inherit credentials,
tunnel through jump hosts, and reach machines behind NAT through **your own relay** — all
protected by a local encrypted vault. It's for developers, sysadmins, homelabbers, IT teams,
students — anyone who connects to more than one remote machine.

## Why Remota

[mRemoteNG](https://mremoteng.org/) is a much-loved multi-protocol connection manager — but it
is **Windows-only**, and there is no official Linux version. Remota was born to fill that gap:
to bring the same tabbed, folder-organized, multi-protocol workflow to **Linux**, as a fast
native app that is **fully open source**. If you've been searching for *"mRemoteNG for Linux"*
or a *"Linux remote connection manager"*, that's exactly what Remota is — plus an encrypted
vault and self-hosted access to machines behind NAT.

## Screenshots

| Encrypted vault | Organize & configure |
|---|---|
| ![Remota vault unlock screen with master password](images/remota_vault_login.png) | ![Remota connection tree with folders and the new-connection editor](images/remota_connections_setup.png) |
| **Live SSH terminal** | **Folders with icons** |
| ![Remota live SSH terminal with tabs and right-click menu](images/remota_ssh_connection.png) | ![Remota folder editor with icon picker and inherited defaults](images/remota_folder_edit.png) |

## Features

- **Multi-protocol** — SSH, RDP, VNC and Telnet, each in its own tab.
- **Organized** — folders and subfolders; drag-and-drop; expand/collapse; custom icons.
- **Credential inheritance** — set a username / password / SSH key / jump host on a folder and
  every connection inside inherits it (override per host).
- **Encrypted vault** — your connections and secrets are stored locally, encrypted with
  **AES-256-GCM + Argon2id**, unlocked by a master password. Nothing leaves your machine.
- **Jump hosts (SSH ProxyJump)** — reach private hosts through a bastion, per host or per folder.
- **Reach machines behind NAT** — run a small agent on a remote machine and connect to it through
  your **self-hosted relay** — a private alternative to AnyDesk / remote.it / RustDesk, no cloud.
- **SSH keys** — password or private-key auth, including on the jump host.
- **Import** — bring your existing setup in from **mRemoteNG** (`confCons.xml`) or Remota JSON.
- **Real terminal** — xterm.js with a properly-sized PTY, so `vim`, `k9s`, `htop` fill the pane
  and follow window resizes.
- **Recycle bin** — deleting moves items to a Trash you can restore from.
- **Native & fast** — a small Rust/Tauri binary, not a bundled browser.

## Install

Remota runs on Linux (x86_64). Pick your format:

### Debian / Ubuntu (`.deb`)

Download [`packages/Remota_0.1.2_amd64.deb`](packages/Remota_0.1.2_amd64.deb) (or from the
[Releases](../../releases) page), then:

```bash
sudo apt install ./Remota_0.1.2_amd64.deb
```

### Fedora / RHEL / openSUSE (`.rpm`)

Download [`packages/Remota-0.1.2-1.x86_64.rpm`](packages/Remota-0.1.2-1.x86_64.rpm), then:

```bash
sudo dnf install ./Remota-0.1.2-1.x86_64.rpm
```

### Any Linux (`.AppImage`)

Grab the `.AppImage` from the [Releases](../../releases) page (portable — no install), then:

```bash
chmod +x Remota_*.AppImage
./Remota_*.AppImage
```

### Build from source

Requires [Rust](https://rustup.rs), Node.js 18+, and the
[Tauri Linux prerequisites](https://tauri.app/start/prerequisites/) (WebKitGTK, etc.).

```bash
git clone https://github.com/privum-cloud/remota.git
cd remota
npm install
npm run tauri build          # bundles land in src-tauri/target/release/bundle/
# or run it live during development:
npm run tauri dev
```

First launch asks you to set a **master password** — it encrypts your vault. Then add a
connection (or import from mRemoteNG) and double-click to open a session.

## Reach machines behind NAT (self-hosted)

Remota can reach a machine on any network — behind NAT, no public IP, no open ports — by running
a tiny **agent** on it that dials out to a **relay** you host. The operator connects to the
device through the relay from the app. See **[remota-client](https://github.com/privum-cloud/remota-client)**
for the agent and installer.

## Architecture

- **Desktop app** — [Tauri](https://tauri.app) v2 (Rust) + React + TypeScript.
- **Local gateway** — a WebSocket bridge bound to `127.0.0.1` with single-use per-session tokens:
  a raw `WS↔TCP` bridge for VNC/Telnet, native SSH via `russh`, and an RDCleanPath proxy for RDP.
- **Renderers** — xterm.js (SSH/Telnet), noVNC (VNC), IronRDP `ironrdp-web`/WASM (RDP).
- **Vault** — portable encrypted file (AES-256-GCM + Argon2id).
- **Relay & agent** — Rust, WSS, outbound-only from the agent; TLS terminated at a reverse proxy.

## Contributing

Issues and pull requests are welcome. Remota is a clean-room project — please don't paste code
from other remote-connection managers.

## License

Licensed under the **GNU Affero General Public License v3.0** — see [`LICENSE`](./LICENSE).

## Attribution

Remota is an independent, clean-room project inspired by the workflow of
[mRemoteNG](https://mremoteng.org/). It shares no code with mRemoteNG.

---

<div align="center">

Built and maintained by **[Privum Cloud](https://privum.cloud)** — Kubernetes, DevSecOps & 24/7 SRE.

</div>

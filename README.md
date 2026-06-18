# Remota

**An open-source, cross-platform remote connection manager for Linux.**

Remota is a desktop application for managing and opening remote sessions across
multiple protocols from a single, organized interface — built for Linux first.

## Protocols (v1)

- **SSH**
- **RDP** (with NLA / CredSSP)
- **VNC**
- **Telnet**

## Architecture

- **Desktop app**: [Tauri](https://tauri.app) v2 (Rust shell) + React + TypeScript.
- **Frame transport**: a local WebSocket gateway bound to `127.0.0.1` with
  single-use per-session tokens — a raw `WS↔TCP` bridge for VNC/Telnet/SSH, and
  an RDCleanPath proxy for RDP (TLS terminates at the gateway).
- **Renderers**: noVNC (VNC), IronRDP (`ironrdp-web`/WASM, for RDP), xterm.js
  (SSH/Telnet).
- **Credentials**: a portable encrypted vault (AES-256-GCM + Argon2id) unlocked
  by a master password.

## Status

Early development. Milestone **M0** (transport + RDP spike) is in progress.

## License

Licensed under the **GNU Affero General Public License v3.0** — see
[`LICENSE`](./LICENSE).

## Attribution

Remota is an independent, clean-room project inspired by the workflow of
[mRemoteNG](https://mremoteng.org/). It shares no code with mRemoteNG.

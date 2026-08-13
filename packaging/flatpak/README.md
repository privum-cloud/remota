# Flathub submission for Remota

This folder holds everything needed to publish Remota on **[Flathub](https://flathub.org)**,
so it shows up in GNOME Software / KDE Discover on every Linux distro.

- `cloud.privum.remota.yml` — Flatpak manifest (builds from the released `.deb`).
- `cloud.privum.remota.metainfo.xml` — AppStream metadata (name, screenshots, release notes).

The app-id `cloud.privum.remota` matches the `privum.cloud` domain, which Flathub can verify
(you own the domain), so no `io.github.*` fallback is needed.

> **Status:** prepared but **not yet build-tested** — `flatpak-builder` isn't installed on the
> current host. Do the local build below before opening the Flathub PR. Never submit a manifest
> you haven't built.

## 1. One-time local setup

```bash
sudo apt install flatpak flatpak-builder
flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//50 org.gnome.Sdk//50
```

## 2. Build and test locally

```bash
cd packaging/flatpak
flatpak-builder --user --force-clean --install build-dir cloud.privum.remota.yml
flatpak run cloud.privum.remota          # confirm it launches, SSH/RDP work
```

Validate the metadata (Flathub runs this too):

```bash
flatpak run --command=appstreamcli org.gnome.Sdk//50 validate cloud.privum.remota.metainfo.xml
```

If WebKitGTK/graphics misbehave, adjust `runtime-version` (try `'48'`) or `finish-args`, then rebuild.

## 3. Submit to Flathub

1. Fork <https://github.com/flathub/flathub> (do **not** clone the huge history: use
   `git clone --depth=1 -b new-pr git@github.com:<you>/flathub`).
2. On a new branch off `new-pr`, add `cloud.privum.remota.yml` **and**
   `cloud.privum.remota.metainfo.xml` at the repo root.
3. Open a PR against the `new-pr` branch. The Flathub build bot compiles it and reports back;
   a reviewer then approves. Once merged, Remota gets its own `flathub/cloud.privum.remota` repo.

Docs: <https://docs.flathub.org/docs/for-app-authors/submission>

## Keeping it updated

On each new Remota release, bump the `url` + `sha256` in the manifest and add a `<release>` entry
to the metainfo, then push to the app's Flathub repo. (Can be automated later with
`flatpak-external-data-checker`.)

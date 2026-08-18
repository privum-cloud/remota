# AUR package (remota-bin)

This packages the released `.deb` for Arch and derivatives, so Arch users can
install Remota with `yay -S remota-bin` (or paru, or plain `makepkg`).

It's a `-bin` package: it downloads `Remota_<ver>_amd64.deb` from the GitHub
release and unpacks it. No compiling. Depends only on `webkit2gtk-4.1` and `gtk3`,
which is what the app actually needs at runtime.

## Try it locally (on an Arch box)

```bash
cd packaging/aur
makepkg -si          # builds and installs
remota               # run it
```

## Publish it to the AUR

You need an AUR account with an SSH key added (https://aur.archlinux.org →
My Account → SSH Public Key).

```bash
git clone ssh://aur@aur.archlinux.org/remota-bin.git
cd remota-bin
cp /path/to/packaging/aur/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO      # regenerate to be safe
git add PKGBUILD .SRCINFO
git commit -m "Initial import: remota-bin 0.1.5"
git push
```

That's it — it shows up on the AUR right away.

## On each new release

Bump `pkgver`, refresh the checksum, regenerate `.SRCINFO`, push:

```bash
updpkgsums                             # updates sha256sums from the new .deb
makepkg --printsrcinfo > .SRCINFO
git commit -am "remota-bin X.Y.Z" && git push
```

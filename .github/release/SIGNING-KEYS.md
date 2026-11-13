# Release signing keys

Every key a SugarSubstitute release is signed with is a secret of the protected GitHub
environment `release`. CI builds and signs every release; no release step runs on a
maintainer's machine. The keys are created once, offline, by the key ceremony.

## Secrets

| Secret                 | Signs                                                                                                                  | Read by                                                                               | Signature lifetime                                       | Rotation                                                                                    |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `UPDATE_ROOT_KEY`      | `root.json`, the trust anchor installations ship with (`.github/update-repository/root.json`)                          | `substitute-update-repository.yml`, `rotate-root` only                                | 1 year                                                   | Dispatch `rotate-root` before expiry (the job warns 60 days ahead).                         |
| `UPDATE_TARGETS_KEY`   | `targets.json`: offered packages and channel descriptors                                                               | `substitute-update-repository.yml`: `release`, `resign`, `rotate-root`                | 90 days, re-signed at each release and monthly           | Through `rotate-root`, which lists the online keys it is given.                             |
| `UPDATE_SNAPSHOT_KEY`  | `snapshot.json`                                                                                                        | same as targets                                                                       | 90 days, re-signed at each release and monthly           | Through `rotate-root`.                                                                      |
| `UPDATE_TIMESTAMP_KEY` | `timestamp.json`                                                                                                       | `substitute-update-repository.yml`, every operation                                   | 14 days, re-signed daily                                 | Through `rotate-root`.                                                                      |
| `RELEASE_MANIFEST_KEY` | `sugarsubstitute-release-v1.manifest`, which binds the shipped host, crash client and crash handler images and symbols | `release.yml`, job `substitute-update-targets`, step "Sign the release manifest" only | No expiry; each build trusts the anchor compiled into it | Generate a new pair, commit the new anchor, replace the secret; the next release trusts it. |
| `UPDATE_NEXT_ROOT_KEY` | the next root, during one `rotate-root` run                                                                            | `substitute-update-repository.yml`, `rotate-root` only                                | –                                                        | Set only for the rotation, then move it to `UPDATE_ROOT_KEY` and delete it.                 |

The role keys are Ed25519 seeds written by the signing tool's `keygen`, stored as 64
hexadecimal digits. The release manifest key is an RSA-3072 private key in PKCS#8 PEM;
signatures are RSA-PSS with SHA-256 and a 32-byte salt, as the application verifies them
with Windows CNG. Its public half is committed as
`Substitute/installer/release/release-anchor.bin` (the 411-byte CNG public blob), which a
release build compiles in through `SUGARSUBSTITUTE_RELEASE_ANCHOR`.

## Environment

- `release` allows deployments from `main` only and has no required reviewers, so the daily
  timestamp refresh is never held waiting for an approval.
- Publication stays switched off until the repository variable `RELEASES_ENABLED` is `true`.

## Key ceremony

Build the signing tool, then run the ceremony from a clean checkout of `main`. `--keys` must
be a new directory outside the checkout, ideally on removable or encrypted storage:

```powershell
cd Substitute
cargo build --locked --release --package sugarsubstitute-update-repository
cd ..
node .github/release/key-ceremony.mjs `
  --tool "$PWD\Substitute\target\release\sugarsubstitute-update-repository.exe" `
  --keys <new private directory> --publish <new public directory>
```

It writes the four role keys and the release manifest key to `--keys`, with `ceremony.json`
naming each key identity; signs the first repository state into `--publish`; writes
`.github/update-repository/root.json` and `Substitute/installer/release/release-anchor.bin`
into the checkout; and prints one command per secret, for example:

```powershell
Get-Content -Raw -LiteralPath '<keys>\update-root.key' | gh secret set UPDATE_ROOT_KEY --env release --repo Artificial-Sweetener/SugarSubstitute-Rust
```

Each command reads the key from its file; no key appears in a command line. Afterwards:

1. Commit `root.json` and `release-anchor.bin`.
2. Upload the files in `--publish` to the `substitute-updates` release, timestamp last. The
   timestamp expires 14 days after the ceremony, so the daily refresh must be running by then.
3. Run the printed commands.
4. Keep `--keys` offline. `UPDATE_ROOT_KEY` is needed again only for `rotate-root`.

## Crash handler build inputs

The release job builds the crash handler from the reviewed, hash-pinned Crashpad source
archives and GN/Ninja binaries. They are assets of the `substitute-build-inputs` release:
`crashpad.tar.gz`, `mini_chromium.tar.gz`, `zlib.tar.gz`, `googletest.tar.gz`,
`buildtools.tar.gz` (pinned in `Substitute/tools/crashpad/verify_source.py`) and `gn.exe`,
`ninja.exe` (pinned in `Substitute/tools/crashpad/build_native.py`). Every file is checked
against its pin before anything is built.

## Rotating the release manifest key

```powershell
node Substitute/installer/release/release-signature.mjs generate <keys>\release-manifest.key.pem Substitute\installer\release\release-anchor.bin
```

The anchor file must be removed first. Commit the new anchor, then install the new PEM with
`Get-Content -Raw -LiteralPath '<keys>\release-manifest.key.pem' | gh secret set RELEASE_MANIFEST_KEY --env release`.

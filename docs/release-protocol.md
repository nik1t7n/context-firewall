# Release Protocol

Context Firewall has real users now. Releases must be boring, tested, and easy
to roll forward.

## Rule

Do not publish a release because `main` looks good. Publish only after the real
install path, real binary, and real release artifacts have been checked.

No skipped checks. No “probably fine”. No untested tags.

## Before Tagging

Run these from a clean `main`:

```bash
git status --short
git pull --ff-only origin main
cfw run -- cargo fmt --check
cfw run -- cargo test
cfw run -- cargo clippy --all-targets --all-features -- -D warnings
cfw run -- cargo build -p cfw
cfw run -- scripts/release-smoke.sh target/debug/cfw
cfw run -- target/debug/cfw update-check --force
cfw run -- git diff --check
```

If any command fails, stop. Fix the real problem and restart the checklist.

## Tagging

Only tag after the full pre-tag checklist passes.

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

## After Publishing

Wait for the release workflow to finish. Then verify the public release, not the
local build:

```bash
cfw run -- gh release view vX.Y.Z --json tagName,isDraft,isPrerelease,publishedAt,assets,url
cfw run -- gh run watch <release-run-id> --exit-status
cfw run -- gh workflow run release-smoke.yml -f tag=vX.Y.Z
cfw run -- gh run watch <release-smoke-run-id> --exit-status
cfw run -- npm view @nik1t7n/context-firewall version dist-tags.latest
```

Download at least one GitHub release artifact and run the smoke test against
that downloaded binary:

```bash
tmpdir=$(mktemp -d)
gh release download vX.Y.Z --pattern 'cfw-aarch64-apple-darwin.tar.xz' --dir "$tmpdir"
tar -xf "$tmpdir/cfw-aarch64-apple-darwin.tar.xz" -C "$tmpdir"
bin=$(find "$tmpdir" -type f -name cfw -perm -111 | head -n 1)
cfw run -- scripts/release-smoke.sh "$bin"
```

Check the Homebrew tap points at the same version:

```bash
cfw run -- gh api repos/nik1t7n/homebrew-tap/contents/Formula/cfw.rb --jq '.content' | base64 --decode | rg 'version "|download/vX.Y.Z|sha256'
```

## Final Gate

A release is ready only when all of this is true:

- GitHub Release exists and is not draft or prerelease.
- Release assets are uploaded for supported platforms.
- Release workflow passed.
- Release smoke passed against downloaded artifacts.
- npm `latest` is the release version.
- Homebrew tap points at the release version.
- The downloaded binary reports the release version.
- `main` CI is green.
- The working tree is clean.

If a release is bad, do not delete evidence. Publish a fixed patch release and
say clearly what changed.

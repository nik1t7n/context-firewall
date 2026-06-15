# Install Context Firewall

Context Firewall ships as one binary: `cfw`.

After installation, connect it to your coding agent with:

```bash
cfw install agent
cfw install gemini
cfw install antigravity
cfw install claude
cfw install cursor
```

Then run noisy commands through the firewall:

```bash
cfw run -- cargo test
cfw run -- rg -n "TODO|FIXME" .
cfw run -- git diff
```

## Recommended Installs

These channels are produced by the release pipeline.

### macOS and Linux with Homebrew

```bash
brew install nik1t7n/tap/cfw
```

If Homebrew asks you to tap first:

```bash
brew tap nik1t7n/tap
brew install cfw
```

### npm and npx

```bash
npm install -g @nik1t7n/context-firewall
```

Run without a global install:

```bash
npx @nik1t7n/context-firewall --help
npx @nik1t7n/context-firewall install gemini
```

### Shell Installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/nik1t7n/context-firewall/releases/latest/download/cfw-installer.sh \
  | sh
```

### Windows PowerShell

```powershell
irm https://github.com/nik1t7n/context-firewall/releases/latest/download/cfw-installer.ps1 | iex
```

### Rust Users

From a checkout:

```bash
cargo install --path crates/cfw-cli
```

After binary releases exist, `cargo-binstall` can install from GitHub release
artifacts:

```bash
cargo binstall cfw
```

## Verify

```bash
cfw --version
cfw receipt
```

## Maintainer Release Checklist

Before release-based install commands work, the maintainer needs:

- A pushed GitHub repository at `nik1t7n/context-firewall`.
- A first SemVer tag, for example `v0.1.0`.
- A GitHub repository secret named `NPM_TOKEN` with publish access to
  `@nik1t7n/context-firewall`.
- An npm organization or scope named `@nik1t7n`, or a different package
  scope configured in `dist-workspace.toml`.
- A GitHub repository named `nik1t7n/homebrew-tap`.
- A GitHub repository secret named `HOMEBREW_TAP_TOKEN` with permission to push
  to the Homebrew tap repository.

The release workflow builds the archives, shell installer, PowerShell
installer, Homebrew formula, and npm package from the same tagged release.

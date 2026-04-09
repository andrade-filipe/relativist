# SPEC-15: Binary Distribution and Installation

**Status:** Draft v2
**Depends on:** SPEC-00 (Glossary), SPEC-07 (Deployment), SPEC-13 (System Architecture)
**Gray zones resolved:** ---
**References consumed:** ---
**Discussions consumed:** ---
**Arguments consumed:** ---
**Code analyses consumed:** ---

---

## 1. Purpose

This spec defines how the Relativist binary is distributed to end users and test machines without requiring the Rust toolchain. SPEC-07 defines how Relativist is deployed and executed once the binary is present on a machine; SPEC-15 defines how the binary gets onto the machine in the first place.

The distribution system has three goals: (1) enable one-command installation on Linux and Windows machines for the TCC experimental campaign across multiple physical computers, (2) provide integrity verification via SHA-256 checksums on every release artifact, and (3) automate the entire release pipeline so that a `git tag` push is the only manual step required to publish a new version.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **Release Artifact** | A precompiled binary archive (`.tar.gz` for Linux, `.zip` for Windows) published to GitHub Releases for a specific OS/architecture combination. Each artifact contains the `relativist` binary and nothing else. |
| **Target Triple** | A Rust compilation target identifier specifying OS, architecture, and ABI (e.g., `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`). Determines the cross-compilation target for `cargo build --target`. |
| **GHCR** | GitHub Container Registry (`ghcr.io`), GitHub's built-in Docker image registry. Used to distribute the Relativist Docker image without requiring users to build from source. |
| **Install Script** | A POSIX shell script downloadable via `curl` that detects the user's OS and architecture, downloads the correct release artifact from GitHub Releases, verifies its checksum, and places the binary in a directory on the user's PATH. |
| **Checksum Manifest** | A file (`SHA256SUMS`) published alongside release artifacts containing the SHA-256 hash of every artifact in the release. Enables integrity verification before installation. |
| **SmartScreen** | Windows Defender SmartScreen, a Microsoft security feature that warns users before running executables that lack a code signing certificate or sufficient download reputation. Unsigned `.exe` files downloaded from the internet trigger a "Windows protected your PC" dialog. |
| **Code Signing** | The process of digitally signing executables with an Authenticode certificate so that Windows recognizes the publisher and suppresses SmartScreen warnings. |

---

## 3. Requirements

### 3.1 GitHub Releases with Precompiled Binaries

**R1.** On every version tag push (`v*`), Relativist MUST publish precompiled release binaries to GitHub Releases for the `x86_64-unknown-linux-gnu` target. The artifact MUST be a gzipped tarball named `relativist-{version}-x86_64-unknown-linux-gnu.tar.gz` containing the `relativist` binary. **(MUST)**

**Rationale:** The TCC experimental campaign runs on Linux machines. This is the primary distribution target. The naming convention includes the full target triple to avoid ambiguity when multiple platforms are supported.

**R2.** Release binaries SHOULD also be published for the `x86_64-pc-windows-msvc` target. The artifact MUST be a zip archive named `relativist-{version}-x86_64-pc-windows-msvc.zip` containing `relativist.exe`. **(SHOULD)**

**Rationale:** The development machine runs Windows. Having a precompiled Windows binary eliminates the need for `cargo build` during development-time testing and enables distribution to Windows test machines.

**R3.** Release binaries MAY be published for `aarch64-apple-darwin` (macOS ARM) and `x86_64-apple-darwin` (macOS Intel). **(MAY)**

### 3.2 CI/CD Release Workflow

**R4.** A GitHub Actions workflow (`release.yml`) MUST automate the compilation, packaging, and upload of release artifacts when a version tag matching `v*` is pushed. No manual steps beyond `git tag` and `git push --tags` are required to produce a complete release. **(MUST)**

**R5.** The release workflow MUST use a build matrix to produce binaries for all MUST and SHOULD targets (R1, R2) in a single workflow run. Each target MUST build on its native runner (`ubuntu-latest` for Linux, `windows-latest` for Windows) to avoid cross-compilation complexity. **(MUST)**

**R6.** The release workflow MUST create a GitHub Release with a title matching the tag name and a body containing the list of commits since the previous tag. The release MUST NOT be marked as draft or prerelease for stable version tags. **(MUST)**

### 3.3 Docker Image on GHCR

**R7.** On every version tag push (`v*`), the Docker workflow MUST build the Docker image AND push it to GHCR at `ghcr.io/andrade-filipe/relativist`. The current `docker.yml` workflow only builds locally without pushing; it MUST be extended. **(MUST)**

**R8.** The Docker image MUST be tagged with both the version tag (e.g., `v0.6.0`) and `latest`. This enables users to pin a specific version or track the most recent release. **(MUST)**

### 3.4 Install Script

**R9.** The repository MUST include an install script at `scripts/install.sh` that can be invoked as: **(MUST)**

```bash
curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

The script MUST:
- Detect the user's OS via `uname -s` (Linux, Darwin) and architecture via `uname -m` (x86_64, aarch64).
- Map the detected OS/architecture to the correct target triple and artifact name.
- Fetch the latest release tag from the GitHub API (`https://api.github.com/repos/andrade-filipe/relativist/releases/latest`).
- Download the correct archive from the release assets.
- Extract the binary and place it in `${INSTALL_DIR:-/usr/local/bin}`. If `/usr/local/bin` is not writable, fall back to `~/.local/bin` and print a PATH warning.
- Print a success message with the installed version.

**R10.** The install script MUST verify the downloaded artifact's SHA-256 checksum against the published `SHA256SUMS` file before installation. If verification fails, the script MUST abort with a non-zero exit code and a clear error message. **(MUST)**

### 3.5 Fallback: cargo install

**R11.** Installation via `cargo install --git https://github.com/andrade-filipe/relativist` MUST be supported and documented as a fallback for platforms without precompiled binaries. **(MUST)**

**Note:** This requirement is already satisfied by the current `Cargo.toml` configuration (single binary target, MIT license, repository URL). No code changes required.

### 3.6 Integrity

**R12.** Every GitHub Release MUST include a `SHA256SUMS` file containing the SHA-256 hash of every release artifact (one line per artifact, format: `<hash>  <filename>`). **(MUST)**

**R13.** The release workflow MUST compute checksums automatically as part of the release pipeline. Checksums MUST NOT be computed manually. **(MUST)**

### 3.7 Versioning

**R14.** Version tags MUST follow Semantic Versioning 2.0.0 in the format `vMAJOR.MINOR.PATCH` (e.g., `v0.6.0`). The `version` field in `Cargo.toml` MUST match the Git tag (without the `v` prefix). Before creating a release tag, the developer MUST update `Cargo.toml` to the target version. **(MUST)**

**Note:** The current `Cargo.toml` says `version = "0.0.1"` while existing Git tags go up to `v0.5.0`. This discrepancy MUST be resolved before the first automated release by updating `Cargo.toml` to match the next release version. *(Resolved in v0.6.0: Cargo.toml synced to 0.6.0.)*

### 3.8 Windows Direct Download

**R15.** Each GitHub Release SHOULD include the bare Windows executable (`relativist-{version}-x86_64-pc-windows-msvc.exe`) as a separate release asset alongside the `.zip` archive (R2). This eliminates the double-extraction step for Windows users who download via browser (browser download produces `.zip`, which must then be extracted — the bare `.exe` is a single step). The `.zip` is retained for backward compatibility and for users who prefer archives. **(SHOULD)**

**Rationale:** When a user downloads a `.zip` from a browser, the browser saves the `.zip` file, and the user must then extract it to obtain `relativist.exe`. With a direct `.exe` download, the user downloads and runs — one step instead of two.

### 3.9 SmartScreen Mitigation

**R16.** The USAGE_GUIDE and GitHub Release notes MUST document how Windows users can bypass the SmartScreen warning for unsigned executables. The documentation MUST include both methods: (1) right-click the `.exe` → Properties → check "Unblock" → OK, and (2) in the SmartScreen dialog, click "More info" → "Run anyway". **(MUST)**

**Rationale:** Until code signing is implemented (R17), unsigned executables will trigger SmartScreen. Clear documentation prevents user confusion and support burden.

**R17.** Windows release executables SHOULD be signed with an Authenticode code signing certificate to suppress SmartScreen warnings automatically. The recommended approach is [SignPath Foundation](https://signpath.org), which provides free code signing certificates for open-source projects hosted on GitHub with an OSI-approved license. Until code signing is implemented, R16 provides the interim solution. **(SHOULD)**

**Note:** SignPath Foundation requirements: OSI-approved license (MIT — satisfied), public GitHub repository (satisfied), MFA enabled on GitHub account (must verify). Application is submitted via signpath.org; approval timeline is days to weeks.

---

## 4. Design

### 4.1 Release Workflow (`.github/workflows/release.yml`)

```
Trigger: push tags ['v*']

Jobs:
  build-binaries:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            archive_ext: tar.gz
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            archive_ext: zip
    steps:
      - checkout
      - install Rust stable
      - cargo build --release --target ${{ matrix.target }}
      - package binary into archive
      - copy bare .exe with versioned name (Windows only, R15)
      - upload artifact (glob matches .tar.gz, .zip, and .exe)

  create-release:
    needs: build-binaries
    steps:
      - download all artifacts
      - compute SHA256SUMS
      - create GitHub Release with all artifacts + SHA256SUMS
```

### 4.2 Docker Workflow (`.github/workflows/docker.yml`)

```
Trigger: push tags ['v*']

Jobs:
  build-and-push:
    permissions:
      packages: write
    steps:
      - checkout
      - login to GHCR via docker/login-action
      - build and push via docker/build-push-action
        tags: ghcr.io/andrade-filipe/relativist:{tag}, :latest
```

### 4.3 Install Script (`scripts/install.sh`)

```
#!/bin/sh
set -eu

1. Detect OS (uname -s → linux/darwin) and ARCH (uname -m → x86_64/aarch64)
2. Map to TARGET_TRIPLE and ARCHIVE_EXT
3. Fetch LATEST_TAG from GitHub API
4. Download archive and SHA256SUMS from GitHub Release
5. Verify checksum (sha256sum -c or shasum -a 256 -c)
6. Extract binary to INSTALL_DIR (/usr/local/bin or ~/.local/bin)
7. Print success: "relativist {version} installed to {path}"
```

### 4.4 Installation Methods Summary

| Method | Command | Requires | Platform |
|--------|---------|----------|----------|
| Install script | `curl -sSfL .../install.sh \| sh` | curl, sh | Linux, macOS |
| Docker | `docker pull ghcr.io/andrade-filipe/relativist` | Docker | Any |
| GitHub Release | Download from Releases page | Browser | Any |
| cargo install | `cargo install --git ...` | Rust toolchain | Any |

### 4.5 File Layout

```
.github/workflows/
  ci.yml              # existing: build + test + lint on push
  docker.yml          # modified: build AND push to GHCR on tags
  release.yml         # new: cross-compile + GitHub Release on tags
scripts/
  install.sh          # new: one-liner installer
```

---

## 5. Verification

1. Push tag `v*` → `release.yml` runs, GitHub Release appears with Linux `.tar.gz`, Windows `.zip`, Windows `.exe`, and `SHA256SUMS`.
2. Push tag `v*` → `docker.yml` runs, `docker pull ghcr.io/andrade-filipe/relativist:{tag}` succeeds.
3. On a clean Ubuntu container: `curl -sSfL .../install.sh | sh && relativist --version` prints the installed version.
4. On Windows: download `.exe` directly from Releases, right-click → Properties → Unblock, run `relativist.exe --version`.
5. Checksum: download artifact and `SHA256SUMS`, run `sha256sum -c SHA256SUMS` → `OK`.
6. `SHA256SUMS` contains entries for all 3 artifacts (linux `.tar.gz`, windows `.zip`, windows `.exe`).

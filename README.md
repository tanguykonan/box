# Box CLI (Rust Engine)

> Autonomous Application Packaging and High-Performance Sandbox Runtime Engine.  
> Package, distribute, and execute Python and Node.js/TypeScript applications as hermetic, self-contained `.box` archives without virtual environments or pre-installed host runtimes.

---

## Overview

Box CLI is a native Rust engine engineered for ultra-fast packaging, deterministic container execution, and secure artifact distribution. Applications are compiled into immutable, compressed `.box` archives containing application source or protected bytecode, pinned dependency libraries, runtime specifications, and persistent storage definitions.

---

## Key Features

- **Hermetic Container Format**: Archives are structured as high-ratio Zstandard-compressed TAR streams accompanied by strict JSON metadata manifests and SHA-256 stream/tree checksums.
- **Bytecode Compilation and Source Protection**: Compiles JavaScript and TypeScript to native V8 bytecode caches (`.jsc`) and Python to optimized bytecode (`.pyc`) with automatic stripping of raw source files.
- **Isolated Ephemeral Sandbox**: Executes applications inside sandboxes isolated in temporary directories, with automatic runtime isolation, signal propagation, and cleanup.
- **Persistent Volume System**: Mounts named storage volumes (`~/.box/volumes/<name>`) or host directory bind mounts with cross-platform path resolution and bidirectional synchronization.
- **Registry and Streaming Distribution**: Native support for pushing, pulling, and auto-executing packages from public registries or self-hosted private registries with chunked streaming and sha256 verification.
- **Strict Environment Validation**: Ascending discovery of `.env` files with automated validation against `.env.example` schemas and placeholder detection before process execution.

---

## CLI Command Matrix

| Command | Syntax | Description |
| :--- | :--- | :--- |
| **`build`** | `box build [<dir>] [-o <output>] [--no-protect]` | Packages a project directory into an optimized `.box` container. |
| **`run`** | `box run <target> [flags] [-- <args>...]` | Executes a `.box` archive or registry package in an isolated sandbox. |
| **`info`** | `box info <target>` | Inspects archive manifests, dependency trees, and cryptographic checksums. |
| **`volume`** | `box volume [create \| list \| inspect \| prune \| remove]` | Manages persistent sandbox storage volumes. |
| **`config`** | `box config [get \| set \| list]` | Reads or writes global runtime parameters and storage directories. |
| **`login`** | `box login [<registry_url>]` | Authenticates against the Box registry and stores credentials securely. |
| **`whoami`** | `box whoami` | Displays the active authenticated identity and registry endpoint. |
| **`logout`** | `box logout [<registry_url>]` | Revokes and clears authentication tokens. |
| **`push`** | `box push <archive.box> [<package_spec>]` | Uploads a `.box` package to the target registry via chunked streaming. |
| **`pull`** | `box pull <package_spec>` | Downloads a package from the registry and registers it locally. |

---

## Quickstart

### 1. Build from Source

Ensure you have the Rust toolchain installed (Rust 1.75 or later):

```bash
# Clone the repository
git clone https://github.com/tanguykonan/box.git
cd box

# Compile in release mode
cargo build --release

# The compiled binary is available at:
./target/release/box --version
```

### 2. Package an Application

Create a `boxconfig.yml` in your project root or run packaging directly:

```bash
# Package the current directory
box build .

# Package a specific directory with custom output path
box build ./services/api -o api-service.box
```

### 3. Run in an Isolated Sandbox

```bash
# Execute a local package
box run api-service.box

# Execute with environment file and volume binding
box run api-service.box --env .env.production -v app_data:/data

# Pass arguments directly to the application entrypoint
box run api-service.box -- --port 8080 --workers 4

# Run directly from the cloud registry with automatic pulling
box run username/api-service:v1.0.0
```

---

## Technical Documentation Index

Detailed architectural deep-dives, protocol specifications, data structures, and state machine lifecycles are documented in dedicated guides:

### Core Architecture and Protocols (`Documentation/`)

| Document | Description |
| :--- | :--- |
| [**Build and Run Process**](Documentation/build_and_run_process.md) | Archive format layout (TAR + Zstd), 5-phase build pipeline, bytecode compilation, Rayon parallel hashing, 4-phase sandbox lifecycle, and dual-tier integrity verification. |
| [**Configuration and Authentication**](Documentation/config_and_login_process.md) | Global storage hierarchy (`~/.box`), 4-tier parameter cascade, JSON schemas (`AppConfigData`, `AuthCredential`), and JWT authentication protocol (`login`, `whoami`, `logout`). |
| [**Registry Protocol (Push and Pull)**](Documentation/push_and_pull_process.md) | Package grammar (`[<url>/][<owner>/]<name>[:<tag>]`), REST API protocol, 64 KB chunked streaming upload, progress indicator architecture, and transparent auto-pull. |
| [**Sandbox Lifecycle and Persistent Volumes**](Documentation/sandbox_volumes_process.md) | Volume storage topology (`~/.box/volumes/`), cross-platform path parsing with Windows drive letter support, 5-phase synchronization lifecycle, and volume administration. |
| [**Environment Variables and Schema Validation**](Documentation/environment_and_variables_guide.md) | Ascending `.env` bootstrap discovery (depth 6), precedence hierarchy, schema enforcement (`.env.example`), placeholder pattern detection, and sandbox process isolation. |

### Manifests and Configuration Templates (`examples/templates/`)

| Document | Description |
| :--- | :--- |
| [**Boxconfig Manifest Specification**](examples/templates/boxconfig_file.md) | Complete YAML specification (`boxconfig.yml` / `boxfile.yml`), field definition table, runtime engines, dependency resolution models, and environment variables. |
| [**Self-Hosted Storage and Registry Configuration**](examples/templates/self_configuration.md) | Configuration guide for redirecting storage backends, deploying private self-hosted registries, and isolating per-domain authentication in `config.json`. |
| [**Persistent Volumes Manifest Configuration**](examples/templates/volumes_configuration.md) | Persistent volume schema, structured map vs. shorthand list formats, path sanitization, and internal `VolumeSpec` data representation. |

### Example Projects (`examples/projects/`)

| Project | Runtime | Description |
| :--- | :--- | :--- |
| [**`python-app`**](examples/projects/python-app) | Python 3.12 | Discord bot archiving messages into a persistent JSON store on a mounted volume (`discord_messages:/data`). |
| [**`js-app`**](examples/projects/js-app) | Node.js 26 | Discord Hello World bot with V8 bytecode compilation (`.jsc`) and raw source stripping. |
| [**`ts-app`**](examples/projects/ts-app) | Node.js 26 | Next.js application built with TypeScript and React. |

---

## Testing and Quality Assurance

The codebase includes an integration test suite covering CLI parsing, packaging, volume mounting, config precedence, and archive integrity.

```bash
# Run all integration test suites
cargo test --verbose

# Run strict static analysis (zero warnings allowed)
cargo clippy --all-targets --all-features -- -D warnings

# Validate code formatting
cargo fmt --all -- --check
```

---

## Contributing and Governance

Contributions are welcome. Please ensure that all submissions follow our standards:

- Read our [**Contribution Guidelines**](.github/CONTRIBUTING.md) for branch management and commit message standards.
- Review our [**Security Policy**](SECURITY.md) for reporting security vulnerabilities.
- Adhere to the [**Code of Conduct**](CODE_OF_CONDUCT.md).

---

## License

This project is licensed under the terms of the GNU General Public License v3.0 or later. See [COPYING](COPYING) for details.

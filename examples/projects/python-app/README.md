# Python Discord Logger Example (`python-app`)

> Official reference project demonstrating how to package, protect, and execute a **Python 3.12** Discord message logging bot with **persistent volume storage** and **bytecode compilation** using **Box CLI**.  
> Package identifier: `python-discord-logger`

---

## Overview and Purpose

### Why was `python-app` created?

`python-app` is a reference implementation designed for **Box CLI (`box-cli-rust`)** to demonstrate stateful backend services in isolated containers. While stateless scripts only require simple execution, stateful daemons and background bots present distinct operational requirements:

- **Persistent Volume Mounting (`volumes`)**: Isolated sandboxes are ephemeral by default. This example demonstrates how Box mounts persistent host-backed storage (`discord_messages:/data`) so that logged JSON data survives container restarts, updates, and rebuilds.
- **Python Bytecode Compilation and Source Protection (`protect: true`)**: Demonstrates Box's native Python compiler, which compiles `.py` source code into optimized `.pyc` bytecode files and strips the original source files from the final `.box` archive.
- **Environment Schema Enforcement (`env_schema`)**: Showcases how Box CLI automatically parses `.env.example`, detects missing required keys or unresolved placeholders (`YOUR_DISCORD_BOT_TOKEN_HERE`), and halts execution safely before runtime errors occur.
- **Hermetic CPython Runtime**: Illustrates how Box provisions a dedicated Python 3.12 runtime and packages dependencies declared in `requirements.txt`.

---

## Dependencies and Packages

The application relies on `requirements.txt` for external package management:

```text
discord.py>=2.3.2
```

### Dependency Rationale

| Dependency | Purpose and Rationale |
| :--- | :--- |
| **`discord.py`** (>= 2.3.2) | Modern asynchronous Python framework for connecting to Discord WebSocket Gateways, handling real-time event loops (`on_ready`, `on_message`), and managing slash or text commands (`!stats`). |
| **`json` (Standard Library)** | Handles serialization and structured read/write operations for the persistent log file (`/data/messages.json`). |
| **`os` and `sys` (Standard Library)** | Manages directory initialization, filesystem verification, environment variable loading, and graceful process exit handling. |
| **`datetime` (Standard Library)** | Generates standardized ISO 8601 UTC timestamps for every incoming message record. |

---

## Box Configuration (`boxconfig.yml`)

The `boxconfig.yml` manifest defines the packaging rules, runtime specifications, storage volumes, and validation templates:

```yaml
name: python-discord-logger
version: 1.0.0

runtime:
  type: python
  version: "3.12"

# Bytecode compilation and source protection
protect: true

# Working directory
workdir: .

# Application entrypoint
entrypoint: main.py

# Production dependencies
dependencies: requirements.txt

# Source files to include
include:
  - main.py

# Persistent volume definition (<volume_name>:<sandbox_path>)
volumes:
  - discord_messages:/data

# Environment schema validation
env_schema: .env.example
```

### Configuration Breakdown

1. **`name` and `version`**
   - Sets the canonical package identifier (`python-discord-logger`) and semantic version (`1.0.0`) for local inspection and registry distribution.

2. **`runtime` (`type: python`, `version: "3.12"`)**
   - Directs Box to provision a hermetic **Python 3.12** runtime interpreter inside the sandbox.

3. **`protect: true`**
   - Enables bytecode compilation. Box compiles `main.py` into optimized Python bytecode (`.pyc`) and strips raw `.py` source files from the `.box` archive for tamper resistance and faster startup.

4. **`workdir: .`**
   - Sets the root working directory inside the sandbox environment.

5. **`entrypoint: main.py`**
   - Designates `main.py` as the execution startup script launched by the Python runtime.

6. **`dependencies: requirements.txt`**
   - Points Box CLI to the dependency manifest to install and bundle required packages during the build process.

7. **`include`**
   - Specifies the source files (`main.py`) to include in the container build.

8. **`volumes: [ discord_messages:/data ]`**
   - Defines a persistent named volume mapping. The host directory `~/.box/volumes/discord_messages` is mounted to `/data` inside the sandbox, ensuring that `/data/messages.json` remains persistent across multiple runs.

9. **`env_schema: .env.example`**
   - Instructs Box CLI to validate the presence of required environment variables against the `.env.example` template prior to starting the container.

---

## Step-by-Step Build and Run Guide

### 1. Configure Environment Variables

Create a `.env` file in the project directory based on `.env.example`:

```bash
# Copy example template
cp .env.example .env
```

Edit `.env` and set your real Discord bot token:

```env
DISCORD_TOKEN=your_actual_bot_token_here
DATA_DIR=/data
```

---

### 2. Package with Box CLI

Build the application into a `.box` container archive:

```bash
# Package from the project directory
box build . -o python-discord-logger.box
```

---

### 3. Run in the Box Sandbox

Execute the package with your environment file:

```bash
box run python-discord-logger.box --env .env
```

When the bot connects to Discord:

- Incoming messages sent in accessible channels are recorded into `/data/messages.json`.
- Typing `!stats` in Discord displays the total count of messages stored in the persistent volume.

---

### 4. Manage Persistent Volumes

You can inspect and manage the volume created by the bot using Box volume commands:

```bash
# List all active Box volumes
box volume list

# Inspect volume details and host storage location
box volume inspect discord_messages
```

Data stored inside `~/.box/volumes/discord_messages` persists even when the `.box` container is stopped or rebuilt.

---

### 5. Inspect Archive Manifest and Metadata (Optional)

Inspect the package structure, bytecode protection status, and metadata:

```bash
box info python-discord-logger.box
```

# JavaScript Discord Bot Example (`js-app`)

> Official reference project demonstrating how to package, protect, and execute a **Node.js JavaScript** Discord bot with **V8 bytecode compilation** and **source protection** using **Box CLI**.  
> Package identifier: `js-discord-bot`

---

## Overview and Purpose

### Why was `js-app` created?

`js-app` is a reference implementation designed for **Box CLI (`box-cli-rust`)** to demonstrate how Node.js applications are compiled into tamper-resistant, high-performance `.box` containers. Key capabilities showcased in this project include:

- **V8 Bytecode Compilation (`protect: true`)**: Demonstrates Box's automated V8 compiler for JavaScript. Source files (`src/index.js`) are compiled into optimized V8 bytecode caches (`.jsc`) and raw `.js` source files are stripped from the container archive.
- **Intellectual Property Protection**: Enables distribution of Node.js services without exposing plaintext source code, while speeding up cold starts by bypassing the JavaScript parsing stage.
- **Environment Schema Enforcement (`env_schema`)**: Illustrates how Box CLI automatically parses `.env.example`, verifies that required keys such as `DISCORD_TOKEN` are defined, and blocks execution if placeholder values remain.
- **Hermetic Node.js Runtime**: Shows how Box provisions a standalone Node.js 26 runtime and bundles dependencies declared in `package.json` into a single portable container.

---

## Dependencies and Packages

The project manages external dependencies through `package.json`:

```json
{
  "dependencies": {
    "discord.js": "^14.16.3"
  }
}
```

### Dependency Rationale

| Dependency | Purpose and Rationale |
| :--- | :--- |
| **`discord.js`** (`^14.16.3`) | Official Discord API client for Node.js. Manages WebSocket Gateway connections, handles event dispatching (`ready`, `messageCreate`), and configures gateway intents (`Guilds`, `GuildMessages`, `MessageContent`). |
| **`process` (Node.js Core)** | Accesses environment variables (`process.env.DISCORD_TOKEN`) and handles process exit codes. |
| **`console` (Node.js Core)** | Outputs structured diagnostic and operational logs to stdout and stderr. |

---

## Box Configuration (`boxconfig.yml`)

The `boxconfig.yml` manifest controls how Box CLI packages, compiles, and launches the application:

```yaml
name: js-discord-bot
version: 1.0.0

runtime:
  type: node
  version: "26"

# V8 Bytecode compilation and source protection
protect: true

# Working directory
workdir: .

# Application entrypoint
entrypoint: src/index.js

# Production dependencies
dependencies: package.json

# Source files to include
include:
  - src/

# Environment schema validation
env_schema: .env.example
```

### Configuration Breakdown

1. **`name` and `version`**
   - Sets the canonical package identifier (`js-discord-bot`) and semantic version (`1.0.0`) used in registry distribution and manifest metadata.

2. **`runtime` (`type: node`, `version: "26"`)**
   - Instructs Box to provision a hermetic **Node.js 26** engine inside the isolated execution sandbox.

3. **`protect: true`**
   - Activates V8 bytecode compilation. Box compiles all `.js` files inside `src/` into native V8 bytecode (`.jsc`) and strips raw JavaScript source code from the final archive.

4. **`workdir: .`**
   - Defines the working root directory inside the sandbox environment.

5. **`entrypoint: src/index.js`**
   - Sets the primary execution script executed by the Node.js runtime when the container starts.

6. **`dependencies: package.json`**
   - Directs Box CLI to install and package production dependencies listed in `package.json`.

7. **`include`**
   - Specifies the source directory (`src/`) to collect and compile into bytecode.

8. **`env_schema: .env.example`**
   - Links the environment template file to validate required environment variables prior to container execution.

---

## Step-by-Step Build and Run Guide

### 1. Configure Environment Variables

Create a local `.env` file from the provided template:

```bash
# Copy example template
cp .env.example .env
```

Edit `.env` and supply your actual Discord bot token:

```env
DISCORD_TOKEN=your_actual_bot_token_here
```

---

### 2. Package with Box CLI

Build the application into a `.box` container archive with bytecode protection:

```bash
# Package from the project directory
box build . -o js-discord-bot.box
```

During this build step, Box compiles `src/index.js` into V8 bytecode and removes the plain-text `.js` files from the output container.

---

### 3. Run in the Box Sandbox

Launch the protected application inside the isolated sandbox:

```bash
box run js-discord-bot.box --env .env
```

When the bot connects to Discord:

- The bot logs in and outputs `[js-discord-bot] Ready and logged in as <bot_tag>`.
- Sending `!hello`, `!ping`, or `hello` in a channel prompts the bot to reply with confirmation that the Box CLI Node.js engine is running.

---

### 4. Inspect Archive Manifest and Metadata (Optional)

Inspect package checksums, runtime parameters, and protection settings:

```bash
box info js-discord-bot.box
```

# TypeScript Next.js Example (`ts-app`)

> Official reference project demonstrating how to package, distribute, and execute a modern **Next.js 16 + React 19 + TypeScript** application inside an isolated container using **Box CLI**.  
> Registry Package: [Box Hub • @tanguykonan/ts-app](https://boxhub.paxiz.org/box/tanguykonan/ts-app)

---

## Overview and Purpose

### Why was `ts-app` created?

`ts-app` is a reference implementation tailored for **Box CLI (`box-cli-rust`)**. While simple command-line scripts or Discord bots (such as `js-app` and `python-app`) execute single entry files, real-world web applications introduce complexities such as:

- **Multi-stage asset pipelines**: Next.js compiles server components, client bundles, CSS, and static metadata into a `.next/` distribution folder.
- **Dynamic Server-Side Rendering (SSR) and Static Generation**: Next.js requires a specific production runtime server (`next start`) rather than standard single-file scripts.
- **Hermetic Node.js Execution**: Demonstrating how Box packages production dependencies and runtime artifacts without relying on host-installed Node.js or global package managers.
- **Non-Bytecode Protection Model**: Illustrating why and how to disable raw V8 bytecode compilation (`protect: false`) when running full-stack frameworks that rely on dynamic module chunking and client hydration.

---

## Special Packages

```json
{
  "dependencies": {
    "@next/swc-wasm-nodejs": "^16.3.5",
    "next": "16.3.4",
    "react": "19.2.8",
    "react-dom": "19.2.8"
  },
  "devDependencies": {
    "@tailwindcss/postcss": "^4",
    "@types/node": "^20",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "eslint": "^9",
    "eslint-config-next": "16.3.4",
    "tailwindcss": "^4",
    "typescript": "^5"
  }
}
```

### Key Dependencies Explained

| Package | Version | Purpose and Rationale |
| :--- | :--- | :--- |
| **`@next/swc-wasm-nodejs`** | `^16.3.5` | **Special Dependency**: Next.js utilizes SWC (Speedy Web Compiler) for minification and bundling. In default environments, Next.js attempts to load architecture-specific native binaries (`.node` files). When packaging applications into portable, hermetic `.box` containers across different operating systems or sandboxes, native binary mismatches can occur. `@next/swc-wasm-nodejs` provides a WebAssembly (WASM) fallback engine that runs consistently on any Node.js runtime without native compilation requirements. |
| **`next`** | `16.3.4` | Core React framework supporting Server Components, App Router, SSR, and API routes. |
| **`react` / `react-dom`** | `19.2.8` | The underlying modern UI library powering Next.js 16 components and client-side hydration. |
| **`@tailwindcss/postcss` and `tailwindcss`** | `^4` | Tailwind CSS v4 integration via PostCSS for utility-first styling. |
| **`typescript` and `@types/*`** | `^5` | Strict static typing and build-time verification. |

---

## Box Configuration (`boxconfig.yml`)

The `boxconfig.yml` file configures how Box CLI packages, bundles, and executes the Next.js application:

```yaml
name: ts-app
version: 0.1.0

runtime:
  type: node
  version: "26"

# Source protection
protect: false

# Working directory
workdir: .

# Next.js production server entrypoint
entrypoint: node_modules/next/dist/bin/next start --port 3000

# Production dependencies
dependencies: package.json

# Source files to include
include:
  - .next/
  - public/
  - next.config.ts
  - package.json
```

### Configuration Breakdown

1. **`name` and `version`**
   - Defines the canonical package identifier (`ts-app`) and its semantic version (`0.1.0`) when building or publishing to a Box registry.

2. **`runtime` (`type: node`, `version: "26"`)**
   - Instructs Box CLI to provision a hermetic **Node.js 26** engine inside the isolated execution sandbox.

3. **`protect: false`**
   - Disables V8 bytecode compilation (`.jsc`) and raw source stripping.
   - *Why?* Next.js compiles code into dynamic chunks inside `.next/` with complex module resolution and client-side JavaScript bundles sent to browsers. Stripping `.js` files or compiling server entrypoints into opaque V8 bytecode can disrupt Next.js's internal runtime chunk loaders.

4. **`workdir: .`**
   - Sets the root working directory inside the sandbox environment.

5. **`entrypoint: node_modules/next/dist/bin/next start --port 3000`**
   - Directly executes the Next.js production server binary provided by `node_modules` and binds it to port `3000`.

6. **`dependencies: package.json`**
   - Tells Box CLI to resolve production dependencies declared in `package.json` and package them into the `.box` archive.

7. **`include`**
   - Explicitly defines the essential files and directories to bundle into the archive:
     - `.next/`: Pre-compiled Next.js production build (server pages, static HTML, chunks, and CSS).
     - `public/`: Static assets such as images and icons served at runtime.
     - `next.config.ts`: Runtime configuration read by the Next.js server.
     - `package.json`: Manifest metadata and module definitions.

---

## Step-by-Step Build and Run Guide

### Quick Run from Box Hub

Execute this package directly from the public registry without manual local builds:

```bash
box run tanguykonan/ts-app
```

Reference: [Box Hub • @tanguykonan/ts-app](https://boxhub.paxiz.org/box/tanguykonan/ts-app)

---

### Local Packaging Workflow

#### 1. Install Dependencies

Install all dependencies including development tools:

```bash
npm install
```

#### 2. Build the Next.js Application

Generate the `.next/` production build output:

```bash
npm run build
```

> **Important**: The `.next/` directory must be generated before packaging, as declared in `boxconfig.yml` under `include: [ .next/ ]`.

#### 3. Package with Box CLI

Package the application into an immutable `.box` container:

```bash
box build . -o ts-app.box
```

#### 4. Run in the Box Sandbox

Launch the application inside an isolated Box sandbox:

```bash
box run ts-app.box
```

Once running, access the application in your browser at:
`http://localhost:3000`

#### 5. Inspect Archive Manifest and Metadata (Optional)

Inspect the integrity checksums, runtime settings, and packaged contents:

```bash
box info ts-app.box
```

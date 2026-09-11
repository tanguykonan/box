# Boxconfig manifest schema and field reference

This document defines the complete YAML specification, field constraints, data types, and default values for the `boxconfig.yml` application manifest.

## Manifest specification

The `boxconfig.yml` file defines how the application source tree is collected, protected, and executed within the isolated runtime container.

```yaml
name: service-template
version: 1.0.0
description: Autonomous microservice container template
category: WEB_APIS
protect: true
private: true
entrypoint: src/server.js
workdir: .

runtime:
  type: node
  version: "20.11"

dependencies: package.json

include:
  - src/
  - public/

exclude:
  - tests/
  - .env.local

env_schema: env.example

env:
  PORT:
    description: Primary network port
    default: "8080"
    required: false
  DATABASE_URL:
    description: Production database URI
    required: true

volumes:
  storage_data:
    mount: /app/storage
    description: Persistent file storage
```

## Field definition table

The following table details all supported fields, data types, necessity levels, and default behaviors parsed by `src/manifest.rs` and `src/builder.rs`.

| Field name | Data type | Necessity | Technical role | Default behavior |
| :--- | :--- | :--- | :--- | :--- |
| `name` | String | Required | Canonical package identifier | None (halts build if omitted) |
| `version` | String | Optional | Semantic version string | `1.0.0` |
| `description` | String | Optional | Human-readable package summary | Empty string |
| `category` | String | Optional | Catalog taxonomy classification | `BOTS` |
| `protect` | Boolean | Optional | Bytecode compilation and source stripping | `false` |
| `private` | Boolean | Optional | Registry visibility permission flag | `true` |
| `entrypoint` | String | Required | Relative path to execution startup script | None (halts build if omitted) |
| `workdir` | String | Optional | Base working directory inside sandbox | `.` |
| `runtime` | Object | Optional | Runtime interpreter type and version | Python 3.12 default |
| `runtime.type` | String | Optional | Target engine (`python` or `node`) | `python` |
| `runtime.version` | String | Optional | Runtime semantic version constraint | `3.12` (Python) / `20.11` (Node) |
| `dependencies` | String or List | Optional | Package specification file or inline list | Auto-detected from directory |
| `include` | List of Strings | Optional | Explicit file and directory inclusion list | `src/`, `app/`, and `entrypoint` |
| `exclude` | List of Strings | Optional | Explicit pattern exclusion list | Standard VCS and cache folders |
| `env_schema` | String | Optional | Path to external environment schema template | None |
| `env` | Object or List | Optional | Expected environment variables and rules | Empty map |
| `volumes` | Object or List | Optional | Persistent volume declarations and mounts | Empty map |

## Runtime configuration object

The `runtime` section informs the engine which hermetic interpreter to provision in the sandbox.

```yaml
runtime:
  type: python
  version: "3.12"
```

Supported runtime engines:

- `python`: Standalone CPython distribution. Supported versions include `3.11` and `3.12`.
- `node`: Standalone Node.js distribution. Supported versions include `18.20`, `20.11`, and `22.0`.

## Dependency resolution models

Dependencies can be declared using either an external manifest reference or inline package arrays.

### External dependency file reference

```yaml
# Node.js external resolution
dependencies: package.json

# Python external resolution
dependencies: requirements.txt
```

### Inline dependency list

```yaml
# Node.js inline packages
dependencies:
  - express@4.19.2
  - dotenv@16.4.5

# Python inline packages
dependencies:
  - fastapi==0.110.0
  - uvicorn==0.28.0
```

## Environment variable declaration models

Environment requirements can be declared via structured maps or shorthand list syntax.

### Structured map format

```yaml
env:
  API_KEY:
    description: Production API authentication key
    required: true
  CACHE_TTL:
    description: Cache lifetime in seconds
    default: "3600"
    required: false
```

### Shorthand array format

```yaml
env:
  - API_KEY
  - CACHE_TTL=3600
```

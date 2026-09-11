# Persistent volumes configuration schema

This document defines the YAML schema, data structures, and mount path rules for declaring persistent volumes in the `boxconfig.yml` application manifest.

## Volume specification syntax

Applications declare stateful storage requirements in the `volumes` section of `boxconfig.yml`. The engine parses both structured object mappings and shorthand array syntax.

### Structured map specification

```yaml
name: stateful-service
version: 1.0.0
runtime:
  type: python
  version: "3.12"
entrypoint: main.py

volumes:
  database_storage:
    mount: /app/data
    description: SQLite database files and session state
  upload_cache:
    mount: /app/public/uploads
    description: User uploaded assets and media cache
```

### Shorthand list specification

```yaml
volumes:
  - database_storage:/app/data
  - upload_cache:/app/public/uploads
```

## Field definition table

The following table defines the volume configuration fields parsed by `src/manifest.rs` into the internal `VolumeSpec` structure.

| Field name | Data type | Necessity | Technical role | Default behavior |
| :--- | :--- | :--- | :--- | :--- |
| `volume_name` | String (Key) | Required | Unique alphanumeric volume identifier | Key must contain only alphanumeric, `-`, or `_` |
| `mount` | String | Required | Absolute or relative mount path inside sandbox | Required (halts parsing if missing) |
| `description` | String | Optional | Documentation describing volume contents | Empty string |

## Mount path resolution rules

When the engine synchronizes declared volumes during sandbox initialization, mount paths are resolved according to standard normalization rules:

1. **Path sanitization**: Leading slashes (`/` or `\`) are stripped to prevent path traversal outside the ephemeral sandbox root.
2. **Directory hierarchy**: Intermediate parent directories within the sandbox (such as `/app/public/uploads`) are automatically created if they do not exist.
3. **Multi-mount isolation**: Multiple volumes declared in the same manifest must target distinct mount paths to avoid write collisions during bidirectional synchronization.

## Internal data representation

In the Rust codebase, the `volumes` section is deserialized into a hash map of `VolumeSpec` structs:

```rust
pub struct VolumeSpec {
    pub mount: String,
    pub description: Option<String>,
}
```

The deserializer (`deserialize_volumes` in `src/manifest.rs`) accepts both JSON objects and string arrays, converting colon-delimited items into canonical `VolumeSpec` records.

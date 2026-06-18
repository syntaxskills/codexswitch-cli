# JSON Output Contract

`--json` uses schema version 1 for every command. Successes are written to
stdout and failures are written to stderr with a non-zero exit status.

```json
{
  "schema_version": 1,
  "command": "list",
  "success": true,
  "data": {
    "profiles": []
  },
  "error": null
}
```

Failures use the same envelope:

```json
{
  "schema_version": 1,
  "command": "delete",
  "success": false,
  "data": null,
  "error": {
    "message": "Label 'missing' was not found."
  }
}
```

## Compatibility Policy

- Consumers must check `schema_version` before reading `data`.
- Schema version 1 may gain optional fields.
- Existing fields will not be removed, renamed, or change meaning within
  schema version 1.
- A breaking JSON change requires a new schema version and a new major CLI
  release.
- Command-specific payloads always live under `data`; their documented field
  names remain part of the schema.
- Usage windows include `left_percent`, `reset_at`, and `window_seconds`.

CodexSwitch CLI 2.0.0 and earlier emitted command-specific top-level JSON
shapes and plain-text errors. Consumers upgrading to the next major release
should migrate top-level payload reads to `data` and read failures from
`error.message`.

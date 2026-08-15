# JSONata Transform — `dev.mcpg.transform.jsonata`

> class `transform` · `native` · package `mcpg-plugin-transform-jsonata` · artifact `libmcpg_plugin_transform_jsonata.so` · Apache-2.0

Transform plugin that applies an operator-supplied [JSONata](https://jsonata.org)
expression to a JSON value. JSONata is a query and reshaping language for JSON:
one expression projects fields, filters arrays, aggregates with functions like
`$sum` and `$count`, and builds an entirely new object shape. Evaluation is pure
compute — no I/O, no host calls, no network — so a single registered instance
serves both the gateway's global transform chain and pipeline steps. Reach for
it when a payload needs restructuring rather than validation or text rendering:
renaming and flattening fields, projecting an array of ids out of an array of
records, or adapting one step's output to the next step's input shape.

## What it does
- Parses and evaluates a JSONata expression against the input value, returning the expression's result as the new value.
- Projects, filters, aggregates, and reshapes in a single expression (`orders.id`, `$sum(items.qty)`, `{ "names": items.name }`).
- Fires on tool arguments, on tool results, or on both, selected by `phase`.
- Rejects any result larger than `max_output_bytes`, so a fan-out expression such as `[1..1000000]` cannot exhaust gateway memory.
- Reports a parse error, an evaluation error, and a config error distinctly, each as a transform error rather than a silent pass-through.
- Declares no `required_capabilities` — it never calls back into the host for network, filesystem, or secret access.

## Configuration
Loaded from the flat top-level `plugins:` list. An entry there joins the global
transform chain and sees every tool call; the same registered plugin can also be
named by a pipeline `plugin_transform` step for a single binding.

```yaml
plugins:
  - id: dev.mcpg.transform.jsonata
    class: transform
    source: { oci: ghcr.io/mcpg-dev/source-code/plugins/transform-jsonata:protocol-1 }
    config:
      phase: arguments
      expression: '{ "names": items.name, "total": $sum(items.qty) }'
```

| Field | Type | Default | Description |
|---|---|---|---|
| `expression` | string | *(required)* | The JSONata expression evaluated against the input value. |
| `phase` | `arguments` \| `result` \| `both` | `both` | Which dispatch phase the global chain fires this transform on. A pipeline step always dispatches through the result path, so `arguments` there turns the step into a no-op. |
| `max_output_bytes` | integer | `1048576` | Reject results whose serialised size exceeds this. |

In the global chain the pre-dispatch value is the tool's `arguments` object and
the post-dispatch value is the serialised tool result — `content`, optional
`structuredContent`, `isError` — so a `phase: result` expression reads
`structuredContent.…` and has to rebuild that envelope, because the expression's
result replaces the whole value.

Unknown fields are rejected, so a mistyped key fails the transform instead of
being silently ignored.

Referenced from a pipeline instead, the plugin receives the whole pipeline
context as its input value — `arguments`, `tool_name`, `steps`, and `context`
(with `principal_id`, `trust_level`, `session_id`, `transport`, `roles`,
`groups`, `scopes`, and `attributes`). Each completed step appears as
`steps.<id>.output` alongside `steps.<id>.is_error` and
`steps.<id>.duration_ms`, so one expression can adapt a previous step's result
into the next step's input:

```yaml
mcp:
  capabilities:
    tools:
      - name: orders.flow
        description: Fetch orders, then reshape them for the enrichment step.
        backend:
          kind: pipeline
          steps:
            - kind: http
              id: fetch
              url: https://orders.example.com/list
            - kind: plugin_transform
              id: reshape
              plugin: dev.mcpg.transform.jsonata
              config:
                expression: '{ "ids": steps.fetch.output.orders.id }'
```

## Observability
Every application through the global chain increments
`mcpg_transform_applies_total` (labels `plugin_id`, `phase` of `pre` or `post`,
`outcome` of `unchanged`, `modified`, or `error`) and records
`mcpg_transform_apply_ms`. A modification also emits the
`mcpg.transform.applied` audit event, which carries hashes and byte counts of
the before and after values rather than their plaintext.

A transform error is not fatal in the global chain: the gateway logs a warning
and carries the last good value forward. Inside a pipeline the same error fails
the step, so a pipeline is the right place to put an expression whose failure
must stop the call.

## Build
Default feature set is OFF (avoids `mcpg_plugin_register` linker
collisions in the workspace build); opt in to the cdylib:

```bash
cargo build -p mcpg-plugin-transform-jsonata --features cdylib-export --release   # → target/release/libmcpg_plugin_transform_jsonata.so
```

## Sign & load (production)
Sign the artifact, pin/verify via the entry's `signature:` block, and honour
revocations. See <https://mcpg.dev/docs/security/plugin-security>.

## See also
- Pipeline step reference: <https://mcpg.dev/docs/reference/pipeline-steps>
- What a plugin is and how the ABI works: <https://mcpg.dev/docs/plugins/plugins-and-protocol>
- Sibling transforms: `libs/plugins/transform/template`, `libs/plugins/transform/csv`, `libs/plugins/transform/xml`
- Validate rather than reshape: `libs/plugins/transform/json-schema`

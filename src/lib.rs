//! JSONata transform plugin.
//!
//! Applies a configured JSONata expression to a JSON value. Stateless apart
//! from the manifest — the expression + options arrive per call in `config`,
//! so one instance serves both the global transform chain (pre/post dispatch)
//! and the pipeline `plugin_transform` bridge. Pure compute; no host calls.

use mcpg_plugin_protocol::{PluginContext, PluginManifest, TransformResult, firstparty_manifest};
use mcpg_plugin_sdk::ffi::SyncTransform;
use serde::Deserialize;
use serde_json::Value;

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

/// Which dispatch phase(s) a global transform fires on. Ignored by the
/// pipeline bridge (the host calls `transform_result` directly there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Arguments,
    Result,
    #[default]
    Both,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonataConfig {
    /// The JSONata expression to evaluate against the input value.
    expression: String,
    #[serde(default)]
    phase: Phase,
    #[serde(default = "default_max_output_bytes")]
    max_output_bytes: usize,
}

fn default_max_output_bytes() -> usize {
    DEFAULT_MAX_OUTPUT_BYTES
}

pub struct JsonataTransform {
    manifest: PluginManifest,
}

impl JsonataTransform {
    pub fn new(_config_json: &str) -> Self {
        Self {
            manifest: firstparty_manifest! {
                id: "dev.mcpg.transform.jsonata",
                name: "JSONata Transform",
                class: Transform,
            },
        }
    }

    fn run(&self, value: &Value, config: &Value, phase: Phase) -> TransformResult {
        let cfg: JsonataConfig = match serde_json::from_value(config.clone()) {
            Ok(c) => c,
            Err(e) => {
                return TransformResult::Error {
                    message: format!("jsonata transform config: {e}"),
                };
            }
        };
        // Global-mode phase gating; pipeline-mode always calls transform_result.
        if cfg.phase != Phase::Both && cfg.phase != phase {
            return TransformResult::Unchanged;
        }
        match apply_jsonata(&cfg.expression, value, cfg.max_output_bytes) {
            Ok(value) => TransformResult::Modified { value },
            Err(message) => TransformResult::Error { message },
        }
    }
}

impl SyncTransform for JsonataTransform {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn transform_arguments(
        &self,
        _ctx: &PluginContext,
        arguments: &Value,
        config: &Value,
    ) -> TransformResult {
        self.run(arguments, config, Phase::Arguments)
    }

    fn transform_result(
        &self,
        _ctx: &PluginContext,
        result: &Value,
        config: &Value,
    ) -> TransformResult {
        self.run(result, config, Phase::Result)
    }
}

/// Evaluate `expression` against `input`, returning the transformed JSON.
/// Bounded by `max_output_bytes` so a fan-out expression can't exhaust memory.
fn apply_jsonata(
    expression: &str,
    input: &Value,
    max_output_bytes: usize,
) -> Result<Value, String> {
    let ast = jsonata_core::parser::parse(expression).map_err(|e| format!("parse error: {e}"))?;
    let input_str = serde_json::to_string(input).map_err(|e| format!("input encode: {e}"))?;
    let data = jsonata_core::value::JValue::from_json_str(&input_str)
        .map_err(|e| format!("input decode: {e}"))?;
    let out = jsonata_core::evaluator::Evaluator::new()
        .evaluate(&ast, &data)
        .map_err(|e| format!("evaluation error: {e}"))?;
    let out_str = out
        .to_json_string()
        .map_err(|e| format!("output encode: {e}"))?;
    if out_str.len() > max_output_bytes {
        return Err(format!(
            "jsonata output {} bytes exceeds max_output_bytes ({max_output_bytes})",
            out_str.len()
        ));
    }
    serde_json::from_str(&out_str).map_err(|e| format!("output decode: {e}"))
}

// cdylib export — gated so a plain workspace build emits only the rlib (no
// duplicate `mcpg_plugin_register` symbol across plugin crates).
#[cfg(any(feature = "cdylib-export", feature = "static-firstparty"))]
mcpg_plugin_sdk::declare_plugin! {
    plugin_id: "dev.mcpg.transform.jsonata",
    plugin_version: env!("CARGO_PKG_VERSION"),
    descriptor_yaml: include_str!("../plugin.yaml"),
    capabilities: &[],
    entities: [
        transform as xform {
            inner_name: "",
            plugin_type: JsonataTransform,
            factory: |cfg: &str, _host: ::mcpg_plugin_sdk::HostHandle| JsonataTransform::new(cfg),
        },
    ],
}

#[cfg(test)]
mod tests;

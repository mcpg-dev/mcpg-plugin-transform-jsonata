use mcpg_plugin_protocol::{PluginContext, PluginIdentity, TransformResult};
use mcpg_plugin_sdk::ffi::SyncTransform;
use serde_json::json;

use super::JsonataTransform;

fn ctx() -> PluginContext {
    PluginContext {
        request_id: "t".into(),
        session_id: None,
        tool_name: "x".into(),
        surface: "tool".into(),
        identity: PluginIdentity {
            kind: "anonymous".into(),
            trust_level: "unauthenticated".into(),
            subject_id: None,
            auth_provider: None,
            issuer: None,
            roles: Vec::new(),
            groups: Vec::new(),
            scopes: Vec::new(),
            attributes: Default::default(),
        },
        transport: "http".into(),
    }
}

#[test]
fn restructures_and_aggregates() {
    let p = JsonataTransform::new("{}");
    let cfg = json!({ "expression": r#"{ "names": items.name, "total": $sum(items.qty), "count": $count(items) }"# });
    let input = json!({ "items": [ {"name":"a","qty":2}, {"name":"b","qty":3} ] });
    match p.transform_result(&ctx(), &input, &cfg) {
        TransformResult::Modified { value } => {
            assert_eq!(value, json!({ "names": ["a","b"], "total": 5, "count": 2 }));
        }
        other => panic!("expected Modified, got {other:?}"),
    }
}

#[test]
fn projects_array_field() {
    let p = JsonataTransform::new("{}");
    let cfg = json!({ "expression": "orders.id" });
    let input = json!({ "orders": [ {"id":1}, {"id":2}, {"id":3} ] });
    match p.transform_result(&ctx(), &input, &cfg) {
        TransformResult::Modified { value } => assert_eq!(value, json!([1, 2, 3])),
        other => panic!("expected Modified, got {other:?}"),
    }
}

#[test]
fn phase_gating() {
    let p = JsonataTransform::new("{}");
    let cfg = json!({ "expression": "x", "phase": "result" });
    // phase=result: transform_arguments is a no-op, transform_result fires.
    assert!(matches!(
        p.transform_arguments(&ctx(), &json!({"x":1}), &cfg),
        TransformResult::Unchanged
    ));
    assert!(matches!(
        p.transform_result(&ctx(), &json!({"x":1}), &cfg),
        TransformResult::Modified { .. }
    ));
}

#[test]
fn missing_expression_is_error() {
    let p = JsonataTransform::new("{}");
    assert!(matches!(
        p.transform_result(&ctx(), &json!({}), &json!({})),
        TransformResult::Error { .. }
    ));
}

#[test]
fn output_cap_is_enforced() {
    let p = JsonataTransform::new("{}");
    // A range fans out well beyond the tiny cap.
    let cfg = json!({ "expression": "[1..1000]", "max_output_bytes": 16 });
    assert!(matches!(
        p.transform_result(&ctx(), &json!({}), &cfg),
        TransformResult::Error { .. }
    ));
}

#[test]
fn malformed_expression_is_error() {
    let p = JsonataTransform::new("{}");
    let cfg = json!({ "expression": "{ this is not jsonata (" });
    assert!(matches!(
        p.transform_result(&ctx(), &json!({}), &cfg),
        TransformResult::Error { .. }
    ));
}

#[test]
fn unknown_config_key_is_rejected() {
    // deny_unknown_fields: a typo'd / stray config key must fail the parse
    // (fail-closed) rather than being silently ignored.
    let p = JsonataTransform::new("{}");
    let cfg = json!({ "expression": "x", "phasee": "result" });
    assert!(matches!(
        p.transform_result(&ctx(), &json!({"x":1}), &cfg),
        TransformResult::Error { .. }
    ));
}

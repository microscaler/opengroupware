//! JSON Schema validation for config-compiler output.
//!
//! Pipeline step 2 (docs/02, ADR-0009): the compiler renders desired state →
//! generated config, then this validates *structural correctness* against a
//! JSON Schema before the config is serialized and applied. A rendered config
//! that fails validation is never written — desired-state bugs are caught here
//! rather than by the downstream service.

use serde_json::Value;

/// Validate `data` against JSON Schema `schema`.
///
/// # Errors
/// Returns a human-readable string listing every violation (joined by `; `),
/// or a compile error when `schema` is not a valid JSON Schema.
pub fn validate(schema: &Value, data: &Value) -> Result<(), String> {
    let compiled =
        jsonschema::JSONSchema::compile(schema).map_err(|e| format!("invalid schema: {e}"))?;
    // Bind the validation result to a named local so its (borrowing) temporary
    // is dropped before `compiled` at end of scope — otherwise the tail `match`
    // keeps the borrow alive past `compiled`'s drop (E0597).
    let result = compiled.validate(data);
    match result {
        Ok(()) => Ok(()),
        Err(errors) => {
            let msgs: Vec<String> = errors
                .map(|e| format!("{e} (at {})", e.instance_path))
                .collect();
            Err(msgs.join("; "))
        }
    }
}

/// Deprecated stub alias kept for source compatibility; delegates to
/// [`validate`].
///
/// # Errors
/// See [`validate`].
pub fn validate_json_schema(schema: &Value, data: &Value) -> Result<(), String> {
    validate(schema, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "required": ["version", "items"],
            "properties": {
                "version": { "type": "integer", "minimum": 1 },
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["name"],
                        "properties": { "name": { "type": "string", "minLength": 1 } }
                    }
                }
            }
        })
    }

    #[test]
    fn valid_document_passes() {
        let data = json!({ "version": 1, "items": [{ "name": "a" }] });
        assert!(validate(&schema(), &data).is_ok());
    }

    #[test]
    fn missing_required_field_fails() {
        let data = json!({ "items": [{ "name": "a" }] });
        let err = validate(&schema(), &data).err().unwrap_or_default();
        assert!(!err.is_empty(), "expected a validation error");
        assert!(
            err.contains("version"),
            "error should mention the missing field: {err}"
        );
    }

    #[test]
    fn wrong_type_fails() {
        let data = json!({ "version": "one", "items": [{ "name": "a" }] });
        assert!(validate(&schema(), &data).is_err());
    }

    #[test]
    fn empty_array_violates_min_items() {
        let data = json!({ "version": 1, "items": [] });
        assert!(validate(&schema(), &data).is_err());
    }
}

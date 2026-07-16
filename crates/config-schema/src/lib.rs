// Config schema validation — stub.

pub const fn validate_json_schema(
    _schema: &serde_json::Value,
    _data: &serde_json::Value,
) -> Result<(), String> {
    Ok(())
}

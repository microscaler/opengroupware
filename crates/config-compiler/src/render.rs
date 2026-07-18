//! Desired-state → Rspamd settings compilation (ADR-0003, ADR-0009).
//!
//! The control plane owns desired state (tenants + domains); Rspamd must not be
//! hand-configured. This module reads active tenants/domains, renders a
//! per-tenant Rspamd `settings` document, validates its structure against a
//! JSON Schema (config-schema), and only then serializes it to UCL and writes
//! it atomically to the path Rspamd includes. Every compile is audited.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use og_db::{record, AuditEntry, Db};
use serde_json::{json, Value};

/// Default Rspamd action thresholds applied per tenant. These are sane
/// starting points; a future per-tenant policy model (control-plane column)
/// will override them without changing the rendering contract.
const REJECT: f64 = 15.0;
const ADD_HEADER: f64 = 6.0;
const GREYLIST: f64 = 4.0;

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("rendered config failed schema validation: {0}")]
    Schema(String),
    #[error("writing {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

/// One tenant's routing policy: which recipient domains map to its settings.
struct TenantPolicy {
    slug: String,
    settings_id: String,
    domains: Vec<String>,
}

/// Run one compile cycle. Returns the number of tenants written.
///
/// # Errors
/// [`CompileError`] on DB failure, schema-validation failure, or write failure.
pub async fn compile_once(db: &Db, out_path: &str) -> Result<usize, CompileError> {
    let policies = load_policies(db).await?;

    let doc = render_doc(&policies);
    config_schema::validate(&settings_schema(), &doc).map_err(CompileError::Schema)?;

    let ucl = render_ucl(&policies);
    write_atomic(out_path, &ucl).map_err(|source| CompileError::Io {
        path: out_path.to_string(),
        source,
    })?;

    let domains: usize = policies.iter().map(|p| p.domains.len()).sum();
    let mut tx = db.platform_tx().await?;
    record(
        &mut tx,
        AuditEntry {
            tenant_id: None,
            actor: "config-compiler",
            action: "config.compiled",
            entity_type: "rspamd_settings",
            entity_id: out_path.to_string(),
            payload: json!({ "tenants": policies.len(), "domains": domains }),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(policies.len())
}

/// Read active tenants + their active domains, grouped per tenant.
async fn load_policies(db: &Db) -> Result<Vec<TenantPolicy>, sqlx::Error> {
    let rows: Vec<(String, String)> = {
        let mut tx = db.platform_tx().await?;
        let rows = sqlx::query_as(
            "SELECT t.slug, d.fqdn
             FROM tenants t
             JOIN domains d ON d.tenant_id = t.id
             WHERE t.status = 'active' AND d.status = 'active'
             ORDER BY t.slug, d.fqdn",
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows
    };

    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (slug, fqdn) in rows {
        grouped.entry(slug).or_default().push(fqdn);
    }

    Ok(grouped
        .into_iter()
        .map(|(slug, domains)| TenantPolicy {
            settings_id: settings_id(&slug),
            slug,
            domains,
        })
        .collect())
}

/// Rspamd settings id: a stable, ucl-safe identifier per tenant slug.
fn settings_id(slug: &str) -> String {
    format!("tenant_{}", slug.replace('-', "_"))
}

/// The JSON structure validated before serialization to UCL.
fn render_doc(policies: &[TenantPolicy]) -> Value {
    let tenants: Vec<Value> = policies
        .iter()
        .map(|p| {
            json!({
                "slug": p.slug,
                "settings_id": p.settings_id,
                "domains": p.domains,
                "actions": { "reject": REJECT, "add_header": ADD_HEADER, "greylist": GREYLIST },
            })
        })
        .collect();
    json!({ "version": 1, "tenants": tenants })
}

/// JSON Schema for the rendered settings document.
fn settings_schema() -> Value {
    json!({
        "type": "object",
        "required": ["version", "tenants"],
        "properties": {
            "version": { "type": "integer", "minimum": 1 },
            "tenants": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["slug", "settings_id", "domains", "actions"],
                    "properties": {
                        "slug": { "type": "string", "minLength": 1 },
                        "settings_id": { "type": "string", "pattern": "^tenant_[a-z0-9_]+$" },
                        "domains": {
                            "type": "array",
                            "minItems": 1,
                            "items": { "type": "string", "minLength": 1 }
                        },
                        "actions": {
                            "type": "object",
                            "required": ["reject", "add_header", "greylist"],
                            "properties": {
                                "reject": { "type": "number" },
                                "add_header": { "type": "number" },
                                "greylist": { "type": "number" }
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Serialize the policies to a Rspamd `settings {}` UCL block.
fn render_ucl(policies: &[TenantPolicy]) -> String {
    let mut s = String::new();
    s.push_str("# GENERATED by opengroupware config-compiler — DO NOT EDIT.\n");
    s.push_str("# Source: control-plane desired state (active tenants + domains).\n");
    s.push_str("settings {\n");
    for p in policies {
        let rcpt = p
            .domains
            .iter()
            .map(|d| format!("\"{d}\""))
            .collect::<Vec<_>>()
            .join(", ");
        // write! into a String is infallible; discard the Result.
        let _ = write!(
            s,
            "  {sid} {{\n    \
             priority = 10;\n    \
             rcpt = [{rcpt}];\n    \
             apply {{\n      actions {{\n        \
             reject = {REJECT};\n        \
             add_header = {ADD_HEADER};\n        \
             greylist = {GREYLIST};\n      \
             }}\n    }}\n  }}\n",
            sid = p.settings_id,
        );
    }
    s.push_str("}\n");
    s
}

/// Write `contents` to `path` atomically (temp file + rename), creating the
/// parent directory if needed.
fn write_atomic(path: &str, contents: &str) -> std::io::Result<()> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = format!("{path}.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.flush()?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TenantPolicy> {
        vec![
            TenantPolicy {
                slug: "acme-corp".to_string(),
                settings_id: settings_id("acme-corp"),
                domains: vec!["acme.example".to_string(), "mail.acme.example".to_string()],
            },
            TenantPolicy {
                slug: "globex".to_string(),
                settings_id: settings_id("globex"),
                domains: vec!["globex.example".to_string()],
            },
        ]
    }

    #[test]
    fn settings_id_is_ucl_safe() {
        assert_eq!(settings_id("acme-corp"), "tenant_acme_corp");
    }

    #[test]
    fn rendered_doc_passes_its_own_schema() {
        let doc = render_doc(&sample());
        assert!(config_schema::validate(&settings_schema(), &doc).is_ok());
    }

    #[test]
    fn empty_desired_state_still_validates() {
        // No tenants → an empty settings block, still schema-valid.
        let doc = render_doc(&[]);
        assert!(config_schema::validate(&settings_schema(), &doc).is_ok());
    }

    #[test]
    fn ucl_contains_domains_and_settings_ids() {
        let ucl = render_ucl(&sample());
        assert!(ucl.contains("tenant_acme_corp {"));
        assert!(ucl.contains("\"acme.example\", \"mail.acme.example\""));
        assert!(ucl.contains("tenant_globex {"));
        assert!(ucl.contains("reject = 15"));
    }
}

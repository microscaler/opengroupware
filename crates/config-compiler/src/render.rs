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

/// Default Rspamd action thresholds. A tenant's `abuse_policy` row overrides
/// these (admin-api `PUT /tenants/{id}/policy`); a tenant without a row falls
/// back here via the query's COALESCE, so these values are the single source
/// of truth for "unset".
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

/// One tenant's routing policy: which recipient domains map to its settings,
/// and the action thresholds to apply.
struct TenantPolicy {
    slug: String,
    settings_id: String,
    domains: Vec<String>,
    reject: f64,
    add_header: f64,
    greylist: f64,
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

/// Read active tenants + their active domains (with each tenant's effective
/// abuse-policy thresholds, COALESCEd to the platform defaults), grouped per
/// tenant.
async fn load_policies(db: &Db) -> Result<Vec<TenantPolicy>, sqlx::Error> {
    // COALESCE against the module defaults so a tenant with no policy row
    // renders identically to one explicitly at defaults.
    let query = format!(
        "SELECT t.slug, d.fqdn,
                COALESCE(ap.reject, {REJECT}) AS reject,
                COALESCE(ap.add_header, {ADD_HEADER}) AS add_header,
                COALESCE(ap.greylist, {GREYLIST}) AS greylist
         FROM tenants t
         JOIN domains d ON d.tenant_id = t.id
         LEFT JOIN abuse_policy ap ON ap.tenant_id = t.id
         WHERE t.status = 'active' AND d.status = 'active'
         ORDER BY t.slug, d.fqdn"
    );
    let rows: Vec<(String, String, f64, f64, f64)> = {
        let mut tx = db.platform_tx().await?;
        let rows = sqlx::query_as(&query).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        rows
    };

    // Thresholds are per-tenant (constant across a tenant's domain rows); take
    // them from the first row seen for each tenant.
    let mut grouped: BTreeMap<String, (Vec<String>, f64, f64, f64)> = BTreeMap::new();
    for (slug, fqdn, reject, add_header, greylist) in rows {
        let entry = grouped
            .entry(slug)
            .or_insert_with(|| (Vec::new(), reject, add_header, greylist));
        entry.0.push(fqdn);
    }

    Ok(grouped
        .into_iter()
        .map(
            |(slug, (domains, reject, add_header, greylist))| TenantPolicy {
                settings_id: settings_id(&slug),
                slug,
                domains,
                reject,
                add_header,
                greylist,
            },
        )
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
                "actions": { "reject": p.reject, "add_header": p.add_header, "greylist": p.greylist },
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
             reject = {reject};\n        \
             add_header = {add_header};\n        \
             greylist = {greylist};\n      \
             }}\n    }}\n  }}\n",
            sid = p.settings_id,
            reject = p.reject,
            add_header = p.add_header,
            greylist = p.greylist,
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
            // acme at platform defaults
            TenantPolicy {
                slug: "acme-corp".to_string(),
                settings_id: settings_id("acme-corp"),
                domains: vec!["acme.example".to_string(), "mail.acme.example".to_string()],
                reject: REJECT,
                add_header: ADD_HEADER,
                greylist: GREYLIST,
            },
            // globex with a custom (stricter) policy
            TenantPolicy {
                slug: "globex".to_string(),
                settings_id: settings_id("globex"),
                domains: vec!["globex.example".to_string()],
                reject: 20.0,
                add_header: 8.0,
                greylist: 5.0,
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
    }

    #[test]
    fn ucl_reflects_per_tenant_thresholds() {
        let ucl = render_ucl(&sample());
        // acme at defaults, globex custom — both must appear.
        assert!(ucl.contains("reject = 15"), "acme default reject");
        assert!(ucl.contains("reject = 20"), "globex custom reject");
        assert!(ucl.contains("greylist = 5"), "globex custom greylist");
    }
}

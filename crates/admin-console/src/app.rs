//! Admin console views — server-rendered read-only pages over admin-api.
//!
//! No hydration, no client JS: each page fetches from admin-api and renders a
//! Leptos view to an HTML string (`Owner::new().with(|| view.to_html())`). The
//! views are pure functions of their data props, so no reactive runtime/async
//! executor is needed.

use leptos::prelude::*;

use crate::client::{AccountDto, DomainDto, PolicyDto, TenantDto};

const STYLE: &str = r"
* { box-sizing: border-box; }
body { margin:0; font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif; color:#1c1c1c; background:#f7f7f5; }
a { color:#185fa5; text-decoration:none; }
a:hover { text-decoration:underline; }
.nav { display:flex; align-items:center; gap:20px; padding:12px 24px; background:#fff; border-bottom:1px solid #e6e6e2; }
.nav .brand { font-weight:600; }
.wrap { max-width:900px; margin:24px auto; padding:0 24px; }
h1 { font-size:22px; font-weight:600; margin:0 0 16px; }
h2 { font-size:15px; font-weight:600; margin:22px 0 8px; color:#555; }
.card { background:#fff; border:1px solid #e6e6e2; border-radius:12px; overflow:hidden; }
table { width:100%; border-collapse:collapse; font-size:14px; }
th { text-align:left; font-weight:600; font-size:12px; color:#666; padding:10px 14px; }
td { padding:10px 14px; border-top:1px solid #efefeb; }
.mono { font-family:ui-monospace,SFMono-Regular,Menlo,monospace; }
.muted { color:#666; }
.right { text-align:right; }
.badge { display:inline-block; font-size:12px; padding:2px 9px; border-radius:20px; background:#eee; color:#444; }
.badge.ok { background:#e1f5ee; color:#0f6e56; }
.badge.warn { background:#faeeda; color:#854f0b; }
.headrow { display:flex; align-items:center; gap:12px; margin:4px 0 8px; }
.avatar { width:40px; height:40px; border-radius:8px; background:#e6f1fb; color:#185fa5; display:flex; align-items:center; justify-content:center; font-weight:600; }
.cards3 { display:grid; grid-template-columns:repeat(3,1fr); gap:10px; }
.metric { background:#f1efe8; border-radius:8px; padding:10px 12px; }
.metric .k { font-size:12px; color:#666; }
.metric .v { font-size:20px; font-weight:600; }
.err { background:#fcebeb; color:#a32d2d; border:1px solid #f09595; padding:12px 14px; border-radius:8px; }
.empty { padding:16px; color:#666; }
";

/// Wrap page `content` in the full HTML document (nav + styles) and render to
/// a string.
fn page(title: &str, content: impl IntoView + 'static) -> String {
    let doc_title = format!("{title} · opengroupware admin");
    let owner = Owner::new();
    owner.with(move || {
        view! {
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    <title>{doc_title}</title>
                    <style inner_html=STYLE></style>
                </head>
                <body>
                    <header class="nav">
                        <span class="brand">"opengroupware admin"</span>
                        <a href="/">"Tenants"</a>
                    </header>
                    <main class="wrap">{content}</main>
                </body>
            </html>
        }
        .to_html()
    })
}

/// A status pill, coloured by lifecycle state.
fn badge(status: &str) -> impl IntoView {
    let class = match status {
        "active" => "badge ok",
        s if s == "suspended" || s.starts_with("pending") => "badge warn",
        _ => "badge",
    };
    let text = status.to_string();
    view! { <span class=class>{text}</span> }
}

fn tenants_view(tenants: Vec<TenantDto>) -> impl IntoView {
    let empty = tenants.is_empty();
    let rows = tenants
        .into_iter()
        .map(|t| {
            let href = format!("/tenants/{}", t.id);
            view! {
                <tr>
                    <td class="mono"><a href=href>{t.slug}</a></td>
                    <td>{t.name}</td>
                    <td class="muted">{t.plan}</td>
                    <td>{badge(&t.status)}</td>
                </tr>
            }
        })
        .collect_view();
    view! {
        <h1>"Tenants"</h1>
        <div class="card">
            <table>
                <thead>
                    <tr>
                        <th>"Slug"</th>
                        <th>"Name"</th>
                        <th>"Plan"</th>
                        <th>"Status"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows}
                    {empty.then(|| view! {
                        <tr><td class="empty" colspan="4">"No tenants yet."</td></tr>
                    })}
                </tbody>
            </table>
        </div>
    }
}

fn detail_view(
    t: TenantDto,
    domains: Vec<DomainDto>,
    accounts: Vec<AccountDto>,
    policy: PolicyDto,
) -> impl IntoView {
    let initial = t
        .name
        .chars()
        .next()
        .map_or_else(|| "?".to_string(), |c| c.to_ascii_uppercase().to_string());
    let subtitle = format!("{} · {} · {}", t.slug, t.plan, t.status);
    let name = t.name;

    let domains_empty = domains.is_empty();
    let domain_rows = domains
        .into_iter()
        .map(|d| {
            view! {
                <tr>
                    <td class="mono">{d.fqdn}</td>
                    <td class="right">{badge(&d.status)}</td>
                </tr>
            }
        })
        .collect_view();

    let accounts_empty = accounts.is_empty();
    let account_rows = accounts
        .into_iter()
        .map(|a| {
            let quota = format!("{} MB", a.quota_mb);
            view! {
                <tr>
                    <td class="mono">{a.email}</td>
                    <td class="muted">{a.display_name}</td>
                    <td class="right muted">{quota}</td>
                    <td class="right">{badge(&a.status)}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <p><a href="/">"← Tenants"</a></p>
        <div class="headrow">
            <div class="avatar">{initial}</div>
            <div>
                <h1 style="margin:0">{name}</h1>
                <div class="muted mono">{subtitle}</div>
            </div>
        </div>

        <h2>"Domains"</h2>
        <div class="card">
            <table>
                <tbody>
                    {domain_rows}
                    {domains_empty.then(|| view! {
                        <tr><td class="empty">"No domains."</td></tr>
                    })}
                </tbody>
            </table>
        </div>

        <h2>"Accounts"</h2>
        <div class="card">
            <table>
                <thead>
                    <tr>
                        <th>"Email"</th>
                        <th>"Name"</th>
                        <th class="right">"Quota"</th>
                        <th class="right">"Status"</th>
                    </tr>
                </thead>
                <tbody>
                    {account_rows}
                    {accounts_empty.then(|| view! {
                        <tr><td class="empty" colspan="4">"No accounts."</td></tr>
                    })}
                </tbody>
            </table>
        </div>

        <h2>"Abuse policy"</h2>
        <div class="cards3">
            <div class="metric"><div class="k">"greylist"</div><div class="v">{fmt1(policy.greylist)}</div></div>
            <div class="metric"><div class="k">"add header"</div><div class="v">{fmt1(policy.add_header)}</div></div>
            <div class="metric"><div class="k">"reject"</div><div class="v">{fmt1(policy.reject)}</div></div>
        </div>
    }
}

fn error_view(title: &str, msg: &str) -> impl IntoView {
    let title = title.to_string();
    let msg = msg.to_string();
    view! {
        <h1>{title}</h1>
        <div class="err">{msg}</div>
        <p><a href="/">"← Back to tenants"</a></p>
    }
}

fn fmt1(v: f64) -> String {
    format!("{v:.1}")
}

// --- Public render entry points (called by the axum handlers) --------------

pub fn render_tenants_page(tenants: Vec<TenantDto>) -> String {
    page("Tenants", tenants_view(tenants))
}

pub fn render_detail_page(
    tenant: TenantDto,
    domains: Vec<DomainDto>,
    accounts: Vec<AccountDto>,
    policy: PolicyDto,
) -> String {
    let title = tenant.name.clone();
    page(&title, detail_view(tenant, domains, accounts, policy))
}

pub fn render_error_page(title: &str, msg: &str) -> String {
    page(title, error_view(title, msg))
}

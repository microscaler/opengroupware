// Admin console app — shared components, routes, and API helpers.

use leptos::prelude::*;
use leptos_router::components::*;
use types::Uuid;

// Re-export domain types for UI bindings.
pub use types::{Tenant, TenantConfig, Account, TenantResourceQuota};

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <AppShell>
                <Outlet />
            </AppShell>
        </Router>
    }
}

#[component]
fn AppShell(children: ChildrenFn) -> impl IntoView {
    let children = children();
    view! {
        <div class="app">
            <header class="app-header">
                <nav class="nav">
                    <a href="/">Tenants</a>
                    <a href="/accounts">Accounts</a>
                    <a href="/quotas">Quotas</a>
                    <a href="/audit">Audit Log</a>
                </nav>
            </header>
            <main class="app-main">
                {children}
            </main>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

#[component]
pub fn TenantsPage() -> impl IntoView {
    view! {
        <h1>"Tenants"</h1>
        <table>
            <thead>
                <tr>
                    <th>"Domain"</th>
                    <th>"Status"</th>
                    <th>"Accounts"</th>
                    <th>"Quota"</th>
                </tr>
            </thead>
            <tbody>
                // TODO: render tenant list from fetch_tenants()
                <tr>
                    <td colspan="4">"Loading…"</td>
                </tr>
            </tbody>
        </table>
    }
}

#[component]
pub fn AccountsPage() -> impl IntoView {
    view! {
        <h1>"Accounts"</h1>
        <p>"Account management loading…"</p>
    }
}

#[component]
pub fn QuotasPage() -> impl IntoView {
    view! {
        <h1>"Quotas"</h1>
        <p>"Resource quota management loading…"</p>
    }
}

#[component]
pub fn AuditPage() -> impl IntoView {
    view! {
        <h1>"Audit Log"</h1>
        <p>"Audit events loading…"</p>
    }
}

// ---------------------------------------------------------------------------
// API helpers
// ---------------------------------------------------------------------------

pub async fn fetch_tenants() -> Result<Vec<Tenant>, String> {
    // TODO: wire to admin-api endpoint
    Err("Not yet wired".to_string())
}

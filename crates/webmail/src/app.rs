// Webmail app — shared components, routes, and API helpers.

use leptos::prelude::*;
use leptos_router::components::{Outlet, Router};
use types::Uuid;

// Re-export domain types for UI bindings.
pub use types::{Account, Mailbox};

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
                    <a href="/">Mailbox</a>
                    <a href="/compose">Compose</a>
                    <a href="/contacts">Contacts</a>
                    <a href="/settings">Settings</a>
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
pub fn InboxPage() -> impl IntoView {
    view! {
        <h1>"Inbox"</h1>
        <p>"Message list loading…"</p>
    }
}

#[component]
pub fn ComposePage() -> impl IntoView {
    view! {
        <h1>"Compose"</h1>
        <form>
            <label>"To"</label>
            <input type="email" placeholder="user@example.com" />
            <label>"Subject"</label>
            <input type="text" placeholder="Subject" />
            <textarea rows="10" style="width:100%" placeholder="Body"></textarea>
            <button type="submit">"Send"</button>
        </form>
    }
}

#[component]
pub fn ContactsPage() -> impl IntoView {
    view! {
        <h1>"Contacts"</h1>
        <p>"Contact list loading…"</p>
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <h1>"Settings"</h1>
        <p>"Account settings loading…"</p>
    }
}

// ---------------------------------------------------------------------------
// API helpers — call backend wrappers
// ---------------------------------------------------------------------------

pub async fn fetch_accounts(tenant_id: Uuid) -> Result<Vec<Account>, String> {
    let _ = tenant_id;
    Err("Not yet wired".to_string())
}

pub async fn fetch_mailboxes(tenant_id: Uuid, account_id: Uuid) -> Result<Vec<Mailbox>, String> {
    let _ = (tenant_id, account_id);
    Err("Not yet wired".to_string())
}

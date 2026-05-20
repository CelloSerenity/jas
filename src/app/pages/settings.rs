use leptos::prelude::*;

use crate::app::get_server_config;

#[component]
pub fn Settings() -> impl IntoView {
    let config = Resource::new(|| (), |_| get_server_config());

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Settings"</h1>
            </div>

            <Suspense fallback=|| {
                view! { <p class="loading">"Loading..."</p> }
            }>
                {move || {
                    config
                        .get()
                        .map(|r| match r {
                            Err(e) => {
                                view! {
                                    <p class="error">"Error loading config: " {e.to_string()}</p>
                                }
                                    .into_any()
                            }
                            Ok(cfg) => {
                                view! {
                                    <div class="card">
                                        <h2>"Server"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Bind Address"</th>
                                                    <td class="mono">{cfg.bind}</td>
                                                </tr>
                                                <tr>
                                                    <th>"Log Level"</th>
                                                    <td>{cfg.log_level}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"Storage"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Database"</th>
                                                    <td class="mono">{cfg.database_path}</td>
                                                </tr>
                                                <tr>
                                                    <th>"IPA Directory"</th>
                                                    <td class="mono">{cfg.ipa_dir}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"Refresh Scheduler"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Check Interval"</th>
                                                    <td>{format!("Every {} hours", cfg.interval_hours)}</td>
                                                </tr>
                                                <tr>
                                                    <th>"Refresh Window"</th>
                                                    <td>
                                                        {format!("{} days before expiry", cfg.refresh_window_days)}
                                                    </td>
                                                </tr>
                                                <tr>
                                                    <th>"Worker Threads"</th>
                                                    <td>{cfg.worker_threads}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                    <div class="card">
                                        <h2>"mDNS Discovery"</h2>
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"Enabled"</th>
                                                    <td>{if cfg.mdns_enabled { "Yes" } else { "No" }}</td>
                                                </tr>
                                                {(!cfg.mdns_interface.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <tr>
                                                                <th>"Interface"</th>
                                                                <td class="mono">{cfg.mdns_interface}</td>
                                                            </tr>
                                                        }
                                                    })}
                                            </tbody>
                                        </table>
                                    </div>
                                    <p class="hint">
                                        "To change settings, edit " <code>"jas.toml"</code>
                                        " (or set " <code>"JAS_CONFIG"</code>
                                        " to a custom path) and restart JAS."
                                    </p>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

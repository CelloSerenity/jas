use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::app::components::confirm;
use crate::app::{
    get_device_info, job_status, list_accounts, list_apps, DeleteApp, InstallIpa, ReconcileApps,
    RefreshApp, SetRefreshEnabled,
};

#[component]
pub fn DeviceDetail() -> impl IntoView {
    let params = use_params_map();
    let device_id = move || {
        params
            .get()
            .get("id")
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let device = Resource::new(device_id, get_device_info);
    let apps = Resource::new(device_id, list_apps);

    let refresh_action = ServerAction::<RefreshApp>::new();
    let toggle_action = ServerAction::<SetRefreshEnabled>::new();
    let delete_app_action = ServerAction::<DeleteApp>::new();
    let reconcile_action = ServerAction::<ReconcileApps>::new();

    Effect::new(move |_| {
        if refresh_action.version().get() > 0
            || toggle_action.version().get() > 0
            || delete_app_action.version().get() > 0
            || reconcile_action.version().get() > 0
        {
            apps.refetch();
        }
    });

    view! {
        <div class="page">
            <Suspense fallback=|| {
                view! { <p class="loading">"Loading device…"</p> }
            }>
                {move || {
                    device
                        .get()
                        .map(|r: Result<crate::app::DeviceInfo, _>| match r {
                            Err(e) => {
                                view! { <p class="error">"Device error: " {e.to_string()}</p> }
                                    .into_any()
                            }
                            Ok(dev) => {
                                view! {
                                    <div class="page-header">
                                        <h1>{dev.name.clone()}</h1>
                                        <span class="badge badge-static">{dev.ip.clone()}</span>
                                    </div>
                                    <div class="card">
                                        <table class="info-table">
                                            <tbody>
                                                <tr>
                                                    <th>"UDID"</th>
                                                    <td class="mono">{dev.udid.clone()}</td>
                                                </tr>
                                                <tr>
                                                    <th>"IP"</th>
                                                    <td>{dev.ip.clone()}</td>
                                                </tr>
                                                <tr>
                                                    <th>"Port"</th>
                                                    <td>{dev.port}</td>
                                                </tr>
                                                <tr>
                                                    <th>"Discovery"</th>
                                                    <td>{dev.discovery.clone()}</td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>

            <section class="card">
                <h2>"Install IPA"</h2>
                <InstallForm
                    device_id=Signal::derive(device_id)
                    on_install=move || apps.refetch()
                />
            </section>

            <section class="card">
                <div class="page-header">
                    <h2>"Installed Apps"</h2>
                    <ActionForm action=reconcile_action>
                        <input type="hidden" name="device_id" value=device_id />
                        <button type="submit" class="btn btn-sm btn-secondary">
                            "Sync from device"
                        </button>
                    </ActionForm>
                </div>
                {move || {
                    reconcile_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(0) => {
                                view! {
                                    <div class="alert alert-success">"In sync with device."</div>
                                }
                                    .into_any()
                            }
                            Ok(n) => {
                                view! {
                                    <div class="alert alert-success">
                                        "Removed " {n} " stale entr"
                                        {if n == 1 { "y" } else { "ies" }} "."
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="alert alert-error">
                                        "Sync error: " {e.to_string()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
                <Suspense fallback=|| {
                    view! { <p class="loading">"Loading apps…"</p> }
                }>
                    {move || {
                        apps
                            .get()
                            .map(|r: Result<Vec<crate::app::AppInfo>, _>| match r {
                                Err(e) => {
                                    view! { <p class="error">"Error: " {e.to_string()}</p> }
                                        .into_any()
                                }
                                Ok(app_list) if app_list.is_empty() => {
                                    view! {
                                        <p class="muted">"No apps installed on this device."</p>
                                    }
                                        .into_any()
                                }
                                Ok(app_list) => {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"App"</th>
                                                    <th>"Bundle ID"</th>
                                                    <th>"Installed"</th>
                                                    <th>"Auto-Refresh"</th>
                                                    <th>"Actions"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {app_list
                                                    .into_iter()
                                                    .map(|app| {
                                                        let refresh_id = app.id.clone();
                                                        let toggle_id = app.id.clone();
                                                        let delete_id = app.id.clone();
                                                        let delete_msg = format!(
                                                            "Delete \"{}\" from this device?",
                                                            app.display_name,
                                                        );
                                                        let installed = app
                                                            .installed_at
                                                            .and_then(|ts| {
                                                                chrono::DateTime::from_timestamp(ts, 0)
                                                                    .map(|d| d.format("%Y-%m-%d").to_string())
                                                            });
                                                        let has_ipa = app.has_ipa;
                                                        let refresh_enabled = app.refresh_enabled;
                                                        view! {
                                                            <tr>
                                                                <td>
                                                                    <strong>{app.display_name.clone()}</strong>
                                                                    {app
                                                                        .version
                                                                        .as_ref()
                                                                        .map(|v| {
                                                                            view! { <span class="version">" v"{v.clone()}</span> }
                                                                        })}
                                                                </td>
                                                                <td class="mono">{app.bundle_id.clone()}</td>
                                                                <td>{installed.unwrap_or_else(|| "-".to_string())}</td>
                                                                <td>
                                                                    <ActionForm action=toggle_action>
                                                                        <input type="hidden" name="app_id" value=toggle_id />
                                                                        <input
                                                                            type="hidden"
                                                                            name="enabled"
                                                                            value=if refresh_enabled { "false" } else { "true" }
                                                                        />
                                                                        <button
                                                                            type="submit"
                                                                            class=if refresh_enabled {
                                                                                "btn btn-sm btn-success"
                                                                            } else {
                                                                                "btn btn-sm btn-secondary"
                                                                            }
                                                                        >
                                                                            {if refresh_enabled { "ON" } else { "OFF" }}
                                                                        </button>
                                                                    </ActionForm>
                                                                </td>
                                                                <td class="actions">
                                                                    {if has_ipa {
                                                                        view! {
                                                                            <ActionForm action=refresh_action>
                                                                                <input type="hidden" name="app_id" value=refresh_id />
                                                                                <button type="submit" class="btn btn-sm btn-primary">
                                                                                    "Refresh"
                                                                                </button>
                                                                            </ActionForm>
                                                                        }
                                                                            .into_any()
                                                                    } else {
                                                                        view! { <span class="muted">"No IPA"</span> }.into_any()
                                                                    }}
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&delete_msg) {
                                                                            delete_app_action
                                                                                .dispatch(DeleteApp {
                                                                                    app_id: delete_id.clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button type="submit" class="btn btn-sm btn-danger">
                                                                            "Delete"
                                                                        </button>
                                                                    </form>
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>

                {move || {
                    refresh_action
                        .value()
                        .get()
                        .map(|r| match r {
                            Ok(job_id) => {
                                view! {
                                    <div>
                                        <div class="alert alert-success">"Refresh queued."</div>
                                        <JobProgress
                                            job_id=job_id
                                            on_done=Callback::new(move |_| apps.refetch())
                                        />
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="alert alert-error">
                                        "Refresh error: " {e.to_string()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </section>
        </div>
    }
}

#[component]
fn InstallForm(device_id: Signal<String>, #[prop(into)] on_install: Callback<()>) -> impl IntoView {
    let accounts = Resource::new(|| (), |_| list_accounts());
    let install_action = ServerAction::<InstallIpa>::new();

    let ipa_b64 = RwSignal::new(String::new());
    let file_status = RwSignal::new(String::new());
    let account_id = RwSignal::new(String::new());
    let store = RwSignal::new(true);
    let active_job = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        if let Some(Ok(ref id)) = install_action.value().get() {
            active_job.set(Some(id.clone()));
            on_install.run(());
        }
    });

    let can_submit = move || !ipa_b64.get().is_empty() && !account_id.get().is_empty();

    view! {
        <div>
            <div class="form-row">
                <label class="form-field">
                    "Account"
                    <Suspense fallback=|| {
                        view! {
                            <select disabled>
                                <option>"Loading..."</option>
                            </select>
                        }
                    }>
                        {move || {
                            accounts
                                .get()
                                .map(|r| {
                                    let accs = r.unwrap_or_default();
                                    view! {
                                        <select on:change=move |e| {
                                            account_id.set(event_target_value(&e))
                                        }>
                                            <option value="">"- Select Account -"</option>
                                            {accs
                                                .into_iter()
                                                .map(|a| {
                                                    let label = match &a.team_name {
                                                        Some(t) => format!("{} ({})", a.apple_id, t),
                                                        None => a.apple_id.clone(),
                                                    };
                                                    view! { <option value=a.id>{label}</option> }
                                                })
                                                .collect_view()}
                                        </select>
                                    }
                                })
                        }}
                    </Suspense>
                </label>
                <label class="form-field">
                    "IPA File"
                    <input
                        type="file"
                        accept=".ipa"
                        on:change=move |e| {
                            #[cfg(target_arch = "wasm32")] read_file_b64(e, ipa_b64, file_status);
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let _ = e;
                            }
                        }
                    /> <small>{file_status}</small>
                </label>
                <label class="form-field checkbox">
                    <input
                        type="checkbox"
                        prop:checked=store
                        on:change=move |e| store.set(event_target_checked(&e))
                    />
                    " Store IPA for auto-refresh"
                </label>
            </div>
            <button
                class="btn btn-primary"
                prop:disabled=move || !can_submit()
                on:click=move |_| {
                    if !can_submit() {
                        return;
                    }
                    install_action
                        .dispatch(InstallIpa {
                            device_id: device_id.get_untracked(),
                            account_id: account_id.get_untracked(),
                            ipa_bytes_b64: ipa_b64.get_untracked(),
                            store: store.get_untracked(),
                        });
                }
            >
                "Install"
            </button>
        </div>

        {move || {
            install_action
                .value()
                .get()
                .map(|r| match r {
                    Err(e) => {
                        view! {
                            <div class="alert alert-error">"Install error: " {e.to_string()}</div>
                        }
                            .into_any()
                    }
                    Ok(_) => {
                        view! { <div class="alert alert-success">"Install queued."</div> }
                            .into_any()
                    }
                })
        }}

        {move || {
            active_job
                .get()
                .map(|id| {
                    view! {
                        <JobProgress job_id=id on_done=Callback::new(move |_| on_install.run(())) />
                    }
                })
        }}
    }
}

#[cfg(target_arch = "wasm32")]
fn read_file_b64(e: leptos::ev::Event, out: RwSignal<String>, status: RwSignal<String>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(input) = e
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };

    let Some(files) = input.files() else { return };
    let Some(file) = files.get(0) else { return };

    status.set(format!("Reading {}...", file.name()));
    let reader = web_sys::FileReader::new().expect("FileReader");
    let r2 = reader.clone();
    let cb: Closure<dyn FnMut()> = Closure::once(move || {
        if let Ok(v) = r2.result() {
            if let Some(data_url) = v.as_string() {
                if let Some(b64) = data_url.split(',').nth(1) {
                    out.set(b64.to_string());
                    status.set("File loaded.".to_string());
                }
            }
        }
    });
    reader.set_onload(Some(cb.as_ref().unchecked_ref()));
    reader.read_as_data_url(&file).expect("read_as_data_url");
    cb.forget();
}

#[component]
fn JobProgress(
    job_id: String,
    #[prop(optional, into)] on_done: Option<Callback<()>>,
) -> impl IntoView {
    let job_id = StoredValue::new(job_id);
    let trigger = RwSignal::new(0u32);
    let job = Resource::new(
        move || (job_id.get_value(), trigger.get()),
        |(id, _)| job_status(id),
    );

    let is_done = move || {
        job.get()
            .and_then(|r| r.ok())
            .map(|j| j.status == "done" || j.status == "failed")
            .unwrap_or(false)
    };

    // Auto-poll every 2 s while the job is in a non-terminal state.
    Effect::new(move |_| {
        if !is_done() {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;
                let cb: Closure<dyn FnMut()> = Closure::once(move || {
                    trigger.update(|n| *n += 1);
                });
                let _ = web_sys::window()
                    .expect("window")
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.as_ref().unchecked_ref(),
                        2_000,
                    );
                cb.forget();
            }
        }
    });

    // Fire on_done exactly once at the false→true edge.
    let fired = StoredValue::new(false);
    Effect::new(move |_| {
        if is_done() && !fired.get_value() {
            fired.set_value(true);
            if let Some(cb) = on_done {
                cb.run(());
            }
        }
    });

    view! {
        <div class="job-progress">
            <Suspense fallback=|| {
                view! { <span class="loading">"..."</span> }
            }>
                {move || {
                    job
                        .get()
                        .map(|r| match r {
                            Err(e) => {
                                view! { <span class="error">"Job error: " {e.to_string()}</span> }
                                    .into_any()
                            }
                            Ok(j) => {
                                let badge_class = match j.status.as_str() {
                                    "done" => "job-badge job-done",
                                    "failed" => "job-badge job-failed",
                                    "running" => "job-badge job-running",
                                    _ => "job-badge job-queued",
                                };
                                let is_running = j.status == "running";
                                let progress = j.progress;
                                let stage = j.stage.clone();
                                view! {
                                    <span class=badge_class>{j.status.clone()}</span>
                                    {is_running
                                        .then(|| {
                                            stage
                                                .map(|s| view! { <span class="job-stage">" - " {s}</span> })
                                        })}
                                    {is_running
                                        .then(|| {
                                            view! {
                                                <div class="job-progress-bar-wrap">
                                                    <progress
                                                        max="100"
                                                        value=progress
                                                        class="job-progress-bar"
                                                    ></progress>
                                                    <span class="job-progress-pct">{progress}"%"</span>
                                                </div>
                                            }
                                        })}
                                    {j
                                        .error
                                        .map(|e| view! { <span class="error">" - " {e}</span> })}
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

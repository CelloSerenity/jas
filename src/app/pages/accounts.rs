use leptos::prelude::*;

use crate::app::components::confirm;
use crate::app::{begin_login, complete_login, list_accounts, DeleteAccount, RevokeAllCerts};

#[component]
pub fn Accounts() -> impl IntoView {
    let accounts = Resource::new(|| (), |_| list_accounts());
    let delete_action = ServerAction::<DeleteAccount>::new();
    let revoke_action = ServerAction::<RevokeAllCerts>::new();

    Effect::new(move |_| {
        if delete_action.version().get() > 0 {
            accounts.refetch();
        }
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Apple Accounts"</h1>
            </div>

            {move || {
                revoke_action
                    .value()
                    .get()
                    .map(|r| match r {
                        Ok(n) => {
                            view! {
                                <div class="alert alert-success">
                                    "Revoked " {n}
                                    " iOS development certificate(s). You can now install apps again."
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => {
                            view! {
                                <div class="alert alert-error">
                                    "Revoke failed: " {e.to_string()}
                                </div>
                            }
                                .into_any()
                        }
                    })
            }}

            <section class="card">
                <h2>"Add Apple ID"</h2>
                <AddAccountForm on_success=move || accounts.refetch() />
            </section>

            <section class="card">
                <h2>"Accounts"</h2>
                <Suspense fallback=|| {
                    view! { <p class="loading">"Loading..."</p> }
                }>
                    {move || {
                        accounts
                            .get()
                            .map(|r| match r {
                                Err(e) => {
                                    view! { <p class="error">"Error: " {e.to_string()}</p> }
                                        .into_any()
                                }
                                Ok(accs) if accs.is_empty() => {
                                    view! { <p class="muted">"No accounts added yet."</p> }
                                        .into_any()
                                }
                                Ok(accs) => {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"Apple ID"</th>
                                                    <th>"Team"</th>
                                                    <th>"Actions"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {accs
                                                    .into_iter()
                                                    .map(|a| {
                                                        let delete_id = a.id.clone();
                                                        let revoke_id = a.id.clone();
                                                        let delete_msg = format!(
                                                            "Remove account \"{}\" from JAS? Installed apps signed with this account will keep running until their certs expire.",
                                                            a.apple_id,
                                                        );
                                                        let revoke_msg = format!(
                                                            "Revoke ALL iOS development certificates for \"{}\"? Apps signed with the revoked cert will stop working until you reinstall them.",
                                                            a.apple_id,
                                                        );
                                                        view! {
                                                            <tr>
                                                                <td>{a.apple_id.clone()}</td>
                                                                <td>
                                                                    {a.team_name.as_deref().unwrap_or("-").to_string()}
                                                                    {a
                                                                        .team_id
                                                                        .as_ref()
                                                                        .map(|t| {
                                                                            view! { <span class="muted">" (" {t.clone()} ")"</span> }
                                                                        })}
                                                                </td>
                                                                <td class="actions">
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&revoke_msg) {
                                                                            revoke_action
                                                                                .dispatch(RevokeAllCerts {
                                                                                    account_id: revoke_id.clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button type="submit" class="btn btn-sm btn-warning">
                                                                            "Revoke Certs"
                                                                        </button>
                                                                    </form>
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&delete_msg) {
                                                                            delete_action
                                                                                .dispatch(DeleteAccount {
                                                                                    id: delete_id.clone(),
                                                                                });
                                                                        }
                                                                    }>
                                                                        <button type="submit" class="btn btn-sm btn-danger">
                                                                            "Remove"
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
            </section>
        </div>
    }
}

#[component]
fn AddAccountForm(#[prop(into)] on_success: Callback<()>) -> impl IntoView {
    let apple_id = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let session_key = RwSignal::<Option<String>>::new(None);
    let tfa_code = RwSignal::new(String::new());
    let error_msg = RwSignal::<Option<String>>::new(None);
    let pending = RwSignal::new(false);

    let on_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let id = apple_id.get();
        let pw = password.get();
        pending.set(true);
        error_msg.set(None);

        leptos::task::spawn_local(async move {
            match begin_login(id, pw).await {
                Ok((key, needs_2fa)) => {
                    if needs_2fa {
                        session_key.set(Some(key));
                    } else {
                        on_success.run(());
                    }
                }
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                }
            }
            pending.set(false);
        });
    };

    let on_2fa_submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let key = match session_key.get() {
            Some(k) => k,
            None => return,
        };
        let code = tfa_code.get();
        pending.set(true);
        error_msg.set(None);

        leptos::task::spawn_local(async move {
            match complete_login(key, code).await {
                Ok(()) => {
                    session_key.set(None);
                    on_success.run(());
                }
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                }
            }
            pending.set(false);
        });
    };

    view! {
        {move || {
            if session_key.get().is_none() {
                view! {
                    <form on:submit=on_submit>
                        <div class="form-row">
                            <label class="form-field">
                                "Apple ID"
                                <input
                                    type="email"
                                    required
                                    placeholder="you@example.com"
                                    prop:value=apple_id
                                    on:input=move |e| apple_id.set(event_target_value(&e))
                                />
                            </label>
                            <label class="form-field">
                                "Password"
                                <input
                                    type="password"
                                    required
                                    placeholder="•••••••••"
                                    prop:value=password
                                    on:input=move |e| password.set(event_target_value(&e))
                                />
                            </label>
                        </div>
                        <p class="hint">"Your password is used once and never stored."</p>
                        <button type="submit" class="btn btn-primary" prop:disabled=pending>
                            {move || if pending.get() { "Signing in..." } else { "Sign In" }}
                        </button>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <form on:submit=on_2fa_submit>
                        <p>"A two-factor code was sent to your trusted device."</p>
                        <div class="form-row">
                            <label class="form-field">
                                "2FA Code"
                                <input
                                    type="text"
                                    required
                                    placeholder="000000"
                                    maxlength="6"
                                    prop:value=tfa_code
                                    on:input=move |e| tfa_code.set(event_target_value(&e))
                                />
                            </label>
                        </div>
                        <button type="submit" class="btn btn-primary" prop:disabled=pending>
                            {move || if pending.get() { "Verifying..." } else { "Submit Code" }}
                        </button>
                    </form>
                }
                    .into_any()
            }
        }}

        {move || error_msg.get().map(|e| view! { <p class="error">{e}</p> })}
    }
}

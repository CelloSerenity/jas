use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::components::confirm;
use crate::app::{list_devices, DeleteDevice, RegisterDevice};

#[component]
pub fn Devices() -> impl IntoView {
    let devices = Resource::new(|| (), |_| list_devices());
    let delete_action = ServerAction::<DeleteDevice>::new();
    let register_action = ServerAction::<RegisterDevice>::new();

    Effect::new(move |_| {
        if delete_action.version().get() > 0 || register_action.version().get() > 0 {
            devices.refetch();
        }
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h1>"Devices"</h1>
            </div>

            <section class="card">
                <h2>"Register Device by IP"</h2>
                <RegisterForm action=register_action />

                {move || {
                    if register_action.pending().get() {
                        Some(
                            view! {
                                <div class="alert alert-info">
                                    "Connecting to device. This can take a few seconds..."
                                </div>
                            }
                                .into_any(),
                        )
                    } else {
                        register_action
                            .value()
                            .get()
                            .map(|r| match r {
                                Ok(id) => {
                                    view! {
                                        <div class="alert alert-success">
                                            "Device registered (ID: " {id} ")"
                                        </div>
                                    }
                                        .into_any()
                                }
                                Err(e) => {
                                    view! {
                                        <div class="alert alert-error">
                                            "Error: " {e.to_string()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            })
                    }
                }}
            </section>

            <section class="card">
                <h2>"Registered Devices"</h2>
                <Suspense fallback=|| {
                    view! { <p class="loading">"Loading..."</p> }
                }>
                    {move || {
                        devices
                            .get()
                            .map(|result| match result {
                                Err(e) => {
                                    view! { <p class="error">"Error: " {e.to_string()}</p> }
                                        .into_any()
                                }
                                Ok(devs) if devs.is_empty() => {
                                    view! { <p class="muted">"No devices registered."</p> }
                                        .into_any()
                                }
                                Ok(devs) => {
                                    view! {
                                        <table class="table">
                                            <thead>
                                                <tr>
                                                    <th>"Name"</th>
                                                    <th>"IP"</th>
                                                    <th>"UDID"</th>
                                                    <th>"Actions"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {devs
                                                    .into_iter()
                                                    .map(|d| {
                                                        let id = d.id.clone();
                                                        let confirm_msg = format!(
                                                            "Delete device \"{}\"? This removes it from JAS but does not touch the device itself.",
                                                            d.name,
                                                        );
                                                        let mdns_ip = d.mdns_ip.clone();
                                                        view! {
                                                            <tr>
                                                                <td>{d.name.clone()}</td>
                                                                <td>
                                                                    <div>{d.ip.clone()}</div>
                                                                    {mdns_ip
                                                                        .map(|ip| {
                                                                            view! { <div class="muted small">"mDNS: "{ip}</div> }
                                                                        })}
                                                                </td>
                                                                <td class="mono">{d.udid}""</td>
                                                                <td class="actions">
                                                                    <A
                                                                        href=format!("/devices/{}", d.id)
                                                                        attr:class="btn btn-sm btn-secondary"
                                                                    >
                                                                        "Manage"
                                                                    </A>
                                                                    <form on:submit=move |e: leptos::ev::SubmitEvent| {
                                                                        e.prevent_default();
                                                                        if confirm(&confirm_msg) {
                                                                            delete_action.dispatch(DeleteDevice { id: id.clone() });
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
            </section>
        </div>
    }
}

#[component]
fn RegisterForm(action: ServerAction<RegisterDevice>) -> impl IntoView {
    let pairing_b64 = RwSignal::new(String::new());
    let file_status = RwSignal::new(String::new());

    view! {
        <ActionForm action=action>
            <div class="form-row">
                <label class="form-field">
                    "IP Address" <input type="text" name="ip" required placeholder="10.0.0.2" />
                </label>
                <label class="form-field">
                    "RPPairing File (.plist)"
                    <input
                        type="file"
                        accept=".plist"
                        on:change=move |e| {
                            #[cfg(target_arch = "wasm32")]
                            read_file_as_b64(e, pairing_b64, file_status);
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let _ = e;
                            }
                        }
                    /> <small>{file_status}</small>
                </label>
            </div>
            <input type="hidden" name="pairing_blob_b64" prop:value=pairing_b64 />
            <button
                type="submit"
                class="btn btn-primary"
                prop:disabled=move || pairing_b64.get().is_empty() || action.pending().get()
            >
                {move || if action.pending().get() { "Registering…" } else { "Register" }}
            </button>
        </ActionForm>
    }
}

#[cfg(target_arch = "wasm32")]
fn read_file_as_b64(e: leptos::ev::Event, out: RwSignal<String>, status: RwSignal<String>) {
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
        if let Ok(result) = r2.result() {
            if let Some(data_url) = result.as_string() {
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

use dioxus::prelude::*;
use shared_types::{PoolStaffResponse, StartTaskResponse, ReplaceStaffParams};
use crate::components::{Modal, toast::{show_toast, use_toast, ToastLevel}};
use super::NavBar;

#[server(session: tower_sessions::Session)]
async fn fetch_pool_staff() -> Result<PoolStaffResponse, ServerFnError> {
    use std::sync::Arc;
    use dioxus::fullstack::FullstackContext;
    use server::AppState;
    use server::auth::routes::USER_SESSION_KEY;
    use shared_types::UserSession;

    let _user: UserSession = session
        .get(USER_SESSION_KEY)
        .await
        .map_err(|e| ServerFnError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let state: Arc<AppState> = FullstackContext::current()
        .and_then(|ctx| ctx.extension::<Arc<AppState>>())
        .ok_or_else(|| ServerFnError::new("AppState not available"))?;

    let result = server::api::pool_staff_handler(axum::extract::State(state))
        .await
        .map_err(|(_status, json)| ServerFnError::new(json.0.error))?;

    Ok(result.0)
}

#[server(session: tower_sessions::Session)]
async fn start_replace_staff(old_name: String, new_name: String, ct_type: String) -> Result<StartTaskResponse, ServerFnError> {
    use std::sync::Arc;
    use dioxus::fullstack::FullstackContext;
    use server::AppState;
    use server::auth::routes::USER_SESSION_KEY;
    use shared_types::UserSession;

    let user: UserSession = session
        .get(USER_SESSION_KEY)
        .await
        .map_err(|e| ServerFnError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let state: Arc<AppState> = FullstackContext::current()
        .and_then(|ctx| ctx.extension::<Arc<AppState>>())
        .ok_or_else(|| ServerFnError::new("AppState not available"))?;

    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| ServerFnError::new("Database not available"))?;

    let params = ReplaceStaffParams { old_name: old_name.clone(), new_name: new_name.clone(), ct_type: ct_type.clone() };
    let description = format!("Replace {} '{}' with '{}'", ct_type, old_name, new_name);

    let task_id = server::tasks::create_task(
        pool,
        &user.sub,
        user.email.as_deref(),
        &description,
        "replace_staff",
        &serde_json::to_value(&params).unwrap_or_default(),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Failed to create task: {e}")))?;

    server::task_runner::spawn_replace_staff_task(
        state.clone(),
        pool.clone(),
        task_id,
        params,
        user.sub,
        user.email,
    );

    Ok(StartTaskResponse {
        task_id: task_id.to_string(),
        message: "Task started".to_string(),
    })
}

#[component]
pub fn PoolStaff() -> Element {
    let staff = use_server_future(fetch_pool_staff)?;
    let mut search = use_signal(|| String::new());
    let mut show_confirm = use_signal(|| false);
    let mut show_success = use_signal(|| false);
    let mut pending_replace: Signal<Option<(String, String, String)>> = use_signal(|| None);
    let mut last_task_id = use_signal(|| String::new());
    let mut toasts = use_toast();

    let mut selections: Signal<std::collections::HashMap<String, String>> = use_signal(|| std::collections::HashMap::new());

    rsx! {
        div { class: "container",
            NavBar {}
            h1 { "Pool Staff" }
            p { "View staff assignments and replace personnel across all future pool sessions." }

            div { style: "margin-bottom: 1rem;",
                input {
                    r#type: "text",
                    placeholder: "Search by name or type...",
                    value: "{search}",
                    oninput: move |e| search.set(e.value()),
                    style: "padding: 0.5rem; width: 300px;",
                }
            }

            match &*staff.read() {
                Some(Ok(data)) => {
                    let filter = search.read().to_lowercase();
                    let filtered: Vec<_> = data.rows.iter().filter(|r| {
                        filter.is_empty()
                            || r.ct_name.to_lowercase().contains(&filter)
                            || r.ct_type.to_lowercase().contains(&filter)
                    }).collect();

                    rsx! {
                        p { "Showing {filtered.len()} of {data.count} entries" }
                        table {
                            thead {
                                tr {
                                    th { "Name" }
                                    th { "Type" }
                                    th { "Sessions" }
                                    th { "First Date" }
                                    th { "Last Date" }
                                    th { "In Codes?" }
                                    th { "Replace With" }
                                    th { "Action" }
                                }
                            }
                            tbody {
                                for row in filtered.iter() {
                                    {
                                        let key = format!("{}|{}", row.ct_name, row.ct_type);
                                        let row_name = row.ct_name.clone();
                                        let row_type = row.ct_type.clone();
                                        let options = row.codes_options.clone();
                                        let key_for_select = key.clone();
                                        let key_for_btn = key.clone();

                                        rsx! {
                                            tr { key: "{key}",
                                                td { "{row.ct_name}" }
                                                td { "{row.ct_type}" }
                                                td { "{row.schedule_count}" }
                                                td { {row.first_date.as_deref().unwrap_or("-")} }
                                                td { {row.last_date.as_deref().unwrap_or("-")} }
                                                td {
                                                    if row.has_codes_entry { "Yes" } else { "No" }
                                                }
                                                td {
                                                    if options.is_empty() {
                                                        "No options"
                                                    } else {
                                                        select {
                                                            onchange: move |e| {
                                                                selections.write().insert(key_for_select.clone(), e.value());
                                                            },
                                                            option { value: "", "-- Select --" }
                                                            for opt in options.iter() {
                                                                option {
                                                                    value: "{opt.co_translation}",
                                                                    "{opt.co_translation} ({opt.co_code})"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td {
                                                    button {
                                                        onclick: {
                                                            let key_for_btn = key_for_btn.clone();
                                                            let row_name = row_name.clone();
                                                            let row_type = row_type.clone();
                                                            move |_| {
                                                                let sel = selections.read();
                                                                let selected = sel.get(&key_for_btn).cloned().unwrap_or_default();
                                                                if selected.is_empty() {
                                                                    show_toast(&mut toasts, "Please select a replacement first", ToastLevel::Error);
                                                                    return;
                                                                }
                                                                pending_replace.set(Some((row_name.clone(), selected, row_type.clone())));
                                                                show_confirm.set(true);
                                                            }
                                                        },
                                                        "Replace All"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => rsx! {
                    p { class: "error", "Error loading staff: {e}" }
                },
                None => rsx! {
                    p { "Loading staff..." }
                },
            }

            Modal { show: show_confirm, title: "Confirm Replacement".to_string(),
                if let Some((ref old, ref new, ref typ)) = *pending_replace.read() {
                    div {
                        p { "Replace all future session assignments for:" }
                        p { strong { "{old}" } " ({typ})" }
                        p { "with:" }
                        p { strong { "{new}" } }
                        div { style: "display: flex; gap: 1rem; margin-top: 1rem;",
                            button {
                                onclick: {
                                    let old = old.clone();
                                    let new = new.clone();
                                    let typ = typ.clone();
                                    move |_| {
                                        show_confirm.set(false);
                                        let old = old.clone();
                                        let new = new.clone();
                                        let typ = typ.clone();
                                        spawn(async move {
                                            match start_replace_staff(old, new, typ).await {
                                                Ok(resp) => {
                                                    last_task_id.set(resp.task_id);
                                                    show_success.set(true);
                                                }
                                                Err(e) => {
                                                    show_toast(&mut toasts, format!("Failed: {e}"), ToastLevel::Error);
                                                }
                                            }
                                        });
                                    }
                                },
                                style: "padding: 0.5rem 1rem; background: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer;",
                                "Confirm"
                            }
                            button {
                                onclick: move |_| {
                                    show_confirm.set(false);
                                    show_toast(&mut toasts, "Action cancelled", ToastLevel::Info);
                                },
                                style: "padding: 0.5rem 1rem; background: #6b7280; color: white; border: none; border-radius: 4px; cursor: pointer;",
                                "Cancel"
                            }
                        }
                    }
                }
            }

            Modal { show: show_success, title: "Task Started".to_string(),
                div {
                    p { "Your staff replacement task has been started." }
                    p {
                        "Track its progress on the "
                        a { href: "/tasks", "Tasks page" }
                        "."
                    }
                    p { "Task ID: " code { "{last_task_id}" } }
                }
            }
        }
    }
}

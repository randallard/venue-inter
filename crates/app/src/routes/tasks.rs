use dioxus::prelude::*;
use shared_types::{TasksResponse, TaskRow};
use super::NavBar;

#[server(session: tower_sessions::Session)]
async fn fetch_tasks() -> Result<TasksResponse, ServerFnError> {
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

    let tasks = server::tasks::get_tasks_for_user(pool, &user.sub)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed: {e}")))?;

    let count = tasks.len();
    Ok(TasksResponse { tasks, count })
}

#[server(session: tower_sessions::Session)]
async fn fetch_task_detail(task_id: String) -> Result<TaskRow, ServerFnError> {
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

    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| ServerFnError::new("Database not available"))?;

    let uuid = uuid::Uuid::parse_str(&task_id)
        .map_err(|_| ServerFnError::new("Invalid task ID"))?;

    let task = server::tasks::get_task_by_id(pool, uuid)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed: {e}")))?
        .ok_or_else(|| ServerFnError::new("Task not found"))?;

    Ok(task)
}

fn status_style(status: &str) -> &'static str {
    match status {
        "completed" => "color: #22c55e; font-weight: bold;",
        "failed" => "color: #ef4444; font-weight: bold;",
        "running" => "color: #3b82f6; font-weight: bold;",
        _ => "color: #6b7280;",
    }
}

#[component]
pub fn Tasks() -> Element {
    let tasks = use_server_future(fetch_tasks)?;

    rsx! {
        div { class: "container",
            NavBar {}
            h1 { "Your Tasks" }
            match &*tasks.read() {
                Some(Ok(data)) => {
                    if data.tasks.is_empty() {
                        rsx! { p { "No tasks yet." } }
                    } else {
                        rsx! {
                            p { "Showing {data.count} task(s)" }
                            table {
                                thead {
                                    tr {
                                        th { "Description" }
                                        th { "Status" }
                                        th { "Created" }
                                        th { "Details" }
                                    }
                                }
                                tbody {
                                    for task in data.tasks.iter() {
                                        tr { key: "{task.id}",
                                            td { "{task.description}" }
                                            td { style: status_style(&task.status), "{task.status}" }
                                            td { {&task.created_at[..19]} }
                                            td {
                                                a { href: "/tasks/{task.id}", "View" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => rsx! {
                    p { class: "error", "Error: {e}" }
                },
                None => rsx! { p { "Loading..." } },
            }
        }
    }
}

#[component]
pub fn TaskDetail(id: String) -> Element {
    let task_id = id.clone();
    let task = use_server_future(move || {
        let tid = task_id.clone();
        async move { fetch_task_detail(tid).await }
    })?;

    rsx! {
        div { class: "container",
            NavBar {}
            a { href: "/tasks", "Back to Tasks" }
            match &*task.read() {
                Some(Ok(t)) => rsx! {
                    h1 { "Task Detail" }
                    table {
                        tbody {
                            tr { td { strong { "ID" } } td { "{t.id}" } }
                            tr { td { strong { "Description" } } td { "{t.description}" } }
                            tr { td { strong { "Type" } } td { "{t.task_type}" } }
                            tr {
                                td { strong { "Status" } }
                                td { style: status_style(&t.status), "{t.status}" }
                            }
                            if let Some(ref summary) = t.result_summary {
                                tr { td { strong { "Result" } } td { "{summary}" } }
                            }
                            if let Some(ref err) = t.error_detail {
                                tr {
                                    td { strong { "Error" } }
                                    td { style: "color: #ef4444;", "{err}" }
                                }
                            }
                            tr { td { strong { "Created" } } td { "{t.created_at}" } }
                            tr { td { strong { "Updated" } } td { "{t.updated_at}" } }
                        }
                    }
                },
                Some(Err(e)) => rsx! {
                    h1 { "Task Detail" }
                    p { class: "error", "Error: {e}" }
                },
                None => rsx! { p { "Loading..." } },
            }
        }
    }
}

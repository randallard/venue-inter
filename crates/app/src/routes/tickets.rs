use dioxus::prelude::*;
use shared_types::TicketsResponse;
use super::NavBar;

#[server(session: tower_sessions::Session)]
async fn fetch_tickets() -> Result<(TicketsResponse, bool), ServerFnError> {
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

    let is_helpdesk = user.groups.iter().any(|g| g == "helpdesk");

    let tickets = if is_helpdesk {
        server::tickets::get_all_tickets(pool).await
    } else {
        server::tickets::get_tickets_for_user(pool, &user.sub).await
    }
    .map_err(|e| ServerFnError::new(format!("Failed: {e}")))?;

    let count = tickets.len();
    Ok((TicketsResponse { tickets, count }, is_helpdesk))
}

fn ticket_status_style(status: &str) -> &'static str {
    match status {
        "resolved" => "color: #22c55e; font-weight: bold;",
        "pending_assignment" => "color: #f59e0b; font-weight: bold;",
        "in_progress" => "color: #3b82f6; font-weight: bold;",
        _ => "color: #6b7280;",
    }
}

#[component]
pub fn Tickets() -> Element {
    let tickets = use_server_future(fetch_tickets)?;
    let mut search = use_signal(|| String::new());

    rsx! {
        div { class: "container",
            NavBar {}
            match &*tickets.read() {
                Some(Ok((data, is_helpdesk))) => {
                    let title = if *is_helpdesk { "All Tickets (Helpdesk)" } else { "Your Tickets" };
                    let filter = search.read().to_lowercase();
                    let filtered: Vec<_> = data.tickets.iter().filter(|t| {
                        filter.is_empty()
                            || t.description.to_lowercase().contains(&filter)
                            || t.status.to_lowercase().contains(&filter)
                            || t.user_email.as_deref().unwrap_or("").to_lowercase().contains(&filter)
                    }).collect();

                    rsx! {
                        h1 { "{title}" }

                        if *is_helpdesk {
                            div { style: "margin-bottom: 1rem;",
                                input {
                                    r#type: "text",
                                    placeholder: "Filter tickets...",
                                    value: "{search}",
                                    oninput: move |e| search.set(e.value()),
                                    style: "padding: 0.5rem; width: 300px;",
                                }
                            }
                        }

                        if filtered.is_empty() {
                            p { "No tickets." }
                        } else {
                            p { "Showing {filtered.len()} ticket(s)" }
                            table {
                                thead {
                                    tr {
                                        th { "Description" }
                                        th { "Status" }
                                        if *is_helpdesk {
                                            th { "User" }
                                        }
                                        th { "Created" }
                                        th { "Task" }
                                    }
                                }
                                tbody {
                                    for ticket in filtered.iter() {
                                        tr { key: "{ticket.id}",
                                            td { "{ticket.description}" }
                                            td {
                                                style: ticket_status_style(&ticket.status),
                                                "{ticket.status}"
                                            }
                                            if *is_helpdesk {
                                                td { {ticket.user_email.as_deref().unwrap_or("-")} }
                                            }
                                            td { {&ticket.created_at[..19]} }
                                            td {
                                                if let Some(ref tid) = ticket.task_id {
                                                    a { href: "/tasks/{tid}", "View Task" }
                                                } else {
                                                    "-"
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
                    h1 { "Tickets" }
                    p { class: "error", "Error: {e}" }
                },
                None => rsx! { p { "Loading..." } },
            }
        }
    }
}

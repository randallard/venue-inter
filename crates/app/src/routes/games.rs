use dioxus::prelude::*;
use shared_types::{ParticipantsResponse, PoolMembersResponse};
use super::NavBar;

#[server(session: tower_sessions::Session)]
async fn fetch_participants() -> Result<ParticipantsResponse, ServerFnError> {
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

    let result = server::api::participants_handler(axum::extract::State(state))
        .await
        .map_err(|(_status, json)| ServerFnError::new(json.0.error))?;

    Ok(result.0)
}

#[server(session: tower_sessions::Session)]
async fn fetch_pool_members(pool_no: i32) -> Result<PoolMembersResponse, ServerFnError> {
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

    let result = server::api::pool_members_handler(
        axum::extract::State(state),
        axum::extract::Path(pool_no),
    )
    .await
    .map_err(|(_status, json)| ServerFnError::new(json.0.error))?;

    Ok(result.0)
}

#[component]
pub fn Participants() -> Element {
    let participants = use_server_future(fetch_participants)?;

    rsx! {
        div { class: "container",
            NavBar {}
            h1 { "Participants" }
            match &*participants.read() {
                Some(Ok(data)) => rsx! {
                    p { "Showing {data.count} participants" }
                    table {
                        thead {
                            tr {
                                th { "ID" }
                                th { "Last Name" }
                                th { "First Name" }
                                th { "City" }
                                th { "State" }
                                th { "Gender" }
                                th { "Race" }
                                th { "Active" }
                                th { "Added" }
                            }
                        }
                        tbody {
                            for p in data.participants.iter() {
                                tr { key: "{p.part_no}",
                                    td { "{p.part_no}" }
                                    td { {p.lname.as_deref().unwrap_or("-")} }
                                    td { {p.fname.as_deref().unwrap_or("-")} }
                                    td { {p.city.as_deref().unwrap_or("-")} }
                                    td { {p.state.as_deref().unwrap_or("-")} }
                                    td { {p.gender.as_deref().unwrap_or("-")} }
                                    td { {p.race_code.as_deref().unwrap_or("-")} }
                                    td {
                                        match p.active.as_deref() {
                                            Some("A") => "Active",
                                            Some("I") => "Inactive",
                                            _ => "-",
                                        }
                                    }
                                    td { {p.date_added.as_deref().unwrap_or("-")} }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! {
                    p { class: "error", "Error loading participants: {e}" }
                },
                None => rsx! {
                    p { "Loading participants..." }
                },
            }
        }
    }
}

#[component]
pub fn PoolDetail(id: i32) -> Element {
    let members = use_server_future(move || fetch_pool_members(id))?;

    rsx! {
        div { class: "container",
            NavBar {}
            a { href: "/pools", "Back to Pools" }
            match &*members.read() {
                Some(Ok(data)) => rsx! {
                    h1 { "Pool #{id} — Members ({data.count})" }
                    table {
                        thead {
                            tr {
                                th { "Part #" }
                                th { "Last Name" }
                                th { "First Name" }
                                th { "Status" }
                                th { "Rand #" }
                                th { "Responded" }
                            }
                        }
                        tbody {
                            for m in data.members.iter() {
                                tr { key: "{m.pm_id}",
                                    td { "{m.part_no}" }
                                    td { {m.lname.as_deref().unwrap_or("-")} }
                                    td { {m.fname.as_deref().unwrap_or("-")} }
                                    td {
                                        match m.status {
                                            1 => "In Pool",
                                            2 => "Qualified",
                                            5 => "Perm. Excused",
                                            6 => "Disqualified",
                                            7 => "Temp. Excused",
                                            _ => "Unknown",
                                        }
                                    }
                                    td { {m.rand_nbr.map(|n| n.to_string()).as_deref().unwrap_or("-").to_string()} }
                                    td { {m.responded.as_deref().unwrap_or("N")} }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! {
                    h1 { "Pool #{id}" }
                    p { class: "error", "Error loading pool members: {e}" }
                },
                None => rsx! {
                    h1 { "Pool #{id}" }
                    p { "Loading members..." }
                },
            }
        }
    }
}

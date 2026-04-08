pub mod games;
pub mod person_schedules;
pub mod tasks;
pub mod tickets;

use games::{Participants, PoolDetail};
use person_schedules::PoolStaff;
use tasks::{Tasks, TaskDetail};
use tickets::Tickets;
use dioxus::prelude::*;
use shared_types::UserSession;

#[server(endpoint = "/api/current_user", session: tower_sessions::Session)]
async fn get_current_user() -> Result<Option<UserSession>, ServerFnError> {
    use server::auth::routes::USER_SESSION_KEY;

    let user = session
        .get::<UserSession>(USER_SESSION_KEY)
        .await
        .map_err(|e| ServerFnError::new(format!("Session read error: {e}")))?;

    Ok(user)
}

#[component]
pub fn NavBar() -> Element {
    let user = use_server_future(get_current_user)?;

    let auth_section = match &*user.read() {
        Some(Ok(Some(session))) => {
            let display = session
                .name
                .as_deref()
                .or(session.email.as_deref())
                .unwrap_or(&session.sub);
            rsx! {
                span { "{display}" }
                {" | "}
                a { href: "/auth/logout", "Logout" }
            }
        }
        _ => rsx! {
            a { href: "/auth/login", "Login" }
        },
    };

    rsx! {
        nav {
            a { href: "/", "Home" }
            {" | "}
            a { href: "/participants", "Participants" }
            {" | "}
            a { href: "/pools", "Pools" }
            {" | "}
            a { href: "/pool-staff", "Pool Staff" }
            {" | "}
            a { href: "/tasks", "Tasks" }
            {" | "}
            a { href: "/tickets", "Tickets" }
            {" | "}
            {auth_section}
        }
    }
}

#[component]
fn AuthGuard() -> Element {
    let user = use_server_future(get_current_user)?;
    let guard = user.read();

    let result = match &*guard {
        Some(Ok(Some(_))) => rsx! { Outlet::<Route> {} },
        Some(Ok(None)) | Some(Err(_)) => {
            rsx! {
                div { class: "container",
                    NavBar {}
                    p { "Redirecting to login..." }
                    script { "window.location.href = '/auth/login';" }
                }
            }
        }
        None => rsx! {
            div { class: "container",
                p { "Loading..." }
            }
        },
    };

    result
}

#[derive(Clone, Routable, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},

    #[layout(AuthGuard)]
        #[route("/participants")]
        Participants {},

        #[route("/pools/:id")]
        PoolDetail { id: i32 },

        #[route("/pool-staff")]
        PoolStaff {},

        #[route("/tasks")]
        Tasks {},

        #[route("/tasks/:id")]
        TaskDetail { id: String },

        #[route("/tickets")]
        Tickets {},
    #[end_layout]

    #[route("/:..route")]
    NotFound { route: Vec<String> },
}

#[component]
fn Home() -> Element {
    rsx! {
        div { class: "container",
            NavBar {}
            h1 { "VenueInter" }
            p { "Venue Audience Management — Legacy Informix Interface" }
            p { class: "subtitle",
                "Browse and manage venue audience participants, show pools, "
                "staff assignments, and background task progress."
            }
            ul {
                li { a { href: "/participants", "Participants" } " — Browse the participant database" }
                li { a { href: "/pool-staff", "Pool Staff" } " — View and replace staff session assignments" }
                li { a { href: "/tasks", "Tasks" } " — Track background task progress" }
                li { a { href: "/tickets", "Tickets" } " — View support tickets" }
            }
        }
    }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    rsx! {
        div { class: "container",
            h1 { "404 — Not Found" }
            p { "The page {route.join(\"/\")} was not found." }
            a { href: "/", "Go home" }
        }
    }
}

use dioxus::prelude::*;

#[component]
pub fn Modal(show: Signal<bool>, title: String, children: Element) -> Element {
    if !show() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| show.set(false),
            style: "position:fixed;top:0;left:0;width:100%;height:100%;background:rgba(0,0,0,0.5);display:flex;align-items:center;justify-content:center;z-index:1000;",
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                style: "background:#fff;padding:1.5rem;border-radius:8px;max-width:500px;width:90%;max-height:80vh;overflow-y:auto;",
                div {
                    class: "modal-header",
                    style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;",
                    h2 { style: "margin:0;", "{title}" }
                    button {
                        onclick: move |_| show.set(false),
                        style: "background:none;border:none;font-size:1.5rem;cursor:pointer;padding:0;",
                        "x"
                    }
                }
                div { class: "modal-body", {children} }
            }
        }
    }
}

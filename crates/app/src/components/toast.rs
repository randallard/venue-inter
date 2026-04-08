use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub id: usize,
}

static TOAST_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn use_toast() -> Signal<Vec<Toast>> {
    use_context::<Signal<Vec<Toast>>>()
}

pub fn show_toast(toasts: &mut Signal<Vec<Toast>>, message: impl Into<String>, level: ToastLevel) {
    let id = TOAST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    toasts.write().push(Toast {
        message: message.into(),
        level,
        id,
    });

    // Auto-remove after 4 seconds
    let mut toasts_clone = *toasts;
    spawn(async move {
        #[cfg(feature = "web")]
        gloo_timers::future::TimeoutFuture::new(4000).await;

        #[cfg(feature = "server")]
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        toasts_clone.write().retain(|t| t.id != id);
    });
}

#[component]
pub fn ToastManager() -> Element {
    let toasts = use_toast();

    if toasts.read().is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            style: "position:fixed;top:1rem;right:1rem;z-index:2000;display:flex;flex-direction:column;gap:0.5rem;",
            for toast in toasts.read().iter() {
                div {
                    key: "{toast.id}",
                    style: format!(
                        "padding:0.75rem 1rem;border-radius:6px;color:#fff;min-width:250px;max-width:400px;box-shadow:0 2px 8px rgba(0,0,0,0.2);{}",
                        match toast.level {
                            ToastLevel::Info => "background:#3b82f6;",
                            ToastLevel::Success => "background:#22c55e;",
                            ToastLevel::Error => "background:#ef4444;",
                        }
                    ),
                    "{toast.message}"
                }
            }
        }
    }
}

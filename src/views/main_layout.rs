use dioxus::prelude::*;
use crate::components::sidebar::Sidebar;
use crate::components::chat_view::ChatView;
use crate::components::invite_modal::InviteModal;
use crate::components::settings_modal::SettingsModal;
use crate::state::ACTIVE_CHAT_ID;

#[component]
pub fn MainLayout() -> Element {
    let has_active_chat = ACTIVE_CHAT_ID.read().is_some();
    let container_class = if has_active_chat {
        "app-container chat-active"
    } else {
        "app-container"
    };

    rsx! {
        div { class: "{container_class}",
            Sidebar {}
            ChatView {}
        }
        InviteModal {}
        SettingsModal {}
    }
}

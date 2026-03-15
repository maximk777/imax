use dioxus::prelude::*;
use crate::components::sidebar::Sidebar;
use crate::components::chat_view::ChatView;
use crate::components::invite_modal::InviteModal;
use crate::components::settings_modal::SettingsModal;

#[component]
pub fn MainLayout() -> Element {
    rsx! {
        div { class: "app-container",
            Sidebar {}
            ChatView {}
        }
        InviteModal {}
        SettingsModal {}
    }
}

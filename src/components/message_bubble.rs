use dioxus::prelude::*;
use crate::state::Message;

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let row_class = if message.is_mine {
        "message-row outgoing"
    } else {
        "message-row incoming"
    };

    let bubble_class = if message.is_mine {
        "message-bubble outgoing"
    } else {
        "message-bubble incoming"
    };

    let status_icon = match message.status.as_str() {
        "read"      => "✓✓",
        "delivered" => "✓✓",
        "sent"      => "✓",
        _           => "",
    };

    let status_color = if message.status == "read" { "color: #6ab3f3" } else { "" };

    rsx! {
        div { class: "{row_class}",
            div { class: "{bubble_class}",
                div { class: "message-content", "{message.content}" }
                div { class: "message-meta",
                    span { class: "message-time", "{message.time}" }
                    if message.is_mine {
                        span {
                            class: "message-status",
                            style: "{status_color}",
                            "{status_icon}"
                        }
                    }
                }
            }
        }
    }
}

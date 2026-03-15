use dioxus::prelude::*;
use dioxus::document::eval;
use crate::state::{SHOW_SETTINGS_MODAL, NICKNAME, SEED_PHRASE};

#[component]
pub fn SettingsModal() -> Element {
    let show = *SHOW_SETTINGS_MODAL.read();
    let nickname = NICKNAME.read().clone();
    let seed = SEED_PHRASE.read().clone();
    let mut show_seed = use_signal(|| false);
    let mut seed_copied = use_signal(|| false);

    if !show {
        return rsx! {};
    }

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| {
                *SHOW_SETTINGS_MODAL.write() = false;
            },
            div {
                class: "modal modal-settings",
                onclick: move |evt| evt.stop_propagation(),

                // Header with close button
                div { class: "modal-header",
                    div { class: "modal-title", "Settings" }
                    button {
                        class: "modal-close",
                        onclick: move |_| {
                            *SHOW_SETTINGS_MODAL.write() = false;
                        },
                        "\u{2715}"
                    }
                }

                // Profile
                div { class: "settings-profile",
                    div { class: "settings-avatar", "{nickname.chars().next().unwrap_or('?').to_uppercase()}" }
                    div { class: "settings-profile-info",
                        div { class: "settings-profile-name", "{nickname}" }
                        div { class: "settings-profile-status", "online" }
                    }
                }

                // Seed phrase
                div { class: "settings-section",
                    p { class: "settings-section-title", "Seed Phrase" }

                    if *show_seed.read() {
                        // Show seed phrase
                        div { class: "seed-phrase-box",
                            div { class: "seed-grid",
                                {
                                    let words: Vec<String> = seed.split_whitespace()
                                        .enumerate()
                                        .map(|(i, w)| format!("{}. {}", i + 1, w))
                                        .collect();
                                    rsx! {
                                        for word in words {
                                            div { class: "seed-word", "{word}" }
                                        }
                                    }
                                }
                            }
                        }
                        button {
                            class: "modal-btn-secondary",
                            onclick: move |_| {
                                let s = SEED_PHRASE.read().clone();
                                eval(&format!(r#"navigator.clipboard.writeText("{s}")"#));
                                *seed_copied.write() = true;
                            },
                            if *seed_copied.read() { "Copied!" } else { "Copy seed phrase" }
                        }
                        button {
                            class: "settings-link",
                            onclick: move |_| {
                                *show_seed.write() = false;
                                *seed_copied.write() = false;
                            },
                            "Hide"
                        }
                    } else {
                        // Hidden state
                        div { class: "seed-hidden",
                            p { class: "seed-hidden-text", "Your seed phrase is hidden for security" }
                            button {
                                class: "modal-btn-secondary",
                                onclick: move |_| {
                                    *show_seed.write() = true;
                                },
                                "Show Seed Phrase"
                            }
                        }
                    }

                    p { class: "settings-hint", "Save your seed phrase to restore your account on another device. Never share it." }
                }

                // Encryption info
                div { class: "settings-section",
                    p { class: "settings-section-title", "Encryption" }
                    div { class: "settings-encryption-info",
                        span { class: "settings-encryption-icon", "\u{1F6E1}" }
                        div {
                            p { class: "settings-encryption-title", "End-to-end encrypted" }
                            p { class: "settings-encryption-desc", "XChaCha20-Poly1305 + X25519 DH" }
                        }
                    }
                }
            }
        }
    }
}

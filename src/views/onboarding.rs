use dioxus::prelude::*;
use imax_core::storage::models;
use crate::state::{start_node, db, ACTIVE_PROFILE_ID, IS_ONBOARDED, ADDING_PROFILE};

#[component]
pub fn Onboarding() -> Element {
    let mut nick_input = use_signal(String::new);
    let mut error_msg = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut show_restore = use_signal(|| false);
    let mut seed_input = use_signal(String::new);
    let is_adding = *ADDING_PROFILE.read();

    let on_start = move |_| {
        let name = nick_input.read().trim().to_string();
        if name.is_empty() {
            *error_msg.write() = "Please enter a nickname.".into();
            return;
        }

        *loading.write() = true;
        *error_msg.write() = String::new();

        match imax_core::identity::generate_mnemonic() {
            Ok(mnemonic) => {
                let signing_key = imax_core::identity::derive_signing_key(&mnemonic);
                let pubkey_bytes = signing_key.verifying_key().to_bytes();
                let sk_bytes = signing_key.to_bytes();
                let seed_phrase = mnemonic.to_string();

                // Create profile in DB
                let profile_id = {
                    let db = db();
                    let pid = models::create_profile(&db, &seed_phrase, &name).unwrap();
                    models::set_active_profile(&db, pid).unwrap();
                    let total = models::get_all_profiles(&db).unwrap_or_default().len();
                    println!("[imax] Created profile id={pid} nick={name}, total profiles: {total}");
                    pid
                };
                *ACTIVE_PROFILE_ID.write() = profile_id;
                if is_adding {
                    crate::state::shutdown_node();
                }
                *ADDING_PROFILE.write() = false;

                start_node(sk_bytes, pubkey_bytes, seed_phrase, name);
            }
            Err(e) => {
                *error_msg.write() = format!("Identity error: {e}");
                *loading.write() = false;
            }
        }
    };

    let on_restore = move |_| {
        let name = nick_input.read().trim().to_string();
        if name.is_empty() {
            *error_msg.write() = "Please enter a nickname.".into();
            return;
        }
        let phrase = seed_input.read().trim().to_string();
        if phrase.is_empty() {
            *error_msg.write() = "Please paste your seed phrase.".into();
            return;
        }

        *loading.write() = true;
        *error_msg.write() = String::new();

        match imax_core::identity::parse_mnemonic(&phrase) {
            Ok(mnemonic) => {
                let signing_key = imax_core::identity::derive_signing_key(&mnemonic);
                let pubkey_bytes = signing_key.verifying_key().to_bytes();
                let sk_bytes = signing_key.to_bytes();
                let seed_phrase = mnemonic.to_string();

                // Create profile in DB
                let profile_id = {
                    let db = db();
                    let pid = models::create_profile(&db, &seed_phrase, &name).unwrap();
                    models::set_active_profile(&db, pid).unwrap();
                    let total = models::get_all_profiles(&db).unwrap_or_default().len();
                    println!("[imax] Restored profile id={pid} nick={name}, total profiles: {total}");
                    pid
                };
                *ACTIVE_PROFILE_ID.write() = profile_id;
                if is_adding {
                    crate::state::shutdown_node();
                }
                *ADDING_PROFILE.write() = false;

                start_node(sk_bytes, pubkey_bytes, seed_phrase, name);
            }
            Err(e) => {
                *error_msg.write() = format!("Invalid seed phrase: {e}");
                *loading.write() = false;
            }
        }
    };

    let on_cancel = move |_| {
        *IS_ONBOARDED.write() = true;
        *ADDING_PROFILE.write() = false;
    };

    if *show_restore.read() {
        return rsx! {
            div { class: "onboarding-screen",
                div { class: "onboarding-card",
                    div { class: "onboarding-logo", "iMax" }
                    p { class: "onboarding-tagline", "Restore from seed phrase" }

                    div { class: "onboarding-form",
                        div {
                            p { class: "onboarding-label", "Your 24-word seed phrase" }
                            textarea {
                                class: "onboarding-input",
                                rows: "4",
                                placeholder: "word1 word2 word3 ... word24",
                                value: "{seed_input}",
                                oninput: move |evt| {
                                    *seed_input.write() = evt.value();
                                    *error_msg.write() = String::new();
                                },
                            }
                        }

                        div {
                            p { class: "onboarding-label", "Your nickname" }
                            input {
                                class: "onboarding-input",
                                r#type: "text",
                                placeholder: "e.g. Alice",
                                value: "{nick_input}",
                                oninput: move |evt| {
                                    *nick_input.write() = evt.value();
                                    *error_msg.write() = String::new();
                                },
                            }
                        }

                        if !error_msg.read().is_empty() {
                            p { class: "onboarding-error", "{error_msg}" }
                        }

                        button {
                            class: "onboarding-btn-primary",
                            disabled: *loading.read(),
                            onclick: on_restore,
                            if *loading.read() { "Restoring..." } else { "Restore" }
                        }
                    }

                    button {
                        class: "onboarding-link",
                        onclick: move |_| {
                            *show_restore.write() = false;
                            *error_msg.write() = String::new();
                        },
                        "Back"
                    }

                    if is_adding {
                        button {
                            class: "onboarding-link",
                            onclick: on_cancel,
                            "Cancel"
                        }
                    }
                }
            }
        };
    }

    rsx! {
        div { class: "onboarding-screen",
            div { class: "onboarding-card",
                div { class: "onboarding-logo", "iMax" }
                p { class: "onboarding-tagline", "P2P · End-to-End Encrypted · Open Source" }

                div { class: "onboarding-form",
                    div {
                        p { class: "onboarding-label", "Your nickname" }
                        input {
                            class: "onboarding-input",
                            r#type: "text",
                            placeholder: "e.g. Alice",
                            value: "{nick_input}",
                            oninput: move |evt| {
                                *nick_input.write() = evt.value();
                                *error_msg.write() = String::new();
                            },
                        }
                    }

                    if !error_msg.read().is_empty() {
                        p { class: "onboarding-error", "{error_msg}" }
                    }

                    button {
                        class: "onboarding-btn-primary",
                        disabled: *loading.read(),
                        onclick: on_start,
                        if *loading.read() { "Generating keys..." } else { "Start Messaging" }
                    }
                }

                button {
                    class: "onboarding-link",
                    onclick: move |_| {
                        *show_restore.write() = true;
                        *error_msg.write() = String::new();
                    },
                    "I have a seed phrase"
                }

                if is_adding {
                    button {
                        class: "onboarding-link",
                        onclick: on_cancel,
                        "Cancel"
                    }
                }
            }
        }
    }
}

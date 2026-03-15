use dioxus::prelude::*;
use crate::state::{IS_ONBOARDED, NICKNAME, SEED_PHRASE, INVITE_CODE};

#[component]
pub fn Onboarding() -> Element {
    let mut nick_input = use_signal(String::new);
    let mut error_msg = use_signal(String::new);
    let mut loading = use_signal(|| false);

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
                let pubkey = signing_key.verifying_key();

                // Save seed phrase for settings
                *SEED_PHRASE.write() = mnemonic.to_string();

                // Generate invite code from pubkey
                let pubkey_bytes = pubkey.to_bytes();
                let code = format!("imax:{}", bs58::encode(&pubkey_bytes).into_string());
                *INVITE_CODE.write() = code;

                *NICKNAME.write() = name;
                *IS_ONBOARDED.write() = true;
            }
            Err(e) => {
                *error_msg.write() = format!("Identity error: {e}");
                *loading.write() = false;
            }
        }
    };

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
                    onclick: |_| {},
                    "I have a seed phrase"
                }
            }
        }
    }
}

use dioxus::prelude::*;
use crate::state::IS_ONBOARDED;
use crate::views::onboarding::Onboarding;
use crate::views::main_layout::MainLayout;

const CSS: &str = include_str!("../assets/main.css");

#[component]
pub fn App() -> Element {
    let onboarded = IS_ONBOARDED.read();

    rsx! {
        style { {CSS} }
        if *onboarded {
            MainLayout {}
        } else {
            Onboarding {}
        }
    }
}

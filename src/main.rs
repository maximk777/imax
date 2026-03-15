mod app;
mod state;
mod components;
mod views;

fn main() {
    state::init_ui_channel();
    dioxus::launch(app::App);
}

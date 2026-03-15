mod app;
mod state;
mod components;
mod views;

fn main() {
    state::init_ui_channel();
    state::init_db();
    dioxus::launch(app::App);
}

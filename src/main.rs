mod actions;
mod app;
mod assets;
mod components;
mod entities;
mod http;
mod icons;
mod importers;
mod theme;
mod utils;
mod views;

use app::SetuApp;

fn main() {
    SetuApp::run();
}

use dioxus::prelude::*;

mod wallet;
mod rpc;
mod prices;
mod transaction;
mod signing;
mod hardware;
mod storage;
mod components;
mod validators;
mod staking;
mod currency;
mod currency_utils;

use components::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}
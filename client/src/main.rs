pub use client::ui;
use rustls::crypto::aws_lc_rs;
use shared::log::init_log;
pub fn main() -> iced::Result {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");
    let _g = init_log("client", "client");
    //启动ui界面
    ui::run()
}

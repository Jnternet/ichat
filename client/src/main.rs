pub use client::ui;
use rustls::crypto::aws_lc_rs;
pub fn main() -> iced::Result {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");
    ui::login::run()
}

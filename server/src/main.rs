use std::thread::park;

mod login;
mod textchat;
#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tokio::spawn(async move {
        if let Err(e) = textchat::run().await {
            dbg!(e);
        }
    });
    park();
}

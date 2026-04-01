use std::thread::park;

mod login;
mod register;
mod textchat;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tokio::spawn(async move {
        if let Err(e) = register::run().await {
            dbg!(e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = login::run().await {
            dbg!(e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = textchat::run().await {
            dbg!(e);
        }
    });
    park();
}

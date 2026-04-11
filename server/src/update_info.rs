use sea_orm::{DatabaseConnection, TransactionTrait};
use shared::update_info::{GetUpdate, NewMessages};

pub async fn get_new_messages(
    db: DatabaseConnection,
    get_update: GetUpdate,
) -> anyhow::Result<NewMessages> {
    let auth = get_update.auth;
    let last_known = get_update.last_known;
    let txn = db.begin().await?;
    txn.commit().await?;
    todo!()
}

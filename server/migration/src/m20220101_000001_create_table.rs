use crate::sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        eprintln!("Migration up: create accounts");
        //使用事务来保证全部成功
        let db = manager.get_connection();
        let txn = db.begin().await?;
        let txn_manager = SchemaManager::new(&txn);

        txn_manager
            .create_table(
                Table::create()
                    .table("accounts")
                    .if_not_exists()
                    .col(ColumnDef::new("uuid").uuid().primary_key())
                    .col(ColumnDef::new("user_name").string_len(20).not_null())
                    .col(
                        ColumnDef::new("account")
                            .string_len(20)
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new("password").binary().not_null())
                    .col(
                        ColumnDef::new("create_at")
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        //使用事务来保证全部成功
        let db = manager.get_connection();
        let txn = db.begin().await?;
        let txn_manager = SchemaManager::new(&txn);

        txn_manager
            .drop_table(Table::drop().table("accounts").to_owned())
            .await?;

        txn.commit().await
    }
}

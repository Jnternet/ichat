use crate::sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        //使用事务来保证全部成功
        let db = manager.get_connection();
        let txn = db.begin().await?;
        let txn_manager = SchemaManager::new(&txn);

        txn_manager
            .create_table(
                Table::create()
                    .table("auths")
                    .if_not_exists()
                    .col(ColumnDef::new("token").uuid().primary_key())
                    .col(ColumnDef::new("account").uuid().not_null())
                    .col(ColumnDef::new("create_at").timestamp().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auths_account_accounts_uuid")
                            .from("auths", "account")
                            .to("accounts", "uuid")
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
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

        // 核心修正：删除 auths 表（而非错误的 post 表）
        txn_manager
            .drop_table(
                Table::drop()
                    .table("auths") // 表名与 up 中创建的一致
                    .if_exists() // 可选：防止表不存在时报错，更健壮
                    .to_owned(),
            )
            .await?;

        txn.commit().await
    }
}

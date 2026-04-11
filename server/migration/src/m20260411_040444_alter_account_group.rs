use sea_orm::TransactionTrait;
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
            .alter_table(
                Table::alter()
                    .table("account_group")
                    .add_column(ColumnDef::new("last_known").timestamp().null())
                    .to_owned(),
            )
            .await?;

        //必须提交，否则不生效
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        //使用事务来保证全部成功
        let db = manager.get_connection();
        let txn = db.begin().await?;
        let txn_manager = SchemaManager::new(&txn);

        // 逆操作：删除 account_group 表的 last_known 列
        txn_manager
            .alter_table(
                Table::alter()
                    .table("account_group")
                    .drop_column("last_known") // 核心：删除up中添加的列
                    .to_owned(),
            )
            .await?;

        //必须提交，否则不生效
        txn.commit().await
    }
}

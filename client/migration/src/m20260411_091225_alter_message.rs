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

        //使用txn_manager对数据库的结构进行修改
        txn_manager
            .alter_table(
                Table::alter()
                    .table("messages")
                    .add_column(
                        ColumnDef::new("create_at")
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
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

        //使用txn_manager对数据库的结构进行修改
        txn_manager
            .alter_table(
                Table::alter()
                    .table("messages")
                    .drop_column("create_at") // 核心：删除up中添加的列
                    .to_owned(),
            )
            .await?;

        //必须提交，否则不生效
        txn.commit().await
    }
}

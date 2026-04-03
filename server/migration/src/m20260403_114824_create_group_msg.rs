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
            .create_table(
                Table::create()
                    .table("groups")
                    .if_not_exists()
                    .col(ColumnDef::new("uuid").uuid().primary_key())
                    .col(ColumnDef::new("group_name").string_len(20).not_null())
                    .col(
                        ColumnDef::new("create_at")
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        txn_manager
            .create_table(
                Table::create()
                    .table("account_group")
                    .if_not_exists()
                    .col(ColumnDef::new("account_uuid").uuid().not_null())
                    .col(ColumnDef::new("group_uuid").uuid().not_null())
                    .primary_key(
                        Index::create()
                            .name("pk_account_group")
                            .col("account_uuid")
                            .col("group_uuid"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_account")
                            .from("account_group", "account_uuid")
                            .to("accounts", "uuid")
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group")
                            .from("account_group", "group_uuid")
                            .to("groups", "uuid")
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        txn_manager
            .create_table(
                Table::create()
                    .table("messages")
                    .if_not_exists()
                    .col(ColumnDef::new("uuid").uuid().primary_key())
                    .col(ColumnDef::new("content").string().not_null())
                    .col(ColumnDef::new("account_uuid").uuid().not_null())
                    .col(ColumnDef::new("group_uuid").uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_message_account")
                            .from("messages", "account_uuid")
                            .to("accounts", "uuid")
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_message_group")
                            .from("messages", "group_uuid")
                            .to("groups", "uuid")
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
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
        // 🔥 核心：按外键依赖顺序删除表（先删子表，后删主表）
        // 1. 删除 messages 表（依赖 groups ,accounts）
        txn_manager
            .drop_table(Table::drop().table("messages").if_exists().to_owned())
            .await?;

        // 2. 删除 account_group 表（依赖 groups、accounts）
        txn_manager
            .drop_table(Table::drop().table("account_group").if_exists().to_owned())
            .await?;

        // 3. 删除 groups 表（无前置依赖，最后删除）
        txn_manager
            .drop_table(Table::drop().table("groups").if_exists().to_owned())
            .await?;

        //必须提交，否则不生效
        txn.commit().await
    }
}

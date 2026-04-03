pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20260402_080713_create_auths;
mod m20260403_114824_create_group_msg;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20260402_080713_create_auths::Migration),
            Box::new(m20260403_114824_create_group_msg::Migration),
        ]
    }
}

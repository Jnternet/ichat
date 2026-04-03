use crate::entity::prelude::*;
use crate::entity::*;
use anyhow::{Context, bail};
use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter};
use shared::auth::Auth;
use std::str::FromStr;

pub async fn auth(db: &impl ConnectionTrait, auth: &Auth) -> bool {
    match _auth(db, auth).await {
        Ok(b) => b,
        Err(e) => {
            dbg!(&e);
            false
        }
    }
}
async fn _auth(db: &impl ConnectionTrait, auth: &Auth) -> anyhow::Result<bool> {
    // 证明存在且账号令牌相对应
    let a = Auths::find()
        .filter(auths::COLUMN.token.eq(uuid::Uuid::from_str(auth.token())?))
        .filter(
            auths::COLUMN
                .account
                .eq(uuid::Uuid::from_str(auth.account_id())?),
        )
        .one(db)
        .await?
        .context("Wrong authentication")?;
    //检查是否过期
    let create_time = a.create_at;
    let et = std::env::var("TOKEN_EXPIRE_TIME")?.parse::<i64>()?;
    if et < 0 {
        bail!("TOKEN_EXPIRE_TIME cannot below 0");
    }
    let et = chrono::Duration::seconds(et);
    let now = chrono::Utc::now();
    if create_time + et < now {
        bail!("Token expired");
    }
    //现在确认有效，通过
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use sea_orm::{ActiveModelTrait, Database, Set};
    use sha2::Digest;

    #[tokio::test]
    async fn test_auth() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        //准备数据库
        // let server_db_url = std::env::var("SERVER_DATABASE")?;
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        //准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let fake_token = "ddda6ea7f0ad4e98b689b96431fb5927";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a";
        let au = Auth::new(account_id, token);
        let fake_au = Auth::new(account_id, fake_token);

        //插入到数据库中
        let _m = accounts::ActiveModel {
            uuid: Set(account_id.parse()?),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id.parse()?),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        let ans = auth(&db, &au).await;
        assert!(ans);
        let ans = auth(&db, &fake_au).await;
        assert!(!ans);
        Ok(())
    }
}

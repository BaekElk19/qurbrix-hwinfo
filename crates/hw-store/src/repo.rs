// hw-store/src/repo.rs
use crate::component::ComponentRow;
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct ComponentRepo {
    pool: SqlitePool,
}

impl ComponentRepo {
    pub async fn open_or_create(path: &str) -> Result<Self> {
        // sqlite://file_path 形式；开启 WAL + Normal 同步以兼顾可靠性与性能
        let url = format!("sqlite://{}", path);
        let opts = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePool::connect_with(opts).await?;
        let me = Self { pool };
        me.init_schema().await?;
        Ok(me)
    }

    async fn init_schema(&self) -> Result<()> {
        let create = r#"
        CREATE TABLE IF NOT EXISTS component_records (
            fd_CODE      TEXT,
            fd_NAME      TEXT,
            fd_SN        TEXT,
            fd_TYPE      TEXT,
            fd_COMPANY   TEXT,
            fd_VOL       TEXT,
            fd_VOL_REAL  TEXT,
            fd_DEV_TYPE  TEXT,
            fd_INFO_EX1  TEXT,
            fd_INFO_EX2  TEXT,
            fd_INFO_EX3  TEXT,
            fd_INFO_EX4  TEXT,
            fd_INFO_EX5  TEXT,
            fd_INFO_EX6  TEXT,
            fd_INFO_EX7  TEXT,
            fd_INFO_EX8  TEXT,
            fd_INFO_EX9  TEXT,
            fd_INFO_EX10 TEXT,
            PRIMARY KEY(fd_CODE, fd_NAME, fd_SN, fd_INFO_EX10)
        );"#;

        sqlx::query(create).execute(&self.pool).await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_component_sn_bindid ON component_records(fd_SN, fd_INFO_EX10);
            CREATE INDEX IF NOT EXISTS idx_component_type ON component_records(fd_TYPE);
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert(&self, row: &ComponentRow) -> Result<()> {
        const INSERT_SQL: &str = r#"
            INSERT INTO component_records (
                fd_CODE, fd_NAME, fd_SN, fd_TYPE, fd_COMPANY, fd_VOL, fd_VOL_REAL, fd_DEV_TYPE,
                fd_INFO_EX1, fd_INFO_EX2, fd_INFO_EX3, fd_INFO_EX4, fd_INFO_EX5,
                fd_INFO_EX6, fd_INFO_EX7, fd_INFO_EX8, fd_INFO_EX9, fd_INFO_EX10
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8, ?9,?10,?11,?12,?13, ?14,?15,?16,?17,?18)
            ON CONFLICT(fd_CODE, fd_NAME, fd_SN, fd_INFO_EX10) DO UPDATE SET
                fd_TYPE=excluded.fd_TYPE,
                fd_COMPANY=excluded.fd_COMPANY,
                fd_VOL=excluded.fd_VOL,
                fd_VOL_REAL=excluded.fd_VOL_REAL,
                fd_DEV_TYPE=excluded.fd_DEV_TYPE,
                fd_INFO_EX1=excluded.fd_INFO_EX1,
                fd_INFO_EX2=excluded.fd_INFO_EX2,
                fd_INFO_EX3=excluded.fd_INFO_EX3,
                fd_INFO_EX4=excluded.fd_INFO_EX4,
                fd_INFO_EX5=excluded.fd_INFO_EX5,
                fd_INFO_EX6=excluded.fd_INFO_EX6,
                fd_INFO_EX7=excluded.fd_INFO_EX7,
                fd_INFO_EX8=excluded.fd_INFO_EX8,
                fd_INFO_EX9=excluded.fd_INFO_EX9
        "#;

        sqlx::query(INSERT_SQL)
            .bind(row.fd_CODE.as_deref())
            .bind(row.fd_NAME.as_deref())
            .bind(row.fd_SN.as_deref())
            .bind(row.fd_TYPE.as_deref())
            .bind(row.fd_COMPANY.as_deref())
            .bind(row.fd_VOL.as_deref())
            .bind(row.fd_VOL_REAL.as_deref())
            .bind(row.fd_DEV_TYPE.as_deref())
            .bind(row.fd_INFO_EX[0].as_deref())
            .bind(row.fd_INFO_EX[1].as_deref())
            .bind(row.fd_INFO_EX[2].as_deref())
            .bind(row.fd_INFO_EX[3].as_deref())
            .bind(row.fd_INFO_EX[4].as_deref())
            .bind(row.fd_INFO_EX[5].as_deref())
            .bind(row.fd_INFO_EX[6].as_deref())
            .bind(row.fd_INFO_EX[7].as_deref())
            .bind(row.fd_INFO_EX[8].as_deref())
            .bind(row.fd_INFO_EX[9].as_deref())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_batch(&self, rows: &[ComponentRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        const INSERT_SQL: &str = r#"
            INSERT INTO component_records(
                fd_CODE, fd_NAME, fd_SN, fd_TYPE, fd_COMPANY,
                fd_VOL, fd_VOL_REAL, fd_DEV_TYPE,
                fd_INFO_EX1, fd_INFO_EX2, fd_INFO_EX3, fd_INFO_EX4, fd_INFO_EX5,
                fd_INFO_EX6, fd_INFO_EX7, fd_INFO_EX8, fd_INFO_EX9, fd_INFO_EX10
            ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT(fd_CODE, fd_NAME, fd_SN, fd_INFO_EX10) DO UPDATE SET
                fd_TYPE=excluded.fd_TYPE,
                fd_COMPANY=excluded.fd_COMPANY,
                fd_VOL=excluded.fd_VOL,
                fd_VOL_REAL=excluded.fd_VOL_REAL,
                fd_DEV_TYPE=excluded.fd_DEV_TYPE,
                fd_INFO_EX1=excluded.fd_INFO_EX1,
                fd_INFO_EX2=excluded.fd_INFO_EX2,
                fd_INFO_EX3=excluded.fd_INFO_EX3,
                fd_INFO_EX4=excluded.fd_INFO_EX4,
                fd_INFO_EX5=excluded.fd_INFO_EX5,
                fd_INFO_EX6=excluded.fd_INFO_EX6,
                fd_INFO_EX7=excluded.fd_INFO_EX7,
                fd_INFO_EX8=excluded.fd_INFO_EX8,
                fd_INFO_EX9=excluded.fd_INFO_EX9
        "#;

        let mut tx = self.pool.begin().await?;
        for r in rows {
            sqlx::query(INSERT_SQL)
                .bind(r.fd_CODE.as_deref())
                .bind(r.fd_NAME.as_deref())
                .bind(r.fd_SN.as_deref())
                .bind(r.fd_TYPE.as_deref())
                .bind(r.fd_COMPANY.as_deref())
                .bind(r.fd_VOL.as_deref())
                .bind(r.fd_VOL_REAL.as_deref())
                .bind(r.fd_DEV_TYPE.as_deref())
                .bind(r.fd_INFO_EX[0].as_deref())
                .bind(r.fd_INFO_EX[1].as_deref())
                .bind(r.fd_INFO_EX[2].as_deref())
                .bind(r.fd_INFO_EX[3].as_deref())
                .bind(r.fd_INFO_EX[4].as_deref())
                .bind(r.fd_INFO_EX[5].as_deref())
                .bind(r.fd_INFO_EX[6].as_deref())
                .bind(r.fd_INFO_EX[7].as_deref())
                .bind(r.fd_INFO_EX[8].as_deref())
                .bind(r.fd_INFO_EX[9].as_deref())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn fetch_bindid(&self) -> Result<Option<String>> {
        const QUERY: &str = r#"
            SELECT fd_INFO_EX10
            FROM component_records
            WHERE fd_TYPE = '0'
            ORDER BY fd_INFO_EX9 DESC
            LIMIT 1
        "#;
        let result: Option<Option<String>> =
            sqlx::query_scalar(QUERY).fetch_optional(&self.pool).await?;
        Ok(result
            .flatten()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()))
    }
}

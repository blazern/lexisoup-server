use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::str::FromStr;

#[cfg(test)]
pub async fn new_test_pool() -> SqlitePool {
    let spellfix_path = if cfg!(target_os = "macos") {
        "./../prebuilt/macos-arm64/spellfix.dylib"
    } else if cfg!(target_os = "linux") {
        "./../prebuilt/linux-x86_64/spellfix.so"
    } else {
        panic!("unsupported OS for spellfix1");
    };

    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("parse sqlite::memory:")
        .create_if_missing(true)
        .extension(spellfix_path);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connection");
    create_full_schema(&pool).await.expect("schema");
    pool
}

#[cfg(test)]
async fn create_full_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    const SCHEMA: &str = r#"
CREATE TABLE langvar (
id integer PRIMARY KEY,
lang_code text,
var_code integer,
uid text,
meaning integer,
name_expr integer,
name_expr_txt text,
region_expr integer,
region_expr_txt text,
script_expr integer,
script_expr_txt text
);
CREATE TABLE source (
id integer PRIMARY KEY,
grp integer,
label text,
reg_date text,
url text,
isbn text,
author text,
title text,
publisher text,
year text,
quality integer,
note text,
license text,
ip_claim text,
ip_claimant text,
ip_claimant_email text
);
CREATE TABLE expr (
  id integer PRIMARY KEY,
  langvar integer,
  txt text,
  denotation_count integer NOT NULL DEFAULT 0
);
CREATE TABLE denotationx (
meaning integer,
source integer,
grp integer,
quality integer,
expr integer,
langvar integer
);
CREATE VIEW lv AS SELECT id as lv, lang_code as lc, var_code as vc, uid, meaning as mn, name_expr as ex, name_expr_txt as tt, region_expr as rg, region_expr_txt as rgtt, script_expr as sc, script_expr_txt as sctt FROM langvar
/* lv(lv,lc,vc,uid,mn,ex,tt,rg,rgtt,sc,sctt) */;
CREATE VIEW ex AS SELECT id as ex, langvar as lv, txt as tt FROM expr
/* ex(ex,lv,tt) */;
CREATE VIEW dnx AS SELECT meaning as mn, source as ap, grp as ui, quality as uq, expr as ex, langvar as lv FROM denotationx
/* dnx(mn,ap,ui,uq,ex,lv) */;
CREATE INDEX expr_langvar ON expr (langvar);
CREATE INDEX expr_txt_langvar ON expr (txt, langvar);
CREATE INDEX denotationx_meaning ON denotationx (meaning);
CREATE INDEX denotationx_expr ON denotationx (expr);
CREATE INDEX denotationx_langvar ON denotationx (langvar);
CREATE VIRTUAL TABLE spell USING spellfix1;
"#;
    for stmt in SCHEMA.split(';') {
        let sql = stmt.trim();
        if !sql.is_empty() {
            sqlx::query(sql).execute(pool).await?;
        }
    }
    Ok(())
}

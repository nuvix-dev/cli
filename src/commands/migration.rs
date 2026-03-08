use crate::cli::{MigrationNewArgs, MigrationPullArgs, MigrationStatusArgs, MigrationUpArgs};
use crate::global_config::GlobalConfig;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use postgres::{Client, NoTls, Row};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_MIGRATIONS_DIR: &str = "nuvix/migrations/sql";
const DEFAULT_SCHEMA_DIR: &str = "nuvix/schema/sql";
const DEFAULT_REMOTE_SNAPSHOT_FILE: &str = "remote.sql";

#[derive(Debug, Clone)]
struct MigrationFile {
    version: i64,
    name: String,
    path: PathBuf,
    sql: String,
    checksum: String,
}

#[derive(Debug, Clone)]
struct AppliedMigration {
    version: i64,
    checksum: String,
}

#[derive(Debug, Clone)]
struct SchemaInfo {
    name: String,
    schema_type: String,
}

#[derive(Debug, Clone)]
struct TableData {
    schema_name: String,
    table_name: String,
    columns: Vec<ColumnData>,
    constraints: Vec<ConstraintData>,
    indexes: Vec<IndexData>,
    triggers: Vec<TriggerData>,
}

#[derive(Debug, Clone)]
struct ColumnData {
    name: String,
    data_type: String,
    is_nullable: bool,
    default_expr: Option<String>,
}

#[derive(Debug, Clone)]
struct ConstraintData {
    name: String,
    contype: String,
    definition: String,
}

#[derive(Debug, Clone)]
struct IndexData {
    definition: String,
}

#[derive(Debug, Clone)]
struct TriggerData {
    definition: String,
}

pub fn new_migration(project_dir: &Path, args: MigrationNewArgs) -> Result<()> {
    let dir = resolve_migrations_dir(project_dir, args.dir.as_ref());
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create migrations directory: {}", dir.display()))?;

    let stamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let safe_name = normalize_name(&args.name);
    if safe_name.is_empty() {
        bail!("migration name is empty after normalization");
    }

    let file = dir.join(format!("{}_{}.sql", stamp, safe_name));
    let template = "-- Write your SQL migration here\n";
    fs::write(&file, template)
        .with_context(|| format!("failed to write migration file: {}", file.display()))?;

    println!("Created migration: {}", file.display());
    Ok(())
}

pub fn pull(project_dir: &Path, args: MigrationPullArgs) -> Result<()> {
    let database_url =
        resolve_database_url(project_dir, args.project_id.as_deref(), args.database_url)?;
    let mut client =
        Client::connect(&database_url, NoTls).context("failed to connect to PostgreSQL")?;

    let schemas = fetch_sql_schemas(&mut client)?;
    let mut tables = Vec::new();
    for schema in &schemas {
        let mut schema_tables = fetch_schema_tables(&mut client, schema)?;
        tables.append(&mut schema_tables);
    }

    let output = resolve_schema_snapshot_path(project_dir, args.output.as_ref());
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let snapshot_sql = render_sql_snapshot(&schemas, &tables);
    fs::write(&output, snapshot_sql)
        .with_context(|| format!("failed to write snapshot file: {}", output.display()))?;

    println!("Pulled remote SQL schema snapshot.");
    println!("Output: {}", output.display());
    println!("Schemas: {}, Tables: {}", schemas.len(), tables.len());
    Ok(())
}

pub fn up(project_dir: &Path, args: MigrationUpArgs) -> Result<()> {
    let dir = resolve_migrations_dir(project_dir, args.dir.as_ref());
    let migrations = load_migration_files(&dir)?;

    let database_url =
        resolve_database_url(project_dir, args.project_id.as_deref(), args.database_url)?;
    let mut client =
        Client::connect(&database_url, NoTls).context("failed to connect to PostgreSQL")?;

    ensure_migration_table_access(&mut client)?;
    let applied_map = applied_migrations(&mut client)?;

    let mut applied_count = 0usize;
    let mut skipped_count = 0usize;

    for migration in migrations {
        if let Some(applied) = applied_map.get(&migration.version) {
            if applied.checksum != migration.checksum {
                bail!(
                    "checksum mismatch for migration {} ({}). expected {}, got {}",
                    migration.version,
                    migration.name,
                    applied.checksum,
                    migration.checksum
                );
            }
            skipped_count += 1;
            continue;
        }

        let mut tx = client
            .transaction()
            .context("failed to start migration transaction")?;
        tx.batch_execute(&migration.sql).with_context(|| {
            format!(
                "failed executing migration {} ({})",
                migration.version,
                migration.path.display()
            )
        })?;

        run_fix_managed_oid_links(&mut tx)?;

        tx.execute(
            "insert into system.migrations (version, name, checksum) values ($1, $2, $3)",
            &[&migration.version, &migration.name, &migration.checksum],
        )
        .context("failed to insert migration tracking row")?;

        tx.commit()
            .context("failed to commit migration transaction")?;
        applied_count += 1;

        println!("Applied {} ({})", migration.version, migration.name);
    }

    println!(
        "Migrations complete. applied: {}, skipped: {}",
        applied_count, skipped_count
    );
    Ok(())
}

pub fn status(project_dir: &Path, args: MigrationStatusArgs) -> Result<()> {
    let dir = resolve_migrations_dir(project_dir, args.dir.as_ref());
    let migrations = load_migration_files(&dir)?;

    let database_url =
        resolve_database_url(project_dir, args.project_id.as_deref(), args.database_url)?;
    let mut client =
        Client::connect(&database_url, NoTls).context("failed to connect to PostgreSQL")?;

    ensure_migration_table_access(&mut client)?;
    let applied_map = applied_migrations(&mut client)?;

    let mut pending = 0usize;
    let mut applied = 0usize;

    for m in migrations {
        if let Some(row) = applied_map.get(&m.version) {
            if row.checksum != m.checksum {
                println!("! {} {} [checksum-mismatch]", m.version, m.name);
            } else {
                println!("= {} {} [applied]", m.version, m.name);
                applied += 1;
            }
        } else {
            println!("+ {} {} [pending]", m.version, m.name);
            pending += 1;
        }
    }

    println!("Summary => applied: {}, pending: {}", applied, pending);
    Ok(())
}

fn resolve_migrations_dir(project_dir: &Path, custom: Option<&PathBuf>) -> PathBuf {
    match custom {
        Some(v) if v.is_absolute() => v.clone(),
        Some(v) => project_dir.join(v),
        None => project_dir.join(DEFAULT_MIGRATIONS_DIR),
    }
}

fn resolve_schema_snapshot_path(project_dir: &Path, custom: Option<&PathBuf>) -> PathBuf {
    match custom {
        Some(v) if v.is_absolute() => v.clone(),
        Some(v) => project_dir.join(v),
        None => project_dir
            .join(DEFAULT_SCHEMA_DIR)
            .join(DEFAULT_REMOTE_SNAPSHOT_FILE),
    }
}

fn normalize_name(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn load_migration_files(dir: &Path) -> Result<Vec<MigrationFile>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];
    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to read dir: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|v| v.to_str()) != Some("sql") {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|v| v.to_str())
            .context("invalid migration filename")?
            .to_string();

        let (version, name) = parse_migration_filename(&file_name)?;
        let sql = fs::read_to_string(&path)
            .with_context(|| format!("failed to read migration: {}", path.display()))?;
        let checksum = checksum_hex(sql.as_bytes());

        files.push(MigrationFile {
            version,
            name,
            path,
            sql,
            checksum,
        });
    }

    files.sort_by(|a, b| a.version.cmp(&b.version).then(a.name.cmp(&b.name)));
    Ok(files)
}

fn parse_migration_filename(file: &str) -> Result<(i64, String)> {
    let raw = file
        .strip_suffix(".sql")
        .with_context(|| format!("invalid migration extension: {file}"))?;

    let (version, name) = raw
        .split_once('_')
        .with_context(|| format!("invalid migration filename format: {file}"))?;

    if version.is_empty() || name.is_empty() {
        bail!("invalid migration filename format: {file}");
    }

    let version_num = version
        .parse::<i64>()
        .with_context(|| format!("migration version must be bigint-compatible: {file}"))?;

    Ok((version_num, name.to_string()))
}

fn checksum_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

fn ensure_migration_table_access(client: &mut Client) -> Result<()> {
    client
        .query(
            "select version, name, checksum from system.migrations limit 1",
            &[],
        )
        .map(|_| ())
        .context("cannot access system.migrations (missing table or permission)")
}

fn applied_migrations(client: &mut Client) -> Result<BTreeMap<i64, AppliedMigration>> {
    let rows = client
        .query(
            "select version, checksum from system.migrations order by version asc",
            &[],
        )
        .context("failed to read applied migrations")?;

    let mut out = BTreeMap::new();
    for row in rows {
        let item = map_applied(row)?;
        out.insert(item.version, item);
    }
    Ok(out)
}

fn map_applied(row: Row) -> Result<AppliedMigration> {
    let version: i64 = row.try_get("version")?;
    let checksum: String = row.try_get("checksum")?;
    Ok(AppliedMigration { version, checksum })
}

fn run_fix_managed_oid_links(tx: &mut postgres::Transaction<'_>) -> Result<()> {
    let rows = tx
        .query(
            "select name from system.schemas where type = 'managed' order by name",
            &[],
        )
        .context("failed to fetch managed schemas from system.schemas")?;

    for row in rows {
        let schema_name: String = row.try_get("name")?;
        tx.execute(
            "select system.fix_managed_oid_links($1::text)",
            &[&schema_name],
        )
        .with_context(|| {
            format!(
                "failed system.fix_managed_oid_links for schema '{}'",
                schema_name
            )
        })?;
    }

    Ok(())
}

fn fetch_sql_schemas(client: &mut Client) -> Result<Vec<SchemaInfo>> {
    let rows = client
        .query(
            "select name, type from system.schemas where type in ('managed','unmanaged') order by name",
            &[],
        )
        .context("failed to fetch schemas from system.schemas")?;

    let mut out = Vec::new();
    for row in rows {
        out.push(SchemaInfo {
            name: row.try_get("name")?,
            schema_type: row.try_get("type")?,
        });
    }
    Ok(out)
}

fn fetch_schema_tables(client: &mut Client, schema: &SchemaInfo) -> Result<Vec<TableData>> {
    let rows = client
        .query(
            r#"
            select c.relname as table_name
            from pg_class c
            join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = $1
              and c.relkind = 'r'
            order by c.relname
            "#,
            &[&schema.name],
        )
        .with_context(|| format!("failed to list tables for schema {}", schema.name))?;

    let mut tables = Vec::new();
    for row in rows {
        let table_name: String = row.try_get("table_name")?;
        let managed = schema.schema_type == "managed";

        if managed && table_name.ends_with("_perms") {
            continue;
        }

        let reg = regclass_literal(&schema.name, &table_name);
        let columns = fetch_columns(client, &reg, managed)?;
        let constraints = fetch_constraints(client, &reg, managed)?;
        let indexes = fetch_indexes(client, &schema.name, &table_name, &constraints, managed)?;
        let triggers = fetch_triggers(client, &reg, managed)?;

        tables.push(TableData {
            schema_name: schema.name.clone(),
            table_name,
            columns,
            constraints,
            indexes,
            triggers,
        });
    }

    Ok(tables)
}

fn fetch_columns(client: &mut Client, regclass: &str, managed: bool) -> Result<Vec<ColumnData>> {
    let rows = client
        .query(
            r#"
            select a.attname as column_name,
                   pg_catalog.format_type(a.atttypid, a.atttypmod) as data_type,
                   not a.attnotnull as is_nullable,
                   pg_get_expr(ad.adbin, ad.adrelid) as default_expr
            from pg_attribute a
            left join pg_attrdef ad on ad.adrelid = a.attrelid and ad.adnum = a.attnum
            where a.attrelid = to_regclass($1)
              and a.attnum > 0
              and not a.attisdropped
            order by a.attnum
            "#,
            &[&regclass],
        )
        .with_context(|| format!("failed to fetch columns for {}", regclass))?;

    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("column_name")?;
        if managed && name == "_id" {
            continue;
        }

        out.push(ColumnData {
            name,
            data_type: row.try_get("data_type")?,
            is_nullable: row.try_get("is_nullable")?,
            default_expr: row.try_get("default_expr")?,
        });
    }

    Ok(out)
}

fn fetch_constraints(
    client: &mut Client,
    regclass: &str,
    managed: bool,
) -> Result<Vec<ConstraintData>> {
    let rows = client
        .query(
            r#"
            select con.conname,
                   con.contype::text as contype,
                   pg_get_constraintdef(con.oid, true) as condef,
                   coalesce(array_agg(att.attname order by u.ordinality) filter (where att.attname is not null), '{}') as cols
            from pg_constraint con
            left join unnest(con.conkey) with ordinality u(attnum, ordinality) on true
            left join pg_attribute att on att.attrelid = con.conrelid and att.attnum = u.attnum
            where con.conrelid = to_regclass($1)
            group by con.oid, con.conname, con.contype
            order by con.contype, con.conname
            "#,
            &[&regclass],
        )
        .with_context(|| format!("failed to fetch constraints for {}", regclass))?;

    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("conname")?;
        let contype: String = row.try_get("contype")?;
        let definition: String = row.try_get("condef")?;
        let cols: Vec<String> = row.try_get("cols")?;

        if managed {
            let has_id_col = cols.iter().any(|c| c == "_id") || contains_reserved_id(&definition);
            if has_id_col {
                continue;
            }
        }

        out.push(ConstraintData {
            name,
            contype,
            definition,
        });
    }

    Ok(out)
}

fn fetch_indexes(
    client: &mut Client,
    schema_name: &str,
    table_name: &str,
    constraints: &[ConstraintData],
    managed: bool,
) -> Result<Vec<IndexData>> {
    let rows = client
        .query(
            r#"
            select indexname, indexdef
            from pg_indexes
            where schemaname = $1 and tablename = $2
            order by indexname
            "#,
            &[&schema_name, &table_name],
        )
        .with_context(|| format!("failed to fetch indexes for {}.{}", schema_name, table_name))?;

    let constraint_indexes: BTreeSet<String> = constraints.iter().map(|c| c.name.clone()).collect();
    let mut out = Vec::new();

    for row in rows {
        let name: String = row.try_get("indexname")?;
        let definition: String = row.try_get("indexdef")?;

        if constraint_indexes.contains(&name) || name.ends_with("_pkey") {
            continue;
        }

        if managed && is_reserved_id_index(&definition) {
            continue;
        }

        out.push(IndexData { definition });
    }

    Ok(out)
}

fn fetch_triggers(client: &mut Client, regclass: &str, managed: bool) -> Result<Vec<TriggerData>> {
    let rows = client
        .query(
            r#"
            select tg.tgname,
                   pg_get_triggerdef(tg.oid, true) as tgdef
            from pg_trigger tg
            where tg.tgrelid = to_regclass($1)
              and not tg.tgisinternal
            order by tg.tgname
            "#,
            &[&regclass],
        )
        .with_context(|| format!("failed to fetch triggers for {}", regclass))?;

    let mut out = Vec::new();
    for row in rows {
        let name: String = row.try_get("tgname")?;
        let definition: String = row.try_get("tgdef")?;

        if managed {
            if name == "on_row_delete" || definition.contains("on_row_delete") {
                continue;
            }
            if contains_reserved_id(&definition) {
                continue;
            }
        }

        out.push(TriggerData { definition });
    }

    Ok(out)
}

fn render_sql_snapshot(schemas: &[SchemaInfo], tables: &[TableData]) -> String {
    let mut out = String::new();
    out.push_str("-- Nuvix SQL schema snapshot\n");
    out.push_str("-- Generated by `nuvix migration pull`\n\n");

    for schema in schemas {
        out.push_str(&format!(
            "-- schema {} ({})\n",
            schema.name, schema.schema_type
        ));
        out.push_str(&format!(
            "select system.create_schema('{}', '{}', null);\n\n",
            escape_sql_literal(&schema.name),
            escape_sql_literal(&schema.schema_type)
        ));
    }

    for table in tables {
        out.push_str(&render_create_table_sql(table));
        out.push('\n');
    }

    for table in tables {
        for c in table.constraints.iter().filter(|c| c.contype != "f") {
            out.push_str(&format!(
                "alter table \"{}\".\"{}\" add constraint \"{}\" {};\n",
                table.schema_name, table.table_name, c.name, c.definition
            ));
        }
    }

    out.push('\n');

    for table in tables {
        for idx in &table.indexes {
            out.push_str(&format!("{};\n", idx.definition));
        }
    }

    out.push('\n');

    for table in tables {
        for c in table.constraints.iter().filter(|c| c.contype == "f") {
            out.push_str(&format!(
                "alter table \"{}\".\"{}\" add constraint \"{}\" {};\n",
                table.schema_name, table.table_name, c.name, c.definition
            ));
        }
    }

    out.push('\n');

    for table in tables {
        for tg in &table.triggers {
            out.push_str(&format!("{};\n", tg.definition));
        }
    }

    out
}

fn render_create_table_sql(table: &TableData) -> String {
    let mut sql = String::new();
    sql.push_str(&format!(
        "create table \"{}\".\"{}\" (\n",
        table.schema_name, table.table_name
    ));

    for (i, col) in table.columns.iter().enumerate() {
        if i > 0 {
            sql.push_str(",\n");
        }
        sql.push_str(&format!("  \"{}\" {}", col.name, col.data_type));
        if !col.is_nullable {
            sql.push_str(" not null");
        }
        if let Some(def) = &col.default_expr {
            sql.push_str(&format!(" default {}", def));
        }
    }

    sql.push_str("\n);\n");
    sql
}

fn regclass_literal(schema_name: &str, table_name: &str) -> String {
    format!("\"{}\".\"{}\"", schema_name, table_name)
}

#[derive(Debug, Deserialize)]
struct LocalProjectConfig {
    project_id: String,
}

fn read_local_project_id(project_dir: &Path) -> Result<Option<String>> {
    let path = project_dir.join("nuvix").join("config.toml");
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read local project config: {}", path.display()))?;
    let cfg: LocalProjectConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse local project config: {}", path.display()))?;
    Ok(Some(cfg.project_id))
}

fn resolve_database_url(
    project_dir: &Path,
    project_id: Option<&str>,
    explicit: Option<String>,
) -> Result<String> {
    if let Some(url) = explicit {
        return Ok(url);
    }

    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.trim().is_empty() {
            return Ok(url);
        }
    }

    let global = GlobalConfig::load_or_default()?;
    let resolved_project_id = match project_id {
        Some(v) => v.to_string(),
        None => {
            if let Some(local) = read_local_project_id(project_dir)? {
                local
            } else {
                global.resolve_project_id(None)?
            }
        }
    };

    let profile = global
        .projects
        .get(&resolved_project_id)
        .with_context(|| format!("project profile '{}' not found", resolved_project_id))?;

    let env_file = profile.self_host_env_file.as_ref().with_context(|| {
        format!(
            "project '{}' has no self_host_env_file in global config. pass --database-url",
            resolved_project_id
        )
    })?;

    env_to_database_url(env_file)
}

fn env_to_database_url(path: &Path) -> Result<String> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read env file: {}", path.display()))?;

    let mut map = BTreeMap::<String, String>::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), trim_env_value(v.trim()));
        }
    }

    let host = map
        .get("NUVIX_DATABASE_HOST")
        .cloned()
        .unwrap_or_else(|| "localhost".to_string());
    let port = map
        .get("NUVIX_DATABASE_PORT")
        .cloned()
        .unwrap_or_else(|| "5432".to_string());
    let user = map
        .get("NUVIX_DATABASE_USER")
        .cloned()
        .unwrap_or_else(|| "postgres".to_string());
    let password = map
        .get("NUVIX_DATABASE_PASSWORD")
        .cloned()
        .context("NUVIX_DATABASE_PASSWORD missing in env file")?;

    Ok(format!(
        "postgres://{}:{}@{}:{}/postgres",
        user, password, host, port
    ))
}

fn trim_env_value(v: &str) -> String {
    v.trim_matches('"').trim_matches('\'').to_string()
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn contains_reserved_id(definition: &str) -> bool {
    let d = definition.to_lowercase();
    d.contains("\"_id\"")
        || d.contains("(_id)")
        || d.contains("( _id")
        || d.contains(", _id")
        || d.contains("_id)")
}

fn is_reserved_id_index(definition: &str) -> bool {
    let d = definition.to_lowercase();
    (d.contains("create unique index") || d.contains("create index")) && contains_reserved_id(&d)
}

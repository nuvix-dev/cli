use crate::cli::{
    CollectionsAddAttributeArgs, CollectionsAddCollectionArgs, CollectionsAddIndexArgs,
    CollectionsInitArgs, CollectionsListArgs, CollectionsPullArgs, CollectionsPushArgs,
    CollectionsRemoveCollectionArgs, CollectionsShowArgs, CollectionsValidateArgs,
    DocumentAttributeType, DocumentIndexType, IndexOrder,
};
use crate::client::ensure_api_url;
use crate::global_config::{GlobalConfig, load_session};
use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, Input, Select};
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_SCHEMAS_DIR: &str = "nuvix/schemas";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentSchemaFile {
    schema: SchemaSpec,
    #[serde(default)]
    collections: Vec<CollectionSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchemaSpec {
    #[serde(rename = "$id")]
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "type")]
    schema_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionSpec {
    #[serde(rename = "$id")]
    id: String,
    name: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    #[serde(rename = "documentSecurity")]
    document_security: bool,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    attributes: Vec<AttributeSpec>,
    #[serde(default)]
    indexes: Vec<IndexSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AttributeSpec {
    key: String,
    #[serde(rename = "type")]
    attr_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    array: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    elements: Vec<String>,
    #[serde(
        rename = "relatedCollectionId",
        skip_serializing_if = "Option::is_none"
    )]
    related_collection_id: Option<String>,
    #[serde(rename = "relationType", skip_serializing_if = "Option::is_none")]
    relation_type: Option<String>,
    #[serde(rename = "twoWay", skip_serializing_if = "Option::is_none")]
    two_way: Option<bool>,
    #[serde(rename = "twoWayKey", skip_serializing_if = "Option::is_none")]
    two_way_key: Option<String>,
    #[serde(rename = "onDelete", skip_serializing_if = "Option::is_none")]
    on_delete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encrypt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(rename = "new_key", skip_serializing_if = "Option::is_none")]
    new_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexSpec {
    key: String,
    #[serde(rename = "type")]
    index_type: String,
    attributes: Vec<String>,
    #[serde(default)]
    orders: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListEnvelope<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct RemoteSchema {
    #[serde(rename = "$id")]
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "type")]
    schema_type: String,
}

#[derive(Debug, Deserialize)]
struct RemoteCollection {
    #[serde(rename = "$id")]
    id: String,
    name: String,
    enabled: bool,
    #[serde(rename = "documentSecurity")]
    document_security: bool,
    #[serde(default)]
    #[serde(rename = "$permissions")]
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteAttribute {
    key: String,
    #[serde(rename = "type")]
    attr_type: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    array: bool,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    default: Option<Value>,
    #[serde(default)]
    elements: Vec<String>,
    #[serde(default)]
    #[serde(rename = "relatedCollection")]
    related_collection_id: Option<String>,
    #[serde(default)]
    #[serde(rename = "relationType")]
    relation_type: Option<String>,
    #[serde(default)]
    #[serde(rename = "twoWay")]
    two_way: Option<bool>,
    #[serde(default)]
    #[serde(rename = "twoWayKey")]
    two_way_key: Option<String>,
    #[serde(default)]
    #[serde(rename = "onDelete")]
    on_delete: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteIndex {
    key: String,
    #[serde(rename = "type")]
    index_type: String,
    attributes: Vec<String>,
    #[serde(default)]
    orders: Vec<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

struct ApiCtx {
    http: Client,
    base_url: String,
    project_id: String,
    session: String,
}

#[derive(Debug, Clone, Copy)]
struct UiOpts {
    non_interactive: bool,
    yes: bool,
}

pub fn init(project_dir: &Path, args: CollectionsInitArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);

    if path.exists() && !args.force {
        bail!(
            "schema file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let description = prompt_optional(
        "Schema description",
        "Optional: short description for this document schema",
        &ui,
    )?;

    let file = DocumentSchemaFile {
        schema: SchemaSpec {
            id: schema_id.clone(),
            name: schema_id.clone(),
            description,
            schema_type: "document".to_string(),
        },
        collections: vec![],
    };

    write_schema_file(&path, &file)?;
    println!("Initialized document schema.");
    println!("Schema ID: {}", file.schema.id);
    println!("Path: {}", path.display());
    Ok(())
}

pub fn list(project_dir: &Path, args: CollectionsListArgs) -> Result<()> {
    let dir = resolve_schemas_dir(project_dir, args.dir.as_ref());
    if !dir.exists() {
        println!("No schema directory found at {}", dir.display());
        return Ok(());
    }

    let mut files = schema_files_in_dir(&dir)?;
    files.sort();

    if files.is_empty() {
        println!("No document schema files found in {}", dir.display());
        return Ok(());
    }

    println!("Document schema files:");
    for path in files {
        let file = read_schema_file(&path)?;
        println!(
            "- {} (name: {}, collections: {})",
            file.schema.id,
            file.schema.name,
            file.collections.len()
        );
    }
    Ok(())
}

pub fn show(project_dir: &Path, args: CollectionsShowArgs) -> Result<()> {
    let schema_id = resolve_schema_id(
        args.schema,
        &UiOpts {
            non_interactive: false,
            yes: false,
        },
    )?;
    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);
    let file = read_schema_file(&path)?;
    println!("{}", serde_json::to_string_pretty(&file)?);
    Ok(())
}

pub fn add_collection(project_dir: &Path, args: CollectionsAddCollectionArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let collection_id = resolve_collection_id(args.name, &ui)?;
    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);

    let mut file = read_schema_file(&path)?;
    if file
        .collections
        .iter()
        .any(|c| normalize_identifier(&c.id) == collection_id)
    {
        bail!("collection '{}' already exists", collection_id);
    }

    let display_name = prompt_with_default(
        "Collection display name",
        "Human readable name shown in dashboards",
        &collection_id,
        &ui,
    )?;
    let document_security = confirm_or_default("Enable document-level permissions?", false, &ui)?;

    file.collections.push(CollectionSpec {
        id: collection_id.clone(),
        name: display_name,
        enabled: true,
        document_security,
        permissions: vec![],
        attributes: vec![],
        indexes: vec![],
    });

    write_schema_file(&path, &file)?;
    println!(
        "Added collection '{}' to schema '{}'.",
        collection_id, schema_id
    );
    Ok(())
}

pub fn remove_collection(project_dir: &Path, args: CollectionsRemoveCollectionArgs) -> Result<()> {
    let schema_id = resolve_schema_id(
        args.schema,
        &UiOpts {
            non_interactive: false,
            yes: false,
        },
    )?;
    let collection_id = resolve_collection_id(
        args.name,
        &UiOpts {
            non_interactive: false,
            yes: false,
        },
    )?;
    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);

    let mut file = read_schema_file(&path)?;
    let before = file.collections.len();
    file.collections
        .retain(|c| normalize_identifier(&c.id) != collection_id);

    if before == file.collections.len() {
        bail!(
            "collection '{}' not found in schema '{}'",
            collection_id,
            schema_id
        );
    }

    write_schema_file(&path, &file)?;
    println!(
        "Removed collection '{}' from schema '{}'.",
        collection_id, schema_id
    );
    Ok(())
}

pub fn add_attribute(project_dir: &Path, args: CollectionsAddAttributeArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let collection_id = resolve_collection_id(args.collection, &ui)?;
    let key = resolve_attribute_key(args.key, &ui)?;
    let attr_type = resolve_attribute_type(args.attribute_type, &ui)?;

    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);
    let mut file = read_schema_file(&path)?;

    let collection = file
        .collections
        .iter_mut()
        .find(|c| normalize_identifier(&c.id) == collection_id)
        .with_context(|| {
            format!(
                "collection '{}' not found in schema '{}'",
                collection_id, schema_id
            )
        })?;

    if collection
        .attributes
        .iter()
        .any(|a| normalize_identifier(&a.key) == key)
    {
        bail!(
            "attribute '{}' already exists in collection '{}'",
            key,
            collection_id
        );
    }

    let mut spec = AttributeSpec {
        key: key.clone(),
        attr_type: attr_type.as_str().to_string(),
        format: None,
        required: args.required,
        array: args.array,
        size: args.size,
        min: None,
        max: None,
        default: parse_default_value(args.default.as_deref())?,
        elements: normalize_string_list(&args.elements),
        related_collection_id: None,
        relation_type: None,
        two_way: None,
        two_way_key: None,
        on_delete: None,
        encrypt: None,
        status: None,
        error: None,
        new_key: None,
    };

    apply_attribute_defaults_for_type(&mut spec, &ui)?;
    collection.attributes.push(spec);

    write_schema_file(&path, &file)?;
    println!(
        "Added attribute '{}' to collection '{}' in schema '{}'.",
        key, collection_id, schema_id
    );
    Ok(())
}

pub fn add_index(project_dir: &Path, args: CollectionsAddIndexArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let collection_id = resolve_collection_id(args.collection, &ui)?;
    let index_key = resolve_index_key(args.key, &ui)?;
    let index_type = resolve_index_type(args.index_type, &ui)?;

    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);
    let mut file = read_schema_file(&path)?;

    let collection = file
        .collections
        .iter_mut()
        .find(|c| normalize_identifier(&c.id) == collection_id)
        .with_context(|| {
            format!(
                "collection '{}' not found in schema '{}'",
                collection_id, schema_id
            )
        })?;

    if collection
        .indexes
        .iter()
        .any(|i| normalize_identifier(&i.key) == index_key)
    {
        bail!(
            "index '{}' already exists in collection '{}'",
            index_key,
            collection_id
        );
    }

    let attributes = if args.attributes.is_empty() {
        prompt_csv(
            "Index attributes",
            "Comma-separated attribute keys included in this index",
            &ui,
        )?
    } else {
        normalize_string_list(&args.attributes)
    };

    if attributes.is_empty() {
        bail!("index requires at least one attribute");
    }

    ensure_index_attributes_exist(collection, &attributes)?;

    let orders = if args.orders.is_empty() {
        vec!["ASC".to_string(); attributes.len()]
    } else {
        let values = args
            .orders
            .iter()
            .map(IndexOrder::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if values.len() != attributes.len() {
            bail!("--orders count must match --attributes count");
        }
        values
    };

    collection.indexes.push(IndexSpec {
        key: index_key.clone(),
        index_type: index_type.as_str().to_string(),
        attributes,
        orders,
        status: None,
        error: None,
    });

    write_schema_file(&path, &file)?;
    println!(
        "Added index '{}' to collection '{}' in schema '{}'.",
        index_key, collection_id, schema_id
    );
    Ok(())
}

pub fn pull(project_dir: &Path, args: CollectionsPullArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let ctx = api_context(args.project_id.as_deref(), &ui)?;

    let schema = fetch_schema(&ctx, &schema_id)?;
    if schema.schema_type != "document" {
        bail!(
            "schema '{}' is type '{}', expected 'document'",
            schema_id,
            schema.schema_type
        );
    }

    let remote_collections: Vec<RemoteCollection> =
        get_list(&ctx, &format!("/schemas/{}/collections", schema.id))?;

    let mut collections = Vec::new();
    for rc in remote_collections {
        let attributes: Vec<RemoteAttribute> = get_list(
            &ctx,
            &format!("/schemas/{}/collections/{}/attributes", schema.id, rc.id),
        )?;
        let indexes: Vec<RemoteIndex> = get_list(
            &ctx,
            &format!("/schemas/{}/collections/{}/indexes", schema.id, rc.id),
        )?;

        collections.push(CollectionSpec {
            id: normalize_identifier(&rc.id),
            name: rc.name,
            enabled: rc.enabled,
            document_security: rc.document_security,
            permissions: rc.permissions,
            attributes: attributes
                .into_iter()
                .map(remote_attribute_to_local)
                .collect(),
            indexes: indexes.into_iter().map(remote_index_to_local).collect(),
        });
    }

    let file = DocumentSchemaFile {
        schema: SchemaSpec {
            id: schema.id.clone(),
            name: schema.name,
            description: schema.description,
            schema_type: schema.schema_type,
        },
        collections,
    };

    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }
    write_schema_file(&path, &file)?;

    println!("Pulled schema '{}' from remote.", schema.id);
    println!("Project: {}", ctx.project_id);
    println!("Collections: {}", file.collections.len());
    println!("Output: {}", path.display());
    Ok(())
}

pub fn push(project_dir: &Path, args: CollectionsPushArgs) -> Result<()> {
    let ui = UiOpts {
        non_interactive: args.non_interactive,
        yes: args.yes,
    };
    let schema_id = resolve_schema_id(args.schema, &ui)?;
    let path = schema_file_path(project_dir, args.dir.as_ref(), &schema_id);
    let local = read_schema_file(&path)?;
    validate_schema_file(&local)?;

    let ctx = api_context(args.project_id.as_deref(), &ui)?;
    let remote_schema_id = ensure_document_schema(&ctx, &local.schema, args.dry_run)?;

    let remote_collections: Vec<RemoteCollection> =
        get_list(&ctx, &format!("/schemas/{}/collections", remote_schema_id))?;
    let remote_collections_map = remote_collections
        .into_iter()
        .map(|c| (normalize_identifier(&c.id), c))
        .collect::<BTreeMap<_, _>>();

    let local_ids = local
        .collections
        .iter()
        .map(|c| normalize_identifier(&c.id))
        .collect::<BTreeSet<_>>();

    for collection in &local.collections {
        sync_collection(
            &ctx,
            &remote_schema_id,
            collection,
            &remote_collections_map,
            &ui,
            args.dry_run,
        )?;
    }

    let stale_collections = remote_collections_map
        .keys()
        .filter(|id| !local_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();

    if !stale_collections.is_empty()
        && confirm_or_default(
            &format!(
                "Delete {} remote collections missing in local file?",
                stale_collections.len()
            ),
            false,
            &ui,
        )?
    {
        for collection_id in stale_collections {
            if args.dry_run {
                println!("[dry-run] delete remote collection '{}'", collection_id);
            } else {
                delete_empty(
                    &ctx,
                    &format!(
                        "/schemas/{}/collections/{}",
                        remote_schema_id, collection_id
                    ),
                )?;
                println!("Deleted remote collection '{}'.", collection_id);
            }
        }
    }

    if args.dry_run {
        println!("Dry-run completed. No remote changes were made.");
    }
    println!("Push completed for schema '{}'.", remote_schema_id);
    println!("Project: {}", ctx.project_id);
    Ok(())
}

pub fn validate(project_dir: &Path, args: CollectionsValidateArgs) -> Result<()> {
    let dir = resolve_schemas_dir(project_dir, args.dir.as_ref());
    if !dir.exists() {
        bail!("schema directory not found: {}", dir.display());
    }

    let mut targets = Vec::new();
    if let Some(schema) = args.schema {
        let schema_id = normalize_identifier(&schema);
        if schema_id.is_empty() {
            bail!("schema id is invalid");
        }
        targets.push(schema_file_path(project_dir, args.dir.as_ref(), &schema_id));
    } else {
        targets = schema_files_in_dir(&dir)?;
    }

    if targets.is_empty() {
        bail!("no schema files found to validate in {}", dir.display());
    }

    targets.sort();
    for path in targets {
        let file = read_schema_file(&path)?;
        validate_schema_file(&file)
            .with_context(|| format!("validation failed for {}", path.display()))?;
        println!("ok {}", path.display());
    }
    Ok(())
}

fn sync_collection(
    ctx: &ApiCtx,
    schema_id: &str,
    local: &CollectionSpec,
    remote_collections: &BTreeMap<String, RemoteCollection>,
    ui: &UiOpts,
    dry_run: bool,
) -> Result<()> {
    let collection_id = normalize_identifier(&local.id);

    if let Some(remote) = remote_collections.get(&collection_id) {
        let update = json!({
            "name": local.name,
            "permissions": local.permissions,
            "documentSecurity": local.document_security,
            "enabled": local.enabled
        });
        if dry_run {
            println!("[dry-run] update collection '{}'", collection_id);
        } else {
            put_json::<Value>(
                ctx,
                &format!("/schemas/{}/collections/{}", schema_id, collection_id),
                &update,
            )?;
        }

        if remote.name != local.name {
            println!("Updated collection '{}'.", collection_id);
        }
    } else {
        let create = json!({
            "collectionId": collection_id,
            "name": local.name,
            "permissions": local.permissions,
            "documentSecurity": local.document_security,
            "enabled": local.enabled
        });
        if dry_run {
            println!("[dry-run] create collection '{}'", collection_id);
        } else {
            post_json::<Value>(ctx, &format!("/schemas/{}/collections", schema_id), &create)?;
            println!("Created collection '{}'.", collection_id);
        }
    }

    sync_collection_attributes(
        ctx,
        schema_id,
        &collection_id,
        &local.attributes,
        ui,
        dry_run,
    )?;
    sync_collection_indexes(ctx, schema_id, &collection_id, &local.indexes, ui, dry_run)?;
    Ok(())
}

fn sync_collection_attributes(
    ctx: &ApiCtx,
    schema_id: &str,
    collection_id: &str,
    local_attributes: &[AttributeSpec],
    ui: &UiOpts,
    dry_run: bool,
) -> Result<()> {
    let remote: Vec<RemoteAttribute> = get_list(
        ctx,
        &format!(
            "/schemas/{}/collections/{}/attributes",
            schema_id, collection_id
        ),
    )?;

    let remote_map = remote
        .into_iter()
        .map(|a| (normalize_identifier(&a.key), a))
        .collect::<BTreeMap<_, _>>();

    let local_keys = local_attributes
        .iter()
        .map(|a| normalize_identifier(&a.key))
        .collect::<BTreeSet<_>>();

    for attr in local_attributes {
        let key = normalize_identifier(&attr.key);
        let (typed, create_payload) = create_attribute_payload(attr)?;

        if remote_map.contains_key(&key) {
            let update_payload = update_attribute_payload(attr)?;
            if dry_run {
                println!(
                    "[dry-run] update attribute '{}.{}.{}'",
                    schema_id, collection_id, key
                );
            } else {
                patch_json::<Value>(
                    ctx,
                    &format!(
                        "/schemas/{}/collections/{}/attributes/{}/{}",
                        schema_id, collection_id, typed, key
                    ),
                    &update_payload,
                )?;
            }
        } else {
            if dry_run {
                println!(
                    "[dry-run] create attribute '{}.{}.{}'",
                    schema_id, collection_id, key
                );
            } else {
                post_json::<Value>(
                    ctx,
                    &format!(
                        "/schemas/{}/collections/{}/attributes/{}",
                        schema_id, collection_id, typed
                    ),
                    &create_payload,
                )?;
                println!(
                    "Created attribute '{}.{}.{}'.",
                    schema_id, collection_id, key
                );
            }
        }
    }

    let stale = remote_map
        .keys()
        .filter(|key| !local_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();

    if !stale.is_empty()
        && confirm_or_default(
            &format!(
                "Delete {} remote attributes missing in local '{}'?",
                stale.len(),
                collection_id
            ),
            false,
            ui,
        )?
    {
        for key in stale {
            if dry_run {
                println!(
                    "[dry-run] delete attribute '{}.{}.{}'",
                    schema_id, collection_id, key
                );
            } else {
                delete_empty(
                    ctx,
                    &format!(
                        "/schemas/{}/collections/{}/attributes/{}",
                        schema_id, collection_id, key
                    ),
                )?;
                println!(
                    "Deleted attribute '{}.{}.{}'.",
                    schema_id, collection_id, key
                );
            }
        }
    }

    Ok(())
}

fn sync_collection_indexes(
    ctx: &ApiCtx,
    schema_id: &str,
    collection_id: &str,
    local_indexes: &[IndexSpec],
    ui: &UiOpts,
    dry_run: bool,
) -> Result<()> {
    let remote: Vec<RemoteIndex> = get_list(
        ctx,
        &format!(
            "/schemas/{}/collections/{}/indexes",
            schema_id, collection_id
        ),
    )?;

    let remote_map = remote
        .into_iter()
        .map(|i| (normalize_identifier(&i.key), i))
        .collect::<BTreeMap<_, _>>();

    let local_keys = local_indexes
        .iter()
        .map(|i| normalize_identifier(&i.key))
        .collect::<BTreeSet<_>>();

    for index in local_indexes {
        let key = normalize_identifier(&index.key);
        let payload = json!({
            "key": key,
            "type": index.index_type,
            "attributes": index.attributes,
            "orders": normalized_orders(index)
        });

        match remote_map.get(&key) {
            Some(existing) => {
                let same = existing.index_type == index.index_type
                    && existing.attributes == index.attributes
                    && existing.orders == normalized_orders(index);
                if !same {
                    if dry_run {
                        println!(
                            "[dry-run] recreate index '{}.{}.{}'",
                            schema_id, collection_id, key
                        );
                    } else {
                        delete_empty(
                            ctx,
                            &format!(
                                "/schemas/{}/collections/{}/indexes/{}",
                                schema_id, collection_id, key
                            ),
                        )?;
                        post_json::<Value>(
                            ctx,
                            &format!(
                                "/schemas/{}/collections/{}/indexes",
                                schema_id, collection_id
                            ),
                            &payload,
                        )?;
                        println!("Recreated index '{}.{}.{}'.", schema_id, collection_id, key);
                    }
                }
            }
            None => {
                if dry_run {
                    println!(
                        "[dry-run] create index '{}.{}.{}'",
                        schema_id, collection_id, key
                    );
                } else {
                    post_json::<Value>(
                        ctx,
                        &format!(
                            "/schemas/{}/collections/{}/indexes",
                            schema_id, collection_id
                        ),
                        &payload,
                    )?;
                    println!("Created index '{}.{}.{}'.", schema_id, collection_id, key);
                }
            }
        }
    }

    let stale = remote_map
        .keys()
        .filter(|key| !local_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();

    if !stale.is_empty()
        && confirm_or_default(
            &format!(
                "Delete {} remote indexes missing in local '{}'?",
                stale.len(),
                collection_id
            ),
            false,
            ui,
        )?
    {
        for key in stale {
            if dry_run {
                println!(
                    "[dry-run] delete index '{}.{}.{}'",
                    schema_id, collection_id, key
                );
            } else {
                delete_empty(
                    ctx,
                    &format!(
                        "/schemas/{}/collections/{}/indexes/{}",
                        schema_id, collection_id, key
                    ),
                )?;
                println!("Deleted index '{}.{}.{}'.", schema_id, collection_id, key);
            }
        }
    }

    Ok(())
}

fn normalized_orders(index: &IndexSpec) -> Vec<String> {
    if index.orders.is_empty() {
        vec!["ASC".to_string(); index.attributes.len()]
    } else {
        index.orders.clone()
    }
}

fn ensure_document_schema(ctx: &ApiCtx, local: &SchemaSpec, dry_run: bool) -> Result<String> {
    if let Ok(schema) = fetch_schema(ctx, &local.id) {
        if schema.schema_type != "document" {
            bail!(
                "schema '{}' exists but type is '{}', expected 'document'",
                local.id,
                schema.schema_type
            );
        }
        return Ok(schema.id);
    }

    let schemas: Vec<RemoteSchema> = get_list(ctx, "/database/schemas")?;
    if let Some(existing) = schemas
        .into_iter()
        .find(|s| s.name == local.name && s.schema_type == "document")
    {
        return Ok(existing.id);
    }

    let payload = json!({
        "name": local.name,
        "description": local.description,
        "type": "document"
    });
    if dry_run {
        println!("[dry-run] create document schema '{}'", local.name);
        return Ok(local.id.clone());
    }

    let created: RemoteSchema = post_json(ctx, "/database/schemas", &payload)?;
    println!("Created document schema '{}'.", created.id);
    Ok(created.id)
}

fn fetch_schema(ctx: &ApiCtx, schema_id: &str) -> Result<RemoteSchema> {
    get_json(ctx, &format!("/database/schemas/{}", schema_id))
}

fn remote_attribute_to_local(attr: RemoteAttribute) -> AttributeSpec {
    let kind = match (attr.attr_type.as_str(), attr.format.as_deref()) {
        ("string", Some("email")) => "email",
        ("string", Some("enum")) => "enum",
        ("string", Some("ip")) => "ip",
        ("string", Some("url")) => "url",
        ("string", Some("datetime")) => "datetime",
        ("relationship", _) => "relationship",
        ("timestamptz", _) => "timestamptz",
        _ => attr.attr_type.as_str(),
    }
    .to_string();

    AttributeSpec {
        key: normalize_identifier(&attr.key),
        attr_type: kind,
        format: attr.format,
        required: attr.required,
        array: attr.array,
        size: attr.size,
        min: attr.min,
        max: attr.max,
        default: attr.default,
        elements: attr.elements,
        related_collection_id: attr.related_collection_id,
        relation_type: attr.relation_type,
        two_way: attr.two_way,
        two_way_key: attr.two_way_key,
        on_delete: attr.on_delete,
        encrypt: None,
        status: attr.status,
        error: attr.error,
        new_key: None,
    }
}

fn remote_index_to_local(idx: RemoteIndex) -> IndexSpec {
    IndexSpec {
        key: normalize_identifier(&idx.key),
        index_type: idx.index_type,
        attributes: normalize_string_list(&idx.attributes),
        orders: idx.orders,
        status: idx.status,
        error: idx.error,
    }
}

fn create_attribute_payload(attr: &AttributeSpec) -> Result<(&'static str, Value)> {
    let key = normalize_identifier(&attr.key);
    match attr.attr_type.as_str() {
        "string" => Ok((
            "string",
            json!({
                "key": key,
                "size": attr.size.unwrap_or(255),
                "required": attr.required,
                "default": attr.default,
                "array": attr.array,
                "encrypt": attr.encrypt.unwrap_or(false)
            }),
        )),
        "email" => Ok((
            "email",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "enum" => {
            if attr.elements.is_empty() {
                bail!("enum attribute '{}' requires elements", attr.key);
            }
            Ok((
                "enum",
                json!({
                    "key": key,
                    "required": attr.required,
                    "default": attr.default,
                    "array": attr.array,
                    "elements": attr.elements
                }),
            ))
        }
        "ip" => Ok((
            "ip",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "url" => Ok((
            "url",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "integer" => Ok((
            "integer",
            json!({
                "key": key,
                "required": attr.required,
                "default": attr.default,
                "array": attr.array,
                "min": attr.min,
                "max": attr.max
            }),
        )),
        "float" => Ok((
            "float",
            json!({
                "key": key,
                "required": attr.required,
                "default": attr.default,
                "array": attr.array,
                "min": attr.min,
                "max": attr.max
            }),
        )),
        "boolean" => Ok((
            "boolean",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "datetime" => Ok((
            "datetime",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "timestamptz" => Ok((
            "timestamptz",
            json!({"key": key, "required": attr.required, "default": attr.default, "array": attr.array}),
        )),
        "relationship" => Ok((
            "relationship",
            json!({
                "key": key,
                "type": attr.relation_type.as_deref().unwrap_or("manyToOne"),
                "onDelete": attr.on_delete.as_deref().unwrap_or("restrict"),
                "relatedCollectionId": attr.related_collection_id,
                "twoWay": attr.two_way.unwrap_or(false),
                "twoWayKey": attr.two_way_key
            }),
        )),
        other => bail!("unsupported attribute type '{}'", other),
    }
}

fn update_attribute_payload(attr: &AttributeSpec) -> Result<Value> {
    let payload = match attr.attr_type.as_str() {
        "string" => json!({
            "new_key": attr.new_key,
            "size": attr.size.unwrap_or(255),
            "required": attr.required,
            "default": attr.default
        }),
        "email" => json!({
            "new_key": attr.new_key,
            "required": attr.required,
            "default": attr.default
        }),
        "enum" => json!({
            "new_key": attr.new_key,
            "required": attr.required,
            "default": attr.default,
            "elements": attr.elements
        }),
        "ip" | "url" | "datetime" | "timestamptz" => json!({
            "new_key": attr.new_key,
            "required": attr.required,
            "default": attr.default
        }),
        "integer" | "float" => json!({
            "new_key": attr.new_key,
            "required": attr.required,
            "default": attr.default,
            "min": attr.min,
            "max": attr.max
        }),
        "boolean" => json!({
            "new_key": attr.new_key,
            "required": attr.required,
            "default": attr.default
        }),
        "relationship" => json!({
            "new_key": attr.new_key,
            "onDelete": attr.on_delete
        }),
        other => bail!("unsupported attribute type '{}'", other),
    };
    Ok(payload)
}

fn apply_attribute_defaults_for_type(spec: &mut AttributeSpec, ui: &UiOpts) -> Result<()> {
    match spec.attr_type.as_str() {
        "string" => {
            if spec.size.is_none() {
                let value: u32 = if ui.non_interactive {
                    255
                } else {
                    Input::new()
                        .with_prompt("String size (1..65535)")
                        .default(255)
                        .interact_text()?
                };
                spec.size = Some(value.max(1));
            }
            spec.encrypt = Some(confirm_or_default(
                "Encrypt this attribute? (encrypted fields are not queryable)",
                false,
                ui,
            )?);
        }
        "enum" => {
            if spec.elements.is_empty() {
                spec.elements = prompt_csv(
                    "Enum values",
                    "Comma-separated enum values, e.g. pending,paid,cancelled",
                    ui,
                )?;
            }
        }
        "integer" | "float" => {
            let min = prompt_optional(
                "Minimum value",
                "Optional lower bound. Leave empty if no lower bound",
                ui,
            )?;
            let max = prompt_optional(
                "Maximum value",
                "Optional upper bound. Leave empty if no upper bound",
                ui,
            )?;
            spec.min = min.and_then(|v| v.parse::<f64>().ok());
            spec.max = max.and_then(|v| v.parse::<f64>().ok());
        }
        "relationship" => {
            spec.related_collection_id = Some(resolve_collection_id(None, ui)?);
            let relation = ["oneToOne", "oneToMany", "manyToOne", "manyToMany"];
            let idx = if ui.non_interactive {
                2
            } else {
                Select::new()
                    .with_prompt("Relationship type")
                    .default(2)
                    .items(&relation)
                    .interact()?
            };
            spec.relation_type = Some(relation[idx].to_string());

            let on_delete = ["restrict", "cascade", "setNull"];
            let odx = if ui.non_interactive {
                0
            } else {
                Select::new()
                    .with_prompt("On delete behavior")
                    .default(0)
                    .items(&on_delete)
                    .interact()?
            };
            spec.on_delete = Some(on_delete[odx].to_string());

            let two_way = confirm_or_default("Create two-way relationship?", false, ui)?;
            spec.two_way = Some(two_way);
            if two_way {
                let two_way_key = prompt_required(
                    None,
                    "Two-way attribute key",
                    "Attribute key created on the related collection",
                    ui,
                )?;
                spec.two_way_key = Some(two_way_key);
            }
        }
        "email" => spec.format = Some("email".to_string()),
        "ip" => spec.format = Some("ip".to_string()),
        "url" => spec.format = Some("url".to_string()),
        "datetime" => spec.format = Some("datetime".to_string()),
        "timestamptz" | "boolean" => {}
        other => bail!("unsupported attribute type '{}'", other),
    }

    Ok(())
}

fn resolve_schema_id(input: Option<String>, ui: &UiOpts) -> Result<String> {
    prompt_required(
        input,
        "Schema ID",
        "Unique document schema identifier, used for remote sync and file name",
        ui,
    )
}

fn resolve_collection_id(input: Option<String>, ui: &UiOpts) -> Result<String> {
    prompt_required(
        input,
        "Collection ID",
        "Unique collection identifier (a-z, 0-9, underscore)",
        ui,
    )
}

fn resolve_attribute_key(input: Option<String>, ui: &UiOpts) -> Result<String> {
    prompt_required(
        input,
        "Attribute key",
        "Unique attribute key inside the collection",
        ui,
    )
}

fn resolve_index_key(input: Option<String>, ui: &UiOpts) -> Result<String> {
    prompt_required(
        input,
        "Index key",
        "Unique index key inside the collection",
        ui,
    )
}

fn resolve_attribute_type(
    input: Option<DocumentAttributeType>,
    ui: &UiOpts,
) -> Result<DocumentAttributeType> {
    if let Some(v) = input {
        return Ok(v);
    }
    if ui.non_interactive {
        bail!("--attribute-type is required in --non-interactive mode");
    }
    let items = [
        DocumentAttributeType::String,
        DocumentAttributeType::Integer,
        DocumentAttributeType::Float,
        DocumentAttributeType::Boolean,
        DocumentAttributeType::Datetime,
        DocumentAttributeType::Timestamptz,
        DocumentAttributeType::Email,
        DocumentAttributeType::Url,
        DocumentAttributeType::Ip,
        DocumentAttributeType::Enum,
        DocumentAttributeType::Relationship,
    ];
    let labels = items
        .iter()
        .map(DocumentAttributeType::as_str)
        .collect::<Vec<_>>();
    let idx = Select::new()
        .with_prompt("Attribute type")
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(items[idx].clone())
}

fn resolve_index_type(input: Option<DocumentIndexType>, ui: &UiOpts) -> Result<DocumentIndexType> {
    if let Some(v) = input {
        return Ok(v);
    }
    if ui.non_interactive {
        bail!("--index-type is required in --non-interactive mode");
    }
    let items = [
        DocumentIndexType::Key,
        DocumentIndexType::Unique,
        DocumentIndexType::Fulltext,
    ];
    let labels = items
        .iter()
        .map(DocumentIndexType::as_str)
        .collect::<Vec<_>>();
    let idx = Select::new()
        .with_prompt("Index type")
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(items[idx].clone())
}

fn prompt_required(
    input: Option<String>,
    label: &str,
    description: &str,
    ui: &UiOpts,
) -> Result<String> {
    if let Some(value) = input {
        let normalized = normalize_identifier(&value);
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }

    if ui.non_interactive {
        bail!(
            "{} is required in --non-interactive mode. pass the matching -- flag",
            label
        );
    }

    let raw = Input::<String>::new()
        .with_prompt(format!("{} ({})", label, description))
        .interact_text()?;
    let normalized = normalize_identifier(&raw);
    if normalized.is_empty() {
        bail!("{} is invalid", label);
    }
    Ok(normalized)
}

fn prompt_with_default(
    label: &str,
    description: &str,
    default: &str,
    ui: &UiOpts,
) -> Result<String> {
    if ui.non_interactive {
        return Ok(default.to_string());
    }
    let raw = Input::<String>::new()
        .with_prompt(format!("{} ({})", label, description))
        .default(default.to_string())
        .interact_text()?;
    Ok(raw.trim().to_string())
}

fn prompt_optional(label: &str, description: &str, ui: &UiOpts) -> Result<Option<String>> {
    if ui.non_interactive {
        return Ok(None);
    }
    let raw = Input::<String>::new()
        .allow_empty(true)
        .with_prompt(format!("{} ({})", label, description))
        .interact_text()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn prompt_csv(label: &str, description: &str, ui: &UiOpts) -> Result<Vec<String>> {
    if ui.non_interactive {
        bail!("{} is required in --non-interactive mode", label);
    }
    let raw = Input::<String>::new()
        .with_prompt(format!("{} ({})", label, description))
        .interact_text()?;
    Ok(raw
        .split(',')
        .map(normalize_identifier)
        .filter(|v| !v.is_empty())
        .collect())
}

fn api_context(requested_project: Option<&str>, ui: &UiOpts) -> Result<ApiCtx> {
    let global = GlobalConfig::load_or_default()?;

    let project_id = match requested_project {
        Some(v) => v.to_string(),
        None => match global.resolve_project_id(None) {
            Ok(v) => v,
            Err(_) => {
                if ui.non_interactive {
                    bail!(
                        "project id is required in --non-interactive mode. pass --project-id or set current project"
                    );
                }
                Input::<String>::new()
                    .with_prompt("Project ID (from `nuvix project` profile)")
                    .interact_text()?
            }
        },
    };

    let profile = global
        .projects
        .get(&project_id)
        .with_context(|| format!("project profile '{}' not found", project_id))?;

    let session = load_session(&project_id, profile)
        .context("missing nc_session. Run `nuvix auth login` first")?;

    let base_url = ensure_api_url(profile)?;
    let http = Client::builder()
        .build()
        .context("failed to initialize HTTP client")?;

    Ok(ApiCtx {
        http,
        base_url: base_url.trim_end_matches('/').to_string(),
        project_id,
        session,
    })
}

fn confirm_or_default(prompt: &str, default: bool, ui: &UiOpts) -> Result<bool> {
    if ui.non_interactive {
        return Ok(ui.yes || default);
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()
        .map_err(Into::into)
}

fn request(ctx: &ApiCtx, builder: RequestBuilder) -> RequestBuilder {
    builder
        .header("X-Nuvix-Project", &ctx.project_id)
        .header("x-nuvix-session", &ctx.session)
        .header("x-nuvix-mode", "admin")
}

fn get_list<T: for<'de> Deserialize<'de>>(ctx: &ApiCtx, path: &str) -> Result<Vec<T>> {
    let resp = request(ctx, ctx.http.get(format!("{}{}", ctx.base_url, path)))
        .send()
        .with_context(|| format!("request failed: GET {}", path))?;
    let resp = ensure_success(resp, path)?;
    let value: Value = resp.json().context("failed to decode list response")?;

    if let Ok(list) = serde_json::from_value::<ListEnvelope<T>>(value.clone()) {
        return Ok(list.data);
    }

    if let Some(data) = value.get("data") {
        return serde_json::from_value::<Vec<T>>(data.clone())
            .context("failed to decode list response data");
    }

    bail!("unexpected list response shape for {}", path)
}

fn get_json<T: for<'de> Deserialize<'de>>(ctx: &ApiCtx, path: &str) -> Result<T> {
    let resp = request(ctx, ctx.http.get(format!("{}{}", ctx.base_url, path)))
        .send()
        .with_context(|| format!("request failed: GET {}", path))?;
    ensure_success(resp, path)?
        .json::<T>()
        .context("failed to decode response body")
}

fn post_json<T: for<'de> Deserialize<'de>>(ctx: &ApiCtx, path: &str, payload: &Value) -> Result<T> {
    let resp = request(ctx, ctx.http.post(format!("{}{}", ctx.base_url, path)))
        .json(payload)
        .send()
        .with_context(|| format!("request failed: POST {}", path))?;
    ensure_success(resp, path)?
        .json::<T>()
        .context("failed to decode response body")
}

fn put_json<T: for<'de> Deserialize<'de>>(ctx: &ApiCtx, path: &str, payload: &Value) -> Result<T> {
    let resp = request(ctx, ctx.http.put(format!("{}{}", ctx.base_url, path)))
        .json(payload)
        .send()
        .with_context(|| format!("request failed: PUT {}", path))?;
    ensure_success(resp, path)?
        .json::<T>()
        .context("failed to decode response body")
}

fn patch_json<T: for<'de> Deserialize<'de>>(
    ctx: &ApiCtx,
    path: &str,
    payload: &Value,
) -> Result<T> {
    let resp = request(ctx, ctx.http.patch(format!("{}{}", ctx.base_url, path)))
        .json(payload)
        .send()
        .with_context(|| format!("request failed: PATCH {}", path))?;
    ensure_success(resp, path)?
        .json::<T>()
        .context("failed to decode response body")
}

fn delete_empty(ctx: &ApiCtx, path: &str) -> Result<()> {
    let resp = request(ctx, ctx.http.delete(format!("{}{}", ctx.base_url, path)))
        .send()
        .with_context(|| format!("request failed: DELETE {}", path))?;
    let _ = ensure_success(resp, path)?;
    Ok(())
}

fn ensure_success(
    resp: reqwest::blocking::Response,
    path: &str,
) -> Result<reqwest::blocking::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }

    let body = resp.text().unwrap_or_default();
    bail!("request {} failed with status {}: {}", path, status, body)
}

fn validate_schema_file(file: &DocumentSchemaFile) -> Result<()> {
    if file.schema.schema_type != "document" {
        bail!("schema.type must be 'document'");
    }

    if normalize_identifier(&file.schema.id).is_empty() {
        bail!("schema.$id is invalid");
    }

    let mut collection_ids = BTreeSet::new();
    for collection in &file.collections {
        let cid = normalize_identifier(&collection.id);
        if cid.is_empty() {
            bail!("collection $id is invalid");
        }
        if !collection_ids.insert(cid.clone()) {
            bail!("duplicate collection id '{}'", collection.id);
        }

        let mut attrs = BTreeSet::new();
        for attr in &collection.attributes {
            let key = normalize_identifier(&attr.key);
            if key.is_empty() {
                bail!("invalid attribute key in collection '{}'", collection.id);
            }
            if !attrs.insert(key.clone()) {
                bail!(
                    "duplicate attribute '{}' in collection '{}'",
                    attr.key,
                    collection.id
                );
            }
            if attr.attr_type == "enum" && attr.elements.is_empty() {
                bail!(
                    "enum attribute '{}' in '{}' requires elements",
                    attr.key,
                    collection.id
                );
            }
            if attr.required && attr.default.is_some() {
                bail!(
                    "attribute '{}' in '{}' cannot be both required and have default",
                    attr.key,
                    collection.id
                );
            }
        }

        let mut idx_keys = BTreeSet::new();
        for idx in &collection.indexes {
            let key = normalize_identifier(&idx.key);
            if key.is_empty() {
                bail!("invalid index key in collection '{}'", collection.id);
            }
            if !idx_keys.insert(key) {
                bail!(
                    "duplicate index '{}' in collection '{}'",
                    idx.key,
                    collection.id
                );
            }
            if idx.attributes.is_empty() {
                bail!(
                    "index '{}' in '{}' has no attributes",
                    idx.key,
                    collection.id
                );
            }
            for attr in &idx.attributes {
                if !attrs.contains(&normalize_identifier(attr)) {
                    bail!(
                        "index '{}' references unknown attribute '{}' in '{}'",
                        idx.key,
                        attr,
                        collection.id
                    );
                }
            }
            if !idx.orders.is_empty() && idx.orders.len() != idx.attributes.len() {
                bail!(
                    "index '{}' in '{}' has mismatched orders",
                    idx.key,
                    collection.id
                );
            }
        }
    }

    Ok(())
}

fn ensure_index_attributes_exist(collection: &CollectionSpec, attributes: &[String]) -> Result<()> {
    let existing: BTreeSet<String> = collection
        .attributes
        .iter()
        .map(|a| normalize_identifier(&a.key))
        .collect();
    for attr in attributes {
        if !existing.contains(&normalize_identifier(attr)) {
            bail!(
                "attribute '{}' not found in collection '{}'",
                attr,
                collection.id
            );
        }
    }
    Ok(())
}

fn read_schema_file(path: &Path) -> Result<DocumentSchemaFile> {
    if !path.exists() {
        bail!("schema file not found: {}", path.display());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read schema file: {}", path.display()))?;
    let file: DocumentSchemaFile = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse schema file: {}", path.display()))?;
    Ok(file)
}

fn write_schema_file(path: &Path, file: &DocumentSchemaFile) -> Result<()> {
    validate_schema_file(file)?;
    let raw = serde_json::to_string_pretty(file).context("failed to serialize schema JSON")?;
    fs::write(path, format!("{}\n", raw))
        .with_context(|| format!("failed to write schema file: {}", path.display()))
}

fn parse_default_value(input: Option<&str>) -> Result<Option<Value>> {
    let Some(value) = input else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(Some(Value::String(trimmed.to_string()))),
    }
}

fn normalize_identifier(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|v| normalize_identifier(v))
        .filter(|v| !v.is_empty())
        .collect()
}

fn resolve_schemas_dir(project_dir: &Path, custom: Option<&PathBuf>) -> PathBuf {
    match custom {
        Some(v) if v.is_absolute() => v.clone(),
        Some(v) => project_dir.join(v),
        None => project_dir.join(DEFAULT_SCHEMAS_DIR),
    }
}

fn schema_file_path(project_dir: &Path, custom_dir: Option<&PathBuf>, schema_id: &str) -> PathBuf {
    resolve_schemas_dir(project_dir, custom_dir).join(format!("{}.json", schema_id))
}

fn schema_files_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to read directory: {}", dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|v| v.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(files)
}

//! models.dev's files in `data/` (MODELS.md), resolved as models.dev resolves them, into one
//! document in the shape of its v2 API, cut to the fields llx reads, built into the crate. The
//! rules (models.dev `generate.ts` and `generate-v2.ts`): a provider model with a `base_model`
//! is that lab model (less its identity and links) with the provider file deep-merged over it
//! (tables key by key, arrays and values replaced), less each `base_model_omit` path; its
//! modalities are the provider file's when it has them; its APIs are its own `[api.*]` tables,
//! else the provider's default API, the protocol from the SDK package and the address from the
//! table, the provider, or the SDK's default.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    reason = "a build script fails the build: its panic is the error message"
)]

use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

/// models.dev `DEFAULT_NPM_BASE_URLS`, for the packages the copied providers use.
const DEFAULT_BASE_URLS: &[(&str, &str)] = &[
    ("@ai-sdk/openai", "https://api.openai.com/v1"),
    ("@ai-sdk/anthropic", "https://api.anthropic.com/v1"),
    (
        "@ai-sdk/google",
        "https://generativelanguage.googleapis.com/v1beta",
    ),
    ("@ai-sdk/mistral", "https://api.mistral.ai/v1"),
    ("@ai-sdk/cerebras", "https://api.cerebras.ai/v1"),
    ("@ai-sdk/xai", "https://api.x.ai/v1"),
    ("@ai-sdk/groq", "https://api.groq.com/openai/v1"),
    ("@ai-sdk/deepinfra", "https://api.deepinfra.com/v1/openai"),
    ("@ai-sdk/togetherai", "https://api.together.xyz/v1"),
    ("@ai-sdk/perplexity", "https://api.perplexity.ai"),
];

/// What a lab model gives a provider model: not its identity or links.
const NOT_INHERITED: &[&str] = &["id", "benchmarks", "license", "links", "weights"];

fn main() {
    println!("cargo:rerun-if-changed=data");
    let data = Path::new("data");
    let mut providers = Map::new();
    for (id, path) in entries(&data.join("providers")) {
        let provider = read(&path.join("provider.toml"));
        let mut models = Map::new();
        let folder = path.join("models");
        for (model_id, file) in files(&folder, &folder) {
            let raw = read(&file);
            let model = resolve(&model_id, &raw, &provider, &data.join("models"));
            models.insert(model_id, kept(&model));
        }
        let field = |key: &str| provider.get(key).cloned().unwrap_or(Value::Null);
        providers.insert(
            id.clone(),
            json!({"id": id, "name": field("name"), "env": field("env"), "doc": field("doc"),
                "models": models}),
        );
    }
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("catalog.json");
    fs::write(out, Value::Object(providers).to_string()).unwrap();
}

/// A provider model file, resolved.
fn resolve(
    id: &str,
    raw: &Map<String, Value>,
    provider: &Map<String, Value>,
    labs: &Path,
) -> Value {
    let mut own = raw.clone();
    own.remove("schema");
    let base = own.remove("base_model");
    let omit = own.remove("base_model_omit");
    let mut model = match base.as_ref().and_then(Value::as_str) {
        Some(base) => {
            let path = labs.join(format!("{base}.toml"));
            let mut lab = read(&path);
            lab.remove("schema");
            for key in NOT_INHERITED {
                lab.remove(*key);
            }
            let mut model = Value::Object(lab);
            merge(&mut model, &Value::Object(own.clone()));
            model
        }
        None => Value::Object(own.clone()),
    };
    for path in omit
        .iter()
        .flat_map(|o| o.as_array().cloned().unwrap_or_default())
    {
        remove(&mut model, path.as_str().unwrap_or(""));
    }
    if let Some(modalities) = own.get("modalities") {
        model["modalities"] = modalities.clone();
    }
    model["id"] = json!(id);
    if let Some(base) = base {
        model["canonical_id"] = base;
    }
    if model.get("type").is_none() {
        model["type"] = json!("chat");
    }
    model["api"] = apis(own.get("api"), provider);
    // The mid-conversation flags are published as true or false: unset is false.
    if let Some(capabilities) = model.get_mut("capabilities").and_then(Value::as_object_mut) {
        for flag in ["system_updates", "effort_updates"] {
            capabilities.entry(flag).or_insert(json!(false));
        }
    }
    model
}

/// models.dev's merge: tables key by key, everything else replaced.
fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                match base.get_mut(key) {
                    Some(slot @ Value::Object(_)) if value.is_object() => merge(slot, value),
                    _ => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, over) => *base = over.clone(),
    }
}

/// Drop a dotted path (`limit.input`).
fn remove(model: &mut Value, path: &str) {
    let mut parts: Vec<&str> = path.split('.').collect();
    let Some(last) = parts.pop() else {
        return;
    };
    let mut at = model;
    for part in parts {
        match at.get_mut(part) {
            Some(next) => at = next,
            None => return,
        }
    }
    if let Some(map) = at.as_object_mut() {
        map.remove(last);
    }
}

/// A model's APIs: its own tables, or the provider's default API.
fn apis(tables: Option<&Value>, provider: &Map<String, Value>) -> Value {
    let npm = provider.get("npm").and_then(Value::as_str).unwrap_or("");
    let address = |table: &Value, sdk: &str| {
        let url = table
            .get("base_url")
            .and_then(Value::as_str)
            .or_else(|| provider.get("api").and_then(Value::as_str))
            .or_else(|| {
                DEFAULT_BASE_URLS
                    .iter()
                    .find(|(p, _)| *p == sdk)
                    .map(|(_, u)| *u)
            })
            .unwrap_or("");
        url.trim_end_matches('/').to_string()
    };
    let mut out = Map::new();
    match tables.and_then(Value::as_object) {
        Some(tables) => {
            for (protocol, table) in tables {
                let sdk = table.pointer("/sdk/aisdk").and_then(Value::as_str);
                let sdk = sdk.filter(|s| *s != "none").unwrap_or(npm);
                let mut entry = json!({"base_url": address(table, sdk)});
                if let Some(compatibility) = table.get("compatibility") {
                    entry["compatibility"] = compatibility.clone();
                }
                out.insert(protocol.clone(), entry);
            }
        }
        None => {
            out.insert(
                protocol(npm).into(),
                json!({"base_url": address(&Value::Null, npm)}),
            );
        }
    }
    Value::Object(out)
}

/// models.dev `resolveProtocol`, for a chat model.
fn protocol(npm: &str) -> &'static str {
    match npm {
        "@ai-sdk/anthropic" => "messages",
        "@ai-sdk/openai" | "@ai-sdk/azure" | "@ai-sdk/xai" => "responses",
        "@ai-sdk/google" => "generate-content",
        _ => "chat-completions",
    }
}

/// The fields llx reads, nothing else: the binary carries no more than it uses.
fn kept(model: &Value) -> Value {
    let mut out = json!({});
    for path in [
        "/id",
        "/canonical_id",
        "/name",
        "/type",
        "/release_date",
        "/status",
        "/modalities/output",
        "/capabilities/tools/supported",
        "/capabilities/reasoning",
        "/capabilities/cache",
        "/capabilities/system_updates",
        "/capabilities/effort_updates",
        "/limit/context",
        "/limit/input",
        "/limit/output",
        "/cost/text/input/price",
        "/cost/text/input/cache/read",
        "/cost/text/input/overrides",
        "/cost/text/output/price",
        "/cost/text/output/overrides",
        "/api",
    ] {
        if let Some(value) = model.pointer(path) {
            set(&mut out, path, value.clone());
        }
    }
    out
}

fn set(out: &mut Value, path: &str, value: Value) {
    let mut at = out;
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    for part in &parts[..parts.len() - 1] {
        at = at
            .as_object_mut()
            .unwrap()
            .entry(*part)
            .or_insert_with(|| json!({}));
    }
    at[parts[parts.len() - 1]] = value;
}

/// The folders under `dir`, by name.
fn entries(dir: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out: Vec<_> = fs::read_dir(dir)
        .expect("data/providers")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            )
        })
        .collect();
    out.sort();
    out
}

/// Every TOML file under `dir`, its id the path from `base` without `.toml`.
fn files(base: &Path, dir: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out = vec![];
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files(base, &path));
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            let id = path.strip_prefix(base).unwrap().with_extension("");
            out.push((id.to_string_lossy().replace('\\', "/"), path));
        }
    }
    out.sort();
    out
}

fn read(path: &Path) -> Map<String, Value> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let table: toml::Table =
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    match serde_json::to_value(table).unwrap() {
        Value::Object(map) => map,
        _ => unreachable!("a TOML document is a table"),
    }
}

use chrono::{DateTime, Utc};
use humantime::parse_duration;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use rocket::{Request, catch, catchers, get, launch, routes};
use std::cmp::Reverse;
use std::ffi::OsStr;
use std::fs::{DirEntry, read_dir, read_to_string};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/api", routes![index, lists, entry])
        .register("/", catchers![default_catcher])
}

fn get_entries(path: impl AsRef<Path>) -> Result<Vec<Entry>, Status> {
    let mut entries: Vec<_> = read_dir(path)
        .map_err(|_| Status::InternalServerError)?
        .flatten()
        .filter_map(get_json)
        .filter(|e| !e.hidden.unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| Reverse(e.created_at));
    Ok(entries)
}

fn get_json(entry: DirEntry) -> Option<Entry> {
    let path = entry.path();
    if path.extension() == Some(OsStr::new("json"))
        && path
            .file_stem()
            .and_then(OsStr::to_str)
            .is_some_and(|s| !s.contains('.'))
    {
        get_entry(path)
    } else {
        None
    }
}

fn get_entry(path: impl AsRef<Path>) -> Option<Entry> {
    serde_json::from_str::<Entry>(&read_to_string(path).ok()?).ok()
}

#[get("/")]
fn index() -> Json<[&'static str; 4]> {
    Json(["vods", "highlights", "clips", "rplay"])
}

#[get("/<kind>")]
fn lists(kind: &str) -> Result<Json<Vec<Entry>>, Status> {
    get_entries(kind).map(Json)
}

#[get("/<kind>/<id>")]
fn entry(kind: &str, id: &str) -> Result<Json<Entry>, Status> {
    let path = PathBuf::from(kind).join(id).with_extension("json");
    if path.exists() {
        get_entry(path).ok_or(Status::InternalServerError).map(Json)
    } else {
        Err(Status::NotFound)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Entry {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    created_at: DateTime<Utc>,
    #[serde(
        deserialize_with = "parse_duration_flex",
        serialize_with = "serialize_duration"
    )]
    duration: Duration,
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden: Option<bool>,
}

fn parse_duration_flex<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(crate = "rocket::serde")]
    #[serde(untagged)]
    enum Either {
        Float(f64),
        String(String),
    }

    match Either::deserialize(deserializer)? {
        Either::Float(secs) => Ok(Duration::from_secs_f64(secs)),
        Either::String(s) => {
            parse_duration(&s).map_err(|_| de::Error::custom("invalid duration string"))
        }
    }
}

fn serialize_duration<S>(dur: &Duration, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let secs = dur.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let s = if h > 0 {
        format!("{h}h{m}m{s}s")
    } else if m > 0 {
        format!("{m}m{s}s")
    } else {
        format!("{s}s")
    };
    ser.serialize_str(&s)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ErrorStatus {
    error: u16,
}

#[catch(default)]
fn default_catcher(status: Status, _: &Request) -> Json<ErrorStatus> {
    Json(ErrorStatus { error: status.code })
}

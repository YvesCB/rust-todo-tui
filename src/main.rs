use std::collections::BTreeMap;
use surrealdb::sql::{self, thing, Datetime, Object, Thing, Value};
use surrealdb::{Datastore, Response, Session};
enum Status {
    New,
    Started,
    Completed,
}

#[derive(Clone)]
struct Todo {
    title: String,
    created_at: DateTime<Utc>,
    status: Status,
}

impl Todo {
    fn new(title: &str) -> Todo {
        Todo {
            title: title,
            created_at: Datetime.default(),
            status: Status::New,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()>{
    println!("Hello, world!");

    Ok(())
}

use std::collections::BTreeMap;
use anyhow::{anyhow, Result};
use surrealdb::sql::{self, thing, Datetime, Object, Thing, Value};
use surrealdb::{Datastore, Response, Session};

type DB = (Datastore, Session);

#[derive(Clone)]
enum Status {
    New,
    Started,
    Completed,
}

#[derive(Clone)]
struct Todo {
    title: String,
    created_at: Datetime,
    status: Status,
}

impl Todo {
    fn new(title: &str) -> Todo {
        Todo {
            title: title.to_string(),
            created_at: Datetime::default(),
            status: Status::New,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()>{
    let db: &DB = &(Datastore::new("memory").await?, Session::for_db("my_ns", "task_db"));
    let (ds, ses) = db;

    let new_task = Todo::new("First task");
    let t1 = create_new_task(db, new_task).await?;
    let new_task = Todo::new("Second task");
    let t2 = create_new_task(db, new_task).await?;

    let sql = "SELECT * FROM todo";
    let ress = ds.execute(sql, ses, None, false).await?; 

    for obj in into_iter_objects(ress)? {
        println!("{}", obj?);
    }

    let response = change_task_status(db, t1, Status::Completed).await?;
    println!("changed status of {}", response);

    let ress = ds.execute(sql, ses, None, false).await?; 
    for obj in into_iter_objects(ress)? {
        println!("{}", obj?);
    }

    Ok(())
}

async fn create_new_task((ds, ses): &DB, task: Todo) -> Result<String> {
    let sql = "CREATE todo CONTENT $data";

    let data: BTreeMap<String, Value> = [
        ("title".into(), task.title.into()),
        ("created_at".into(), task.created_at.into()),
        ("status".into(), match task.status {
            Status::Completed => "completed".into(),
            Status::New => "new".into(),
            Status::Started => "started".into(),
        }),
    ]
    .into();

    let vars: BTreeMap<String, Value> = [("data".into(), data.into())].into();

    let ress = ds.execute(sql, ses, Some(vars), false).await?;

    into_iter_objects(ress)?
        .next()
        .transpose()?
        .and_then(|obj| obj.get("id").map(|id| id.to_string()))
        .ok_or_else(|| anyhow!("No id returned"))
}

async fn change_task_status((ds, ses): &DB, task_id: String, new_status: Status) -> Result<String> {
    let sql = "UPDATE $th MERGE $data RETURN id";

    let data: BTreeMap<String, Value> = [
        ("status".into(), match new_status {
            Status::Completed => "completed".into(),
            Status::New => "new".into(),
            Status::Started => "started".into(),
        }),
    ]
    .into();

    let vars: BTreeMap<String, Value> = [
        ("th".into(), thing(&task_id)?.into()),
        ("data".into(), data.into()),
    ]
    .into();

    let ress = ds.execute(sql, ses, Some(vars), true).await?;

    into_iter_objects(ress)?
        .next()
        .transpose()?
        .and_then(|obj| obj.get("id").map(|id| id.to_string()))
        .ok_or_else(|| anyhow!("No id returned"))
}

fn into_iter_objects(ress: Vec<Response>) -> Result<impl Iterator<Item = Result<Object>>> {
    let res = ress.into_iter().next().map(|rp| rp.result).transpose()?;

    match res {
        Some(Value::Array(arr)) => {
            let it = arr.into_iter().map(|v| match v {
                Value::Object(object) => Ok(object),
                _ => Err(anyhow!("A records was not an object")),
            });
            Ok(it)
        }
        _ => Err(anyhow!("No records found.")),
    }
}
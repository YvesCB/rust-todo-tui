use anyhow::{anyhow, Result};
use crossterm::{event::{EnableMouseCapture, DisableMouseCapture, self}, execute, terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen}};
use tokio::sync::mpsc;
use std::{thread, time::{Duration, Instant}, collections::BTreeMap, io};
use surrealdb::sql::{self, thing, Datetime, Object, Thing, Value};
use surrealdb::{Datastore, Response, Session};
use tui::{backend::{CrosstermBackend, Backend}, widgets::{Block, Borders, Paragraph, BorderType, Tabs}, Terminal, Frame, layout::{Layout, Direction, Constraint, Alignment}, style::{Style, Color, Modifier}, text::{Span, Spans}};

type DB = (Datastore, Session);

#[derive(Clone)]
enum Status {
    New,
    Started,
    Completed,
}

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Todo,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Todo => 1,
        }
    }
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
async fn main() -> Result<()> {
    let db: &DB = &(
        Datastore::new("memory").await?,
        Session::for_db("my_ns", "task_db"),
    );
    let (ds, ses) = db;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let (tx, rx) = mpsc::channel(32);
    let tick_rate = Duration::from_millis(200);

    tokio::spawn(async move {
        let mut last_tick = Instant::now();
        loop {

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                match event::read().expect("can't read") {
                    event::Event::Key(key) => {
                        tx.send(Event::Input(key)).await;
                    },
                    _ => {}
                }
                if let event::Event::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).await;
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick).await {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let menu_titles = vec!["Home", "Todo", "Add", "Delete", "Quit"];
    let mut active_menu_item = MenuItem::Home;

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(2),
                        Constraint::Length(3)
                    ]
                    .as_ref(),
                )
                .split(size);

            let copyright = Paragraph::new("todo-tui - all rights reserved")
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
                        .title("Copyright")
                        .border_type(BorderType::Plain),
                );
            
            rect.render_widget(copyright, chunks[2]);

            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first, 
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            rect.render_widget(tabs, chunks[0]);
        }).expect("shit's fucked");
    }

    // let new_task = Todo::new("First task");
    // let t1 = create_new_task(db, new_task).await?;
    // let new_task = Todo::new("Second task");
    // let t2 = create_new_task(db, new_task).await?;

    // let sql = "SELECT * FROM todo";
    // let ress = ds.execute(sql, ses, None, false).await?;

    // for obj in into_iter_objects(ress)? {
    //     println!("{}", obj?);
    // }

    // let response = change_task_status(db, t1, Status::Completed).await?;
    // println!("changed status of {}", response);

    // let ress = ds.execute(sql, ses, None, false).await?;
    // for obj in into_iter_objects(ress)? {
    //     println!("{}", obj?);
    // }

    // let response = delete_task(db, t2).await?;
    // println!("Deleted task");

    // let ress = ds.execute(sql, ses, None, false).await?;
    // for obj in into_iter_objects(ress)? {
    //     println!("{}", obj?);
    // }

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10)
            ].as_ref()
        )
        .split(f.size());

        let block = Block::default()
            .title("Block")
            .borders(Borders::ALL);
        f.render_widget(block, chunks[0]);
        let block = Block::default()
            .title("Block2")
            .borders(Borders::ALL);
        f.render_widget(block, chunks[1]);
}

async fn create_new_task((ds, ses): &DB, task: Todo) -> Result<String> {
    let sql = "CREATE todo CONTENT $data";

    let data: BTreeMap<String, Value> = [
        ("title".into(), task.title.into()),
        ("created_at".into(), task.created_at.into()),
        (
            "status".into(),
            match task.status {
                Status::Completed => "completed".into(),
                Status::New => "new".into(),
                Status::Started => "started".into(),
            },
        ),
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

    let data: BTreeMap<String, Value> = [(
        "status".into(),
        match new_status {
            Status::Completed => "completed".into(),
            Status::New => "new".into(),
            Status::Started => "started".into(),
        },
    )]
    .into();

    let vars: BTreeMap<String, Value> = [
        ("th".into(), thing(&task_id)?.into()),
        ("data".into(), data.into()),
    ]
    .into();

    let ress = ds.execute(sql, ses, Some(vars), true).await?;
    println!("{:?}", ress);

    into_iter_objects(ress)?
        .next()
        .transpose()?
        .and_then(|obj| obj.get("id").map(|id| id.to_string()))
        .ok_or_else(|| anyhow!("No id returned"))
}

async fn delete_task((ds, ses): &DB, task_id: String) -> Result<()> {
    let sql = "DELETE $th";

    let vars: BTreeMap<String, Value> = [("th".into(), thing(&task_id)?.into())].into();

    let ress = ds.execute(sql, ses, Some(vars), true).await?;
    println!("{:?}", &ress);

    Ok(())
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

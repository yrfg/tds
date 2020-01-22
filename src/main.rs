use chrono::{DateTime, Utc};
use colored::*;
use dotenv::dotenv;
use postgres::{Client, NoTls};
use std::collections::VecDeque;
use std::env;

extern crate colored;
extern crate dotenv;

mod draw;
use draw::sbui::*;

mod time;

#[derive(Debug, Copy, Clone)]
enum ToDoState {
    NextUp,
    InProgress,
    Review,
    Completed,
    Closed,
    Unknown,
}

impl From<i32> for ToDoState {
    fn from(n: i32) -> Self {
        match n {
            0 => ToDoState::NextUp,
            1 => ToDoState::InProgress,
            2 => ToDoState::Review,
            3 => ToDoState::Completed,
            4 => ToDoState::Closed,
            _ => ToDoState::Unknown,
        }
    }
}

impl From<&str> for ToDoState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "n" | "next" => ToDoState::NextUp,
            "p" | "progress" => ToDoState::InProgress,
            "r" | "review" => ToDoState::Review,
            "c" | "cp" | "complete" | "completed" => ToDoState::Completed,
            "cl" | "close" | "closed" => ToDoState::Closed,
            _ => ToDoState::Unknown,
        }
    }
}

impl Into<Option<i32>> for ToDoState {
    fn into(self) -> Option<i32> {
        match self {
            ToDoState::NextUp => Some(0),
            ToDoState::InProgress => Some(1),
            ToDoState::Review => Some(2),
            ToDoState::Completed => Some(3),
            ToDoState::Closed => Some(4),
            ToDoState::Unknown => None,
        }
    }
}

impl std::fmt::Display for ToDoState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ToDoState::NextUp => "Next Up",
            ToDoState::InProgress => "In Progress",
            ToDoState::Review => "Review",
            ToDoState::Completed => "Completed",
            ToDoState::Closed => "Closed",
            _ => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug)]
struct Todo {
    id: i32,
    title: String,
    content: String,
    author: String,
    last_edit_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    due_at: Option<DateTime<Utc>>,
    state: ToDoState,
}

#[derive(Debug)]
struct TodoDisplay {
    id: i32,
    title: String,
    due: String,
    run: String,
    state: ToDoState,
}

#[derive(Debug)]
struct CreateTodo {
    title: String,
    author: String,
    due_at: Option<DateTime<Utc>>,
}

enum Command {
    InitStore,
    List,
    Inspect(i32),
    Add(CreateTodo),
    Set(i32, ToDoState),
    Del(i32),
    Visual,
    Help,
}

impl Command {
    fn new_from_args() -> Self {
        let mut args: VecDeque<String> = env::args().skip(1).collect();
        match args.len() {
            0 => Command::List,
            i => {
                let rawcmd = args.pop_front().unwrap().to_lowercase().clone();
                let i = i - 1;
                match (rawcmd.as_str(), i) {
                    // example: --init
                    ("--init", _) => Command::InitStore,
                    // example: l
                    ("l", 0) | ("--list", 0) => Command::List,
                    // example: a jog
                    ("a", 1) | ("--add", 1) => Command::Add(CreateTodo {
                        title: args[0].clone(),
                        author: String::from("yiranfeng"),
                        due_at: None,
                    }),
                    // example: a jog 12h
                    ("a", 2) | ("--add", 2) => {
                        let due = time::get_time_by_str(args[1].as_str());
                        Command::Add(CreateTodo {
                            title: args[0].clone(),
                            author: String::from("yiranfeng"),
                            due_at: due,
                        })
                    }
                    // example: s 2 c
                    ("s", 2) | ("--set", 2) => {
                        if let Ok(id) = args[0].parse() {
                            return Command::Set(id, ToDoState::from(args[1].as_str()));
                        }
                        Command::Help
                    }
                    // example: d 2
                    ("d", 1) | ("--del", 1) => {
                        if let Ok(id) = args[0].parse() {
                            return Command::Del(id);
                        }
                        Command::Help
                    }
                    ("i", 1) | ("--inspect", 1) => {
                        if let Ok(id) = args[0].parse() {
                            return Command::Inspect(id);
                        }
                        Command::Help
                    }
                    ("v", 0) | ("--visual", 0) => Command::Visual,
                    ("h", 0) | ("--help", 0) => Command::Help,
                    _ => Command::Help,
                }
            }
        }
    }

    fn run(&self, c: &mut Client) -> Result<(), String> {
        const TABLE_TODO_NAME: &str = "iiran_todo";
        match self {
            Command::InitStore => init_todo_table(c, TABLE_TODO_NAME),
            Command::List => list_todo(c, TABLE_TODO_NAME),
            Command::Inspect(target) => print_todo_detail(c, TABLE_TODO_NAME, *target),
            Command::Add(todo_mould) => create_todo(c, TABLE_TODO_NAME, todo_mould),
            Command::Set(target, state) => update_todo_state(c, TABLE_TODO_NAME, *target, *state),
            Command::Del(target) => delete_todo(c, TABLE_TODO_NAME, *target),
            Command::Visual => enter_visual(),
            Command::Help => print_help(),
        }
    }

    fn is_write_cmd(&self) -> bool {
        match self {
            // ignore initstore
            Command::Add(_) | Command::Del(_) | Command::Set(_, _) => true,
            _ => false,
        }
    }
}

fn enter_visual() -> Result<(), String> {
    println!("enter visual mode");
    Ok(())
}

fn delete_todo(c: &mut Client, table_name: &str, id: i32) -> Result<(), String> {
    let sql = format!(
        r#"
    UPDATE {0}
    SET deleted_at = $1
    WHERE id = $2
    "#,
        table_name
    );
    let now = Utc::now();
    let q_res = c.execute(sql.as_str(), &[&now, &id]);
    match q_res {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn update_todo_state(
    c: &mut Client,
    table_name: &str,
    id: i32,
    state: ToDoState,
) -> Result<(), String> {
    let sql = format!(
        r#"
    UPDATE {0} 
    SET state = $1 
    WHERE id = $2;
    "#,
        table_name
    );
    let q_res = c.execute(sql.as_str(), &[&(state as i32), &id]);
    match q_res {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn print_todo_detail(c: &mut Client, table_name: &str, id: i32) -> Result<(), String> {
    let sql = format!(
        r#"
    SELECT id, title, content, author, last_edit_by, created_at, updated_at, state, weight, premise
    FROM {0}
    WHERE deleted_at IS NULL AND ID = $1
    LIMIT 1
    "#,
        table_name
    );
    let q_res = c.query(sql.as_str(), &[&id]);
    match q_res {
        Ok(rows) => {
            // TODO: Print ToDo detail
            rows.iter().for_each(|row| {
                let title: String = row.get(1);
                println!("{}", title.as_str());
            });
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn init_todo_table(c: &mut Client, table_name: &str) -> Result<(), String> {
    let q_res = c.simple_query(
        format!(
            r#"
        CREATE TABLE IF NOT EXISTS {0}  (
            id            SERIAL         PRIMARY KEY     NOT NULL                                    ,
            title         TEXT                           NOT NULL                                    ,
            content       TEXT                           NOT NULL  DEFAULT   ''                      ,
            author        VARCHAR(128)                   NOT NULL  DEFAULT   ''                      , 
            last_edit_by  VARCHAR(128)                   NOT NULL  DEFAULT   ''                      ,
            created_at    TIMESTAMPTZ                    NOT NULL  DEFAULT   now()                   ,
            updated_at    TIMESTAMPTZ                    NOT NULL  DEFAULT   now()                   ,
            deleted_at    TIMESTAMPTZ                              DEFAULT   NULL                    ,
            due_at        TIMESTAMPTZ                              DEFAULT   NULL                    ,
            state         INT                            NOT NULL  DEFAULT   0                       ,
            weight        INT                            NOT NULL  DEFAULT   1                       ,  
            premise       INT                                      DEFAULT   NULL                     
        );
        CREATE INDEX  IF NOT EXISTS state_index  ON {0}(state);
        CREATE INDEX  IF NOT EXISTS due_index    ON {0}(due_at);
       "#,
            table_name
        )
        .as_str(),
    );
    match &q_res {
        Ok(_) => Ok(()),
        Err(r) => Err(r.to_string()),
    }
}

fn get_db_info_from_env() -> (String, String, String, String) {
    const ENV_HOST: &str = "IIRAN_TODO_DB_HOST";
    const ENV_PORT: &str = "IIRAN_TODO_DB_PORT";
    const ENV_USER: &str = "IIRAN_TODO_DB_USER";
    const ENV_PW: &str = "IIRAN_TODO_DB_PASSWORD";
    let host = env::var(ENV_HOST).ok().unwrap_or_default();
    let port = env::var(ENV_PORT).ok().unwrap_or_default();
    let user = env::var(ENV_USER).ok().unwrap_or_default();
    let password = env::var(ENV_PW).ok().unwrap_or_default();
    (host, port, user, password)
}

fn main() {
    let cmd = Command::new_from_args();

    dotenv().ok();
    let (host, port, user, password) = get_db_info_from_env();
    let mut client = Client::connect(
        format!(
            "host={} port={} user={} password={}",
            host, port, user, password
        )
        .as_str(),
        NoTls,
    )
    .expect("connect error");

    if let Err(e) = cmd.run(&mut client) {
        println!("ERROR: {}", e)
    } else if cmd.is_write_cmd() {
        Command::List.run(&mut client).unwrap();
        ()
    }
}

fn print_help() -> Result<(), String> {
    const HELP_MAN: &str = r#"
    tds 0.1.0
    A tool to manage to-do items.

    USAGE:
    tds [COMMAND] [OPTION]

    COMMAND:
    l --list         List all todo status.
    i --inspect      Check todo.
    a --add          Create new todo.
    s --set          Update todo status.
    d --del          Delete todo.
    v --visual       Visual Mode
    "#;
    println!("{}", HELP_MAN);
    Ok(())
}

fn create_todo(c: &mut Client, table_name: &str, todo: &CreateTodo) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO {} (title, author, last_edit_by, due_at) VALUES ($1, $2, $3, $4)",
        table_name
    );

    match c.execute(
        sql.as_str(),
        &[&todo.title, &todo.author, &todo.author, &todo.due_at],
    ) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn list_todo(c: &mut Client, table_name: &str) -> Result<(), String> {
    let sql = format!(
        r#"
        SELECT id, title, content, author, last_edit_by, created_at, updated_at, due_at, state 
        FROM {} 
        WHERE deleted_at IS NULL
        ORDER BY state ASC
        "#,
        table_name
    );

    let now = Utc::now();

    match c.query(sql.as_str(), &[]) {
        Err(e) => Err(e.to_string()),
        Ok(rs) => Ok({
            let mut next = Vec::new();
            let mut inprog = Vec::new();
            let mut review = Vec::new();
            let mut complete = Vec::new();
            let mut closed = Vec::new();

            for r in rs {
                let mut title: String = r.get(1);
                filter_wid_char!(title);
                let state: i32 = r.get(8);
                let due_at: Option<DateTime<Utc>> = r.get(7);
                let created_at: DateTime<Utc> = r.get(5);

                let todo = TodoDisplay {
                    id: r.get(0),
                    title: title,
                    due: get_todo_due_str(due_at, now),
                    run: time::get_str_by_time(created_at, now),
                    state: ToDoState::from(state),
                };

                match todo.state {
                    ToDoState::NextUp => next.push(todo),
                    ToDoState::InProgress => inprog.push(todo),
                    ToDoState::Review => review.push(todo),
                    ToDoState::Completed => complete.push(todo),
                    ToDoState::Closed => closed.push(todo),
                    _ => (),
                };
            }

            let mut win = SymbolWindow::new(70);
            win.add_tag(&["TASK", "DUE", "RUN", "ID"]);
            win.change_weight("TASK", |w| 3.0 * w);
            win.change_weight("ID", |w| 0.5 * w);
            win.refresh();
            print_title!(win);
            print_div!(win, format!("{}({})", ToDoState::NextUp, next.len()), green);
            print_rows!(win,next, &[title; due; run; id]);
            print_div!(
                win,
                format!("{}({})", ToDoState::InProgress, inprog.len()),
                yellow
            );
            print_rows!(win,inprog, &[title; due; run; id]);
            print_div!(
                win,
                format!("{}({})", ToDoState::Review, review.len()),
                purple
            );
            print_rows!(win,review, &[title; due; run; id]);
            print_foot!(win);
        }),
    }
}

fn get_todo_due_str(due: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    match due {
        Some(due_at) => {
            if due_at < now {
                String::from("TIMEOUT")
            } else {
                time::get_str_by_time(due_at, now)
            }
        }
        None => String::from("-")
    }
}

//// # example
//// ```rust
//// let mut s = String::from("你好啊!");
//// filter_wid_char!(s);
//// assert_eq!(s, "!");
#[macro_export]
macro_rules! filter_wid_char {
    ($s:ident) => {
        $s = $s.chars()
        .map(|c| if c.len_utf8() == 1 { c } else { '*' })
        .collect();
    };
}

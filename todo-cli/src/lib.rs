use anyhow::{Context, bail};
use chrono::NaiveDate;
use clap::Subcommand;
use todo_core::{Database, STATE_LAST_WORKSPACE_ID, Task, Workspace};

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Add a task
    Add {
        /// Task title
        #[arg(required = true, num_args = 1..)]
        title: Vec<String>,
        /// Workspace name or id (defaults to the last workspace used)
        #[arg(short, long)]
        workspace: Option<String>,
        /// Add as a subtask of this task id
        #[arg(short, long)]
        parent: Option<i64>,
        /// Due date (YYYY-MM-DD)
        #[arg(short, long)]
        due: Option<String>,
    },
    /// List tasks
    #[command(alias = "ls")]
    List {
        /// Workspace name or id (defaults to the last workspace used)
        #[arg(short, long, conflicts_with = "all")]
        workspace: Option<String>,
        /// List tasks in every workspace
        #[arg(short, long)]
        all: bool,
    },
    /// Mark tasks as completed
    Done {
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Mark tasks as not completed
    Undone {
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Delete tasks
    Rm {
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// List or create workspaces
    #[command(alias = "ws")]
    Workspaces {
        #[command(subcommand)]
        action: Option<WorkspaceCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Create a workspace
    Add { name: String },
}

pub async fn run(db: &Database, command: CliCommand) -> anyhow::Result<()> {
    match command {
        CliCommand::Add {
            title,
            workspace,
            parent,
            due,
        } => {
            let title = title.join(" ");
            let due = due.as_deref().map(parse_due_date).transpose()?;

            let id = if let Some(parent_id) = parent {
                let parent = find_task(db, parent_id).await?;
                if let Some(name) = &workspace {
                    let ws = resolve_workspace(db, Some(name)).await?;
                    if ws.id != parent.workspace_id {
                        bail!("task {parent_id} is not in workspace '{}'", ws.name);
                    }
                }
                db.create_subtask(&title, parent.workspace_id, parent_id)
                    .await?
            } else {
                let ws = resolve_workspace(db, workspace.as_deref()).await?;
                db.create_task(&title, ws.id).await?
            };

            if let Some(due) = &due {
                db.update_task_due_date(id, Some(due)).await?;
            }
            println!("added {id}: {title}");
        }
        CliCommand::List { workspace, all } => {
            let workspaces = if all {
                db.get_workspaces().await?
            } else {
                vec![resolve_workspace(db, workspace.as_deref()).await?]
            };
            for (i, ws) in workspaces.iter().enumerate() {
                if all {
                    if i > 0 {
                        println!();
                    }
                    println!("{}", ws.name);
                }
                let tasks = db.get_tasks_for_workspace(ws.id).await?;
                if tasks.is_empty() {
                    println!("  (no tasks)");
                }
                print_tasks(&tasks);
            }
        }
        CliCommand::Done { ids } => set_completed(db, &ids, true).await?,
        CliCommand::Undone { ids } => set_completed(db, &ids, false).await?,
        CliCommand::Rm { ids } => {
            for id in ids {
                let task = find_task(db, id).await?;
                db.delete_task(id).await?;
                println!("deleted {id}: {}", task.title);
            }
        }
        CliCommand::Workspaces { action } => match action {
            None => {
                for ws in db.get_workspaces().await? {
                    println!("{:>4}  {}", ws.id, ws.name);
                }
            }
            Some(WorkspaceCommand::Add { name }) => {
                let name = name.trim();
                if name.is_empty() {
                    bail!("workspace name cannot be empty");
                }
                let id = db.create_workspace(name).await?;
                println!("created workspace {id}: {name}");
            }
        },
    }

    Ok(())
}

async fn set_completed(db: &Database, ids: &[i64], completed: bool) -> anyhow::Result<()> {
    for &id in ids {
        let task = find_task(db, id).await?;
        db.set_task_completed(id, completed).await?;
        let mark = if completed { "x" } else { " " };
        println!("[{mark}] {id}: {}", task.title);
    }
    Ok(())
}

async fn find_task(db: &Database, id: i64) -> anyhow::Result<Task> {
    db.get_task(id)
        .await?
        .with_context(|| format!("no task with id {id}"))
}

/// Looks up a workspace by id or (case-insensitive) name. Without a query,
/// falls back to the workspace last used in the TUI, then to the only one.
async fn resolve_workspace(db: &Database, query: Option<&str>) -> anyhow::Result<Workspace> {
    let workspaces = db.get_workspaces().await?;
    if workspaces.is_empty() {
        bail!("no workspaces yet, create one with `todo workspaces add <name>`");
    }

    if let Some(query) = query {
        let by_id = query
            .parse::<i64>()
            .ok()
            .and_then(|id| workspaces.iter().find(|w| w.id == id));
        let by_name = || {
            workspaces
                .iter()
                .find(|w| w.name.eq_ignore_ascii_case(query))
        };
        return by_id
            .or_else(by_name)
            .cloned()
            .with_context(|| format!("no workspace named '{query}'"));
    }

    let last_id = db
        .get_state(STATE_LAST_WORKSPACE_ID)
        .await?
        .and_then(|id| id.parse::<i64>().ok());
    if let Some(ws) = last_id.and_then(|id| workspaces.iter().find(|w| w.id == id)) {
        return Ok(ws.clone());
    }
    if let [only] = workspaces.as_slice() {
        return Ok(only.clone());
    }
    bail!("multiple workspaces, pick one with --workspace")
}

fn parse_due_date(input: &str) -> anyhow::Result<String> {
    let date = NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d")
        .with_context(|| format!("invalid due date '{input}', expected YYYY-MM-DD"))?;
    Ok(date.format("%Y-%m-%d").to_string())
}

fn print_tasks(tasks: &[Task]) {
    let is_root = |t: &Task| {
        t.parent_task_id
            .is_none_or(|parent| !tasks.iter().any(|p| p.id == parent))
    };
    for task in tasks.iter().filter(|t| is_root(t)) {
        print_task_tree(tasks, task, 0);
    }
}

fn print_task_tree(tasks: &[Task], task: &Task, level: usize) {
    let indent = "  ".repeat(level + 1);
    let mark = if task.completed { "x" } else { " " };
    let due = task
        .due_date
        .as_ref()
        .map(|d| format!(" (due {d})"))
        .unwrap_or_default();
    println!("{indent}[{mark}] {:>4}  {}{due}", task.id, task.title);

    for child in tasks.iter().filter(|t| t.parent_task_id == Some(task.id)) {
        print_task_tree(tasks, child, level + 1);
    }
}

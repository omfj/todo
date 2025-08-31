use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;

use todo_core::{Database, Task, Workspace};

#[derive(Debug, Clone)]
pub struct TaskDisplay {
    pub task: Task,
    pub level: usize,
    pub index: usize,
}

#[derive(PartialEq)]
pub enum Focus {
    Workspaces,
    Tasks,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
    DeleteConfirm,
    Help,
    Creating,
}

pub struct App {
    pub workspaces: Vec<Workspace>,
    pub tasks: Vec<Task>,
    pub task_displays: Vec<TaskDisplay>,
    pub workspace_state: ListState,
    pub task_state: ListState,
    pub selected_workspace: Option<usize>,
    pub db: Database,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub delete_target: Option<String>,
    pub creating_subtask: bool,
}

impl App {
    pub fn new(db: Database) -> Self {
        let mut workspace_state = ListState::default();
        workspace_state.select(Some(0));

        Self {
            workspaces: vec![],
            tasks: vec![],
            task_displays: vec![],
            workspace_state,
            task_state: ListState::default(),
            selected_workspace: Some(0),
            db,
            focus: Focus::Workspaces,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            delete_target: None,
            creating_subtask: false,
        }
    }

    pub async fn load_workspaces(&mut self) -> Result<()> {
        self.workspaces = self.db.get_workspaces().await?;
        if !self.workspaces.is_empty() {
            self.workspace_state.select(Some(0));
            self.selected_workspace = Some(0);
            self.load_tasks_for_selected_workspace().await?;
        }
        Ok(())
    }

    pub async fn load_tasks_for_selected_workspace(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_workspace {
            if let Some(workspace) = self.workspaces.get(selected) {
                self.tasks = self.db.get_tasks_for_workspace(workspace.id).await?;
                self.build_task_hierarchy();
                self.task_state.select(if self.task_displays.is_empty() {
                    None
                } else {
                    Some(0)
                });
            }
        }
        Ok(())
    }

    fn build_task_hierarchy(&mut self) {
        self.task_displays.clear();
        let tasks = self.tasks.clone();

        let mut incomplete_root_tasks: Vec<Task> = tasks
            .iter()
            .filter(|t| !t.completed && t.parent_task_id.is_none())
            .cloned()
            .collect();
        incomplete_root_tasks.sort_by_key(|t| t.created_at);

        let mut completed_root_tasks: Vec<Task> = tasks
            .iter()
            .filter(|t| t.completed && t.parent_task_id.is_none())
            .cloned()
            .collect();
        completed_root_tasks.sort_by_key(|t| t.created_at);

        let mut index = 0;

        for task in incomplete_root_tasks {
            self.add_task_and_children(&tasks, &task, 0, &mut index);
        }

        for task in completed_root_tasks {
            self.add_task_and_children(&tasks, &task, 0, &mut index);
        }
    }

    fn add_task_and_children(
        &mut self,
        all_tasks: &[Task],
        task: &Task,
        level: usize,
        index: &mut usize,
    ) {
        self.task_displays.push(TaskDisplay {
            task: task.clone(),
            level,
            index: *index,
        });
        *index += 1;

        let mut children: Vec<Task> = all_tasks
            .iter()
            .filter(|t| t.parent_task_id == Some(task.id))
            .cloned()
            .collect();
        children.sort_by_key(|t| t.created_at);

        let incomplete_children: Vec<Task> =
            children.iter().filter(|t| !t.completed).cloned().collect();
        let completed_children: Vec<Task> =
            children.iter().filter(|t| t.completed).cloned().collect();

        for child in incomplete_children {
            self.add_task_and_children(all_tasks, &child, level + 1, index);
        }

        for child in completed_children {
            self.add_task_and_children(all_tasks, &child, level + 1, index);
        }
    }

    pub async fn next_workspace(&mut self) -> Result<()> {
        let i = match self.workspace_state.selected() {
            Some(i) => {
                if i >= self.workspaces.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.workspace_state.select(Some(i));
        self.selected_workspace = Some(i);
        self.load_tasks_for_selected_workspace().await?;
        Ok(())
    }

    pub async fn previous_workspace(&mut self) -> Result<()> {
        let i = match self.workspace_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.workspaces.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.workspace_state.select(Some(i));
        self.selected_workspace = Some(i);
        self.load_tasks_for_selected_workspace().await?;
        Ok(())
    }

    pub fn next_task(&mut self) {
        let i = match self.task_state.selected() {
            Some(i) => {
                if i >= self.task_displays.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.task_state.select(Some(i));
    }

    pub fn previous_task(&mut self) {
        let i = match self.task_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.task_displays.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.task_state.select(Some(i));
    }

    pub fn start_creating_subtask(&mut self) {
        self.input_buffer.clear();
        self.creating_subtask = true;
        self.input_mode = InputMode::Creating;
    }

    pub fn start_creating_task(&mut self) {
        self.input_buffer.clear();
        self.creating_subtask = false;
        self.input_mode = InputMode::Creating;
    }

    pub async fn finish_creating(&mut self) -> Result<()> {
        if self.input_buffer.trim().is_empty() {
            self.cancel_creating();
            return Ok(());
        }

        match self.focus {
            Focus::Workspaces => {
                self.db.create_workspace(&self.input_buffer).await?;
                self.load_workspaces().await?;
            }
            Focus::Tasks => {
                if let Some(selected) = self.selected_workspace {
                    if let Some(workspace) = self.workspaces.get(selected) {
                        if self.creating_subtask {
                            if let Some(task_display_idx) = self.task_state.selected() {
                                if let Some(task_display) = self.task_displays.get(task_display_idx)
                                {
                                    self.db
                                        .create_subtask(
                                            &self.input_buffer,
                                            workspace.id,
                                            task_display.task.id,
                                        )
                                        .await?;
                                } else {
                                    self.db
                                        .create_task(&self.input_buffer, workspace.id)
                                        .await?;
                                }
                            } else {
                                self.db
                                    .create_task(&self.input_buffer, workspace.id)
                                    .await?;
                            }
                        } else {
                            self.db
                                .create_task(&self.input_buffer, workspace.id)
                                .await?;
                        }
                        self.load_tasks_for_selected_workspace().await?;
                    }
                }
            }
        }
        self.cancel_creating();
        Ok(())
    }

    pub fn cancel_creating(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub async fn toggle_current_task_completion(&mut self) -> Result<()> {
        if self.focus == Focus::Tasks {
            if let Some(selected_task_idx) = self.task_state.selected() {
                if let Some(task_display) = self.task_displays.get(selected_task_idx) {
                    self.db.toggle_task_completion(task_display.task.id).await?;
                    let current_selection = self.task_state.selected();
                    self.load_tasks_for_selected_workspace().await?;
                    self.task_state.select(current_selection);
                }
            }
        }
        Ok(())
    }

    pub fn start_rename(&mut self) {
        let current_name = match self.focus {
            Focus::Workspaces => {
                if let Some(selected) = self.workspace_state.selected() {
                    self.workspaces
                        .get(selected)
                        .map(|w| w.name.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            Focus::Tasks => {
                if let Some(selected) = self.task_state.selected() {
                    self.task_displays
                        .get(selected)
                        .map(|td| td.task.title.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
        };
        self.input_buffer = current_name;
        self.input_mode = InputMode::Insert;
    }

    pub async fn finish_rename(&mut self) -> Result<()> {
        match self.focus {
            Focus::Workspaces => {
                if let Some(selected) = self.workspace_state.selected() {
                    if let Some(workspace) = self.workspaces.get(selected) {
                        self.db
                            .update_workspace_name(workspace.id, &self.input_buffer)
                            .await?;
                        self.load_workspaces().await?;
                    }
                }
            }
            Focus::Tasks => {
                if let Some(selected) = self.task_state.selected() {
                    if let Some(task_display) = self.task_displays.get(selected) {
                        self.db
                            .update_task_name(task_display.task.id, &self.input_buffer)
                            .await?;
                        self.load_tasks_for_selected_workspace().await?;
                    }
                }
            }
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        Ok(())
    }

    pub fn cancel_rename(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn start_delete_confirm(&mut self) {
        let target_name = match self.focus {
            Focus::Workspaces => {
                if let Some(selected) = self.workspace_state.selected() {
                    self.workspaces
                        .get(selected)
                        .map(|w| w.name.clone())
                        .unwrap_or_default()
                } else {
                    return;
                }
            }
            Focus::Tasks => {
                if let Some(selected) = self.task_state.selected() {
                    self.task_displays
                        .get(selected)
                        .map(|td| td.task.title.clone())
                        .unwrap_or_default()
                } else {
                    return;
                }
            }
        };
        self.delete_target = Some(target_name);
        self.input_mode = InputMode::DeleteConfirm;
    }

    pub async fn confirm_delete(&mut self) -> Result<()> {
        match self.focus {
            Focus::Workspaces => {
                if let Some(selected) = self.workspace_state.selected() {
                    if let Some(workspace) = self.workspaces.get(selected) {
                        self.db.delete_workspace(workspace.id).await?;
                        self.load_workspaces().await?;
                        if !self.workspaces.is_empty() {
                            let new_selection = if selected >= self.workspaces.len() {
                                self.workspaces.len() - 1
                            } else {
                                selected
                            };
                            self.workspace_state.select(Some(new_selection));
                            self.selected_workspace = Some(new_selection);
                            self.load_tasks_for_selected_workspace().await?;
                        }
                    }
                }
            }
            Focus::Tasks => {
                if let Some(selected) = self.task_state.selected() {
                    if let Some(task_display) = self.task_displays.get(selected) {
                        self.db.delete_task(task_display.task.id).await?;
                        self.load_tasks_for_selected_workspace().await?;
                        if !self.task_displays.is_empty() {
                            let new_selection = if selected >= self.task_displays.len() {
                                self.task_displays.len() - 1
                            } else {
                                selected
                            };
                            self.task_state.select(Some(new_selection));
                        }
                    }
                }
            }
        }
        self.cancel_delete_confirm();
        Ok(())
    }

    pub fn cancel_delete_confirm(&mut self) {
        self.input_mode = InputMode::Normal;
        self.delete_target = None;
    }

    pub fn show_help(&mut self) {
        self.input_mode = InputMode::Help;
    }

    pub fn hide_help(&mut self) {
        self.input_mode = InputMode::Normal;
    }
}

pub async fn run_app(db: Database) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(db);
    app.load_workspaces().await?;

    let res = run_app_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => match app.focus {
                        Focus::Workspaces => app.next_workspace().await?,
                        Focus::Tasks => app.next_task(),
                    },
                    KeyCode::Up | KeyCode::Char('k') => match app.focus {
                        Focus::Workspaces => app.previous_workspace().await?,
                        Focus::Tasks => app.previous_task(),
                    },
                    KeyCode::Right | KeyCode::Char('l') => {
                        app.focus = Focus::Tasks;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        app.focus = Focus::Workspaces;
                    }
                    KeyCode::Tab => {
                        app.focus = match app.focus {
                            Focus::Workspaces => Focus::Tasks,
                            Focus::Tasks => Focus::Workspaces,
                        };
                    }
                    KeyCode::Char('a') => {
                        if app.focus == Focus::Tasks {
                            app.start_creating_subtask();
                        } else {
                            app.start_creating_task();
                        }
                    }
                    KeyCode::Char('A') => {
                        app.start_creating_task();
                    }
                    KeyCode::Char('c') => {
                        app.toggle_current_task_completion().await?;
                    }
                    KeyCode::Char('r') => {
                        app.start_rename();
                    }
                    KeyCode::Char('D') => {
                        app.start_delete_confirm();
                    }
                    KeyCode::Char('?') => {
                        app.show_help();
                    }
                    _ => {}
                },
                InputMode::Insert => match key.code {
                    KeyCode::Enter => {
                        app.finish_rename().await?;
                    }
                    KeyCode::Esc => {
                        app.cancel_rename();
                    }
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        app.input_buffer.push(c);
                    }
                    _ => {}
                },
                InputMode::Creating => match key.code {
                    KeyCode::Enter => {
                        app.finish_creating().await?;
                    }
                    KeyCode::Esc => {
                        app.cancel_creating();
                    }
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        app.input_buffer.push(c);
                    }
                    _ => {}
                },
                InputMode::DeleteConfirm => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        app.confirm_delete().await?;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.cancel_delete_confirm();
                    }
                    _ => {}
                },
                InputMode::Help => match key.code {
                    KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                        app.hide_help();
                    }
                    _ => {}
                },
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(main_chunks[0]);

    let workspace_items: Vec<ListItem> = app
        .workspaces
        .iter()
        .map(|w| ListItem::new(Span::raw(&w.name)))
        .collect();

    let workspace_block = if app.focus == Focus::Workspaces {
        Block::default()
            .title("workspaces")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
    } else {
        Block::default().title("workspaces").borders(Borders::ALL)
    };
    let workspaces = List::new(workspace_items)
        .block(workspace_block)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(workspaces, content_chunks[0], &mut app.workspace_state);

    let task_items: Vec<ListItem> = app
        .task_displays
        .iter()
        .map(|td| {
            let indent = "  ".repeat(td.level);
            let checkbox = if td.task.completed { "×" } else { " " };
            let date = td.task.created_at.format("%m/%d/%y").to_string();

            let task_span = Span::raw(format!("{}[{}] {}", indent, checkbox, td.task.title));
            let date_span =
                Span::styled(format!(" ({date})"), Style::default().fg(Color::DarkGray));

            if td.task.completed {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}[{}] {}", indent, checkbox, td.task.title),
                        Style::default()
                            .add_modifier(Modifier::CROSSED_OUT)
                            .fg(Color::DarkGray),
                    ),
                    date_span,
                ]))
            } else {
                ListItem::new(Line::from(vec![task_span, date_span]))
            }
        })
        .collect();

    let task_block = if app.focus == Focus::Tasks {
        Block::default()
            .title("tasks")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
    } else {
        Block::default().title("tasks").borders(Borders::ALL)
    };
    let tasks = List::new(task_items)
        .block(task_block)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(tasks, content_chunks[1], &mut app.task_state);

    match app.input_mode {
        InputMode::Insert => {
            let popup_area = centered_rect(60, 20, f.area());
            f.render_widget(Clear, popup_area);

            let input = Paragraph::new(app.input_buffer.as_str())
                .block(Block::default().title("rename").borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(input, popup_area);
        }
        InputMode::DeleteConfirm => {
            let popup_area = centered_rect(60, 20, f.area());
            f.render_widget(Clear, popup_area);

            let target_name = app.delete_target.as_deref().unwrap_or("item");
            let confirm_text = format!("Delete '{target_name}'?\n\ny: confirm | n/esc: cancel");
            let confirm = Paragraph::new(confirm_text)
                .block(
                    Block::default()
                        .title("confirm delete")
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::Red));
            f.render_widget(confirm, popup_area);
        }
        InputMode::Creating => {
            let popup_area = centered_rect(60, 20, f.area());
            f.render_widget(Clear, popup_area);

            let title = match app.focus {
                Focus::Workspaces => "new workspace",
                Focus::Tasks => {
                    if app.creating_subtask {
                        "new subtask"
                    } else {
                        "new task"
                    }
                }
            };
            let input = Paragraph::new(app.input_buffer.as_str())
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(Style::default().fg(Color::Green));
            f.render_widget(input, popup_area);
        }
        InputMode::Help => {
            let popup_area = centered_rect(80, 60, f.area());
            f.render_widget(Clear, popup_area);

            let help_text = r#"Navigation:
  h/l/tab: switch focus between workspaces and tasks
  j/k: navigate up/down in focused panel

Actions:
  a: add subtask (when on tasks) or workspace
  A: add new top-level task
  r: rename selected item
  c: complete/uncomplete task
  D: delete selected item
  ?: show/hide this help
  q: quit

Press ? or ESC to close"#;
            let help = Paragraph::new(help_text)
                .block(Block::default().title("help").borders(Borders::ALL))
                .style(Style::default().fg(Color::White));
            f.render_widget(help, popup_area);
        }
        InputMode::Normal => {}
    }

    let status_bar = Paragraph::new("q: quit | ?: help")
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(status_bar, main_chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

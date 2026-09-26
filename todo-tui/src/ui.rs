use anyhow::Result;
use chrono::{Local, NaiveDate};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        cursor::SetCursorStyle,
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::io;

use todo_core::{Database, Task, Workspace, WorkspaceStats};
use tui_input::{Input, backend::crossterm::EventHandler};

#[derive(Debug, Clone)]
pub struct TaskDisplay {
    pub task: Task,
    pub level: usize,
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
    Search,
}

#[derive(PartialEq)]
pub enum EditField {
    Title,
    DueDate,
}

pub struct App {
    pub workspaces: Vec<Workspace>,
    pub workspace_stats: HashMap<i64, WorkspaceStats>,
    pub tasks: Vec<Task>,
    pub task_displays: Vec<TaskDisplay>,
    pub workspace_state: ListState,
    pub task_state: ListState,
    pub selected_workspace: Option<usize>,
    pub db: Database,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buffer: Input,
    pub edit_title_buffer: Input,
    pub edit_due_date_buffer: Input,
    pub edit_field: EditField,
    pub search_query: String,
    pub delete_target: Option<String>,
    pub creating_subtask: bool,
    pub sort_created_desc: bool,
}

impl App {
    pub fn new(db: Database) -> Self {
        let mut workspace_state = ListState::default();
        workspace_state.select(Some(0));

        Self {
            workspaces: vec![],
            workspace_stats: HashMap::new(),
            tasks: vec![],
            task_displays: vec![],
            workspace_state,
            task_state: ListState::default(),
            selected_workspace: Some(0),
            db,
            focus: Focus::Workspaces,
            input_mode: InputMode::Normal,
            input_buffer: Input::default(),
            edit_title_buffer: Input::default(),
            edit_due_date_buffer: Input::default(),
            edit_field: EditField::Title,
            search_query: String::new(),
            delete_target: None,
            creating_subtask: false,
            sort_created_desc: true,
        }
    }

    pub async fn load_workspaces(&mut self) -> Result<()> {
        self.workspaces = self.db.get_workspaces().await?;
        self.refresh_workspace_stats().await?;
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
                self.refresh_workspace_stats().await?;
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

    async fn refresh_workspace_stats(&mut self) -> Result<()> {
        self.workspace_stats = self
            .db
            .get_workspace_stats()
            .await?
            .into_iter()
            .map(|stats| (stats.workspace_id, stats))
            .collect();
        Ok(())
    }

    fn build_task_hierarchy(&mut self) {
        self.task_displays.clear();
        let search_query = self.search_query.trim().to_lowercase();
        let tasks: Vec<Task> = self
            .tasks
            .iter()
            .filter(|task| search_query.is_empty() || fuzzy_matches(&task.title, &search_query))
            .cloned()
            .collect();
        let active_task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();

        let mut incomplete_root_tasks: Vec<Task> = tasks
            .iter()
            .filter(|t| {
                !t.completed
                    && t.parent_task_id
                        .is_none_or(|parent_id| !active_task_ids.contains(&parent_id))
            })
            .cloned()
            .collect();
        self.sort_tasks_by_created_at(&mut incomplete_root_tasks);

        let mut completed_root_tasks: Vec<Task> = tasks
            .iter()
            .filter(|t| {
                t.completed
                    && t.parent_task_id
                        .is_none_or(|parent_id| !active_task_ids.contains(&parent_id))
            })
            .cloned()
            .collect();
        self.sort_tasks_by_created_at(&mut completed_root_tasks);

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
        });
        *index += 1;

        let mut children: Vec<Task> = all_tasks
            .iter()
            .filter(|t| t.parent_task_id == Some(task.id))
            .cloned()
            .collect();
        self.sort_tasks_by_created_at(&mut children);

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

    fn sort_tasks_by_created_at(&self, tasks: &mut [Task]) {
        if self.sort_created_desc {
            tasks.sort_by_key(|t| Reverse(t.created_at));
        } else {
            tasks.sort_by_key(|t| t.created_at);
        }
    }

    pub fn toggle_sort_order(&mut self) {
        let selected_task_id = self
            .task_state
            .selected()
            .and_then(|idx| self.task_displays.get(idx))
            .map(|td| td.task.id);

        self.sort_created_desc = !self.sort_created_desc;
        self.build_task_hierarchy();

        if let Some(task_id) = selected_task_id {
            let new_selection = self
                .task_displays
                .iter()
                .position(|td| td.task.id == task_id)
                .or_else(|| (!self.task_displays.is_empty()).then_some(0));
            self.task_state.select(new_selection);
        }
    }

    fn select_first_visible_task(&mut self) {
        self.task_state.select(if self.task_displays.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    pub fn start_search(&mut self) {
        self.input_buffer = self.search_query.clone().into();
        self.input_mode = InputMode::Search;
        self.focus = Focus::Tasks;
    }

    pub fn update_search(&mut self) {
        self.search_query = self.input_buffer.value().to_string();
        self.build_task_hierarchy();
        self.select_first_visible_task();
    }

    pub fn finish_search(&mut self) {
        self.search_query = self.input_buffer.value().to_string();
        self.input_mode = InputMode::Normal;
        self.input_buffer.reset();
        self.build_task_hierarchy();
        self.select_first_visible_task();
    }

    pub fn cancel_search(&mut self) {
        self.search_query.clear();
        self.input_mode = InputMode::Normal;
        self.input_buffer.reset();
        self.build_task_hierarchy();
        self.select_first_visible_task();
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
        if self.task_displays.is_empty() {
            self.task_state.select(None);
            return;
        }

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
        if self.task_displays.is_empty() {
            self.task_state.select(None);
            return;
        }

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
        self.input_buffer.reset();
        self.creating_subtask = true;
        self.input_mode = InputMode::Creating;
    }

    pub fn start_creating_task(&mut self) {
        self.input_buffer.reset();
        self.creating_subtask = false;
        self.input_mode = InputMode::Creating;
    }

    pub async fn finish_creating(&mut self) -> Result<()> {
        if self.input_buffer.value().trim().is_empty() {
            self.cancel_creating();
            return Ok(());
        }

        match self.focus {
            Focus::Workspaces => {
                self.db.create_workspace(self.input_buffer.value()).await?;
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
                                            self.input_buffer.value(),
                                            workspace.id,
                                            task_display.task.id,
                                        )
                                        .await?;
                                } else {
                                    self.db
                                        .create_task(self.input_buffer.value(), workspace.id)
                                        .await?;
                                }
                            } else {
                                self.db
                                    .create_task(self.input_buffer.value(), workspace.id)
                                    .await?;
                            }
                        } else {
                            self.db
                                .create_task(self.input_buffer.value(), workspace.id)
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
        self.input_buffer.reset();
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

    pub async fn archive_completed_tasks(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_workspace {
            if let Some(workspace) = self.workspaces.get(selected) {
                self.db.archive_completed_tasks(workspace.id).await?;
                self.load_tasks_for_selected_workspace().await?;
            }
        }
        Ok(())
    }

    pub fn start_rename(&mut self) {
        match self.focus {
            Focus::Workspaces => {
                let current_name = self
                    .workspace_state
                    .selected()
                    .and_then(|selected| self.workspaces.get(selected))
                    .map(|w| w.name.clone())
                    .unwrap_or_default();
                self.input_buffer = current_name.into();
            }
            Focus::Tasks => {
                let Some(selected) = self.task_state.selected() else {
                    return;
                };
                let Some(task_display) = self.task_displays.get(selected) else {
                    return;
                };
                self.edit_title_buffer = task_display.task.title.clone().into();
                self.edit_due_date_buffer = task_display
                    .task
                    .due_date
                    .clone()
                    .unwrap_or_default()
                    .into();
                self.edit_field = EditField::Title;
            }
        }
        self.input_mode = InputMode::Insert;
    }

    pub fn toggle_edit_field(&mut self) {
        self.edit_field = match self.edit_field {
            EditField::Title => EditField::DueDate,
            EditField::DueDate => EditField::Title,
        };
    }

    pub fn active_edit_input_mut(&mut self) -> &mut Input {
        match self.focus {
            Focus::Workspaces => &mut self.input_buffer,
            Focus::Tasks => match self.edit_field {
                EditField::Title => &mut self.edit_title_buffer,
                EditField::DueDate => &mut self.edit_due_date_buffer,
            },
        }
    }

    pub async fn finish_rename(&mut self) -> Result<()> {
        match self.focus {
            Focus::Workspaces => {
                if let Some(selected) = self.workspace_state.selected() {
                    if let Some(workspace) = self.workspaces.get(selected) {
                        self.db
                            .update_workspace_name(workspace.id, self.input_buffer.value())
                            .await?;
                        self.load_workspaces().await?;
                    }
                }
            }
            Focus::Tasks => {
                if let Some(selected) = self.task_state.selected() {
                    if let Some(task_display) = self.task_displays.get(selected) {
                        let due_date = self.edit_due_date_buffer.value().trim();
                        let normalized_due_date = if due_date.is_empty() {
                            None
                        } else {
                            let Ok(date) = NaiveDate::parse_from_str(due_date, "%Y-%m-%d") else {
                                return Ok(());
                            };
                            Some(date.format("%Y-%m-%d").to_string())
                        };

                        self.db
                            .update_task_name(task_display.task.id, self.edit_title_buffer.value())
                            .await?;
                        self.db
                            .update_task_due_date(
                                task_display.task.id,
                                normalized_due_date.as_deref(),
                            )
                            .await?;
                        self.load_tasks_for_selected_workspace().await?;
                    }
                }
            }
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.reset();
        self.edit_title_buffer.reset();
        self.edit_due_date_buffer.reset();
        Ok(())
    }

    pub fn cancel_rename(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.reset();
        self.edit_title_buffer.reset();
        self.edit_due_date_buffer.reset();
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

        match app.input_mode {
            InputMode::Insert | InputMode::Creating | InputMode::Search => {
                execute!(io::stdout(), SetCursorStyle::BlinkingBar)?;
            }
            _ => {
                execute!(io::stdout(), SetCursorStyle::DefaultUserShape)?;
            }
        }

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
                    KeyCode::Esc => {
                        if !app.search_query.is_empty() {
                            app.cancel_search();
                        }
                    }
                    KeyCode::Char('A') => {
                        if app.focus == Focus::Tasks {
                            app.start_creating_subtask();
                        } else {
                            app.start_creating_task();
                        }
                    }
                    KeyCode::Char('a') => {
                        app.start_creating_task();
                    }
                    KeyCode::Char('c') | KeyCode::Char(' ') => {
                        app.toggle_current_task_completion().await?;
                    }
                    KeyCode::Char('e') => {
                        app.start_rename();
                    }
                    KeyCode::Char('/') => {
                        app.start_search();
                    }
                    KeyCode::Char('s') => {
                        app.toggle_sort_order();
                    }
                    KeyCode::Char('x') => {
                        app.archive_completed_tasks().await?;
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
                    KeyCode::Tab => {
                        if app.focus == Focus::Tasks {
                            app.toggle_edit_field();
                        }
                    }
                    KeyCode::Up => {
                        if app.focus == Focus::Tasks {
                            app.edit_field = EditField::Title;
                        }
                    }
                    KeyCode::Down => {
                        if app.focus == Focus::Tasks {
                            app.edit_field = EditField::DueDate;
                        }
                    }
                    _ => {
                        app.active_edit_input_mut().handle_event(&Event::Key(key));
                    }
                },
                InputMode::Creating => match key.code {
                    KeyCode::Enter => {
                        app.finish_creating().await?;
                    }
                    KeyCode::Esc => {
                        app.cancel_creating();
                    }
                    _ => {
                        app.input_buffer.handle_event(&Event::Key(key));
                    }
                },
                InputMode::Search => match key.code {
                    KeyCode::Enter => {
                        app.finish_search();
                    }
                    KeyCode::Esc => {
                        app.cancel_search();
                    }
                    _ => {
                        app.input_buffer.handle_event(&Event::Key(key));
                        app.update_search();
                    }
                },
                InputMode::DeleteConfirm => match key.code {
                    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
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
        .map(|w| {
            let stats = app.workspace_stats.get(&w.id);
            let completed = stats.map_or(0, |s| s.completed);
            let total = stats.map_or(0, |s| s.total);

            ListItem::new(Line::from(vec![
                Span::raw(&w.name),
                Span::styled(
                    format!(" ({completed}/{total})"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
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
            let today = Local::now().date_naive();
            let due_date_span = td.task.due_date.as_ref().map(|due_date| {
                let due_date_color = match NaiveDate::parse_from_str(due_date, "%Y-%m-%d") {
                    Ok(parsed_due_date) if today > parsed_due_date => Color::Red,
                    _ => Color::Yellow,
                };
                Span::styled(
                    format!(" due {due_date}"),
                    Style::default().fg(due_date_color),
                )
            });

            if td.task.completed {
                let mut spans = vec![
                    Span::styled(
                        format!("{}[{}] {}", indent, checkbox, td.task.title),
                        Style::default()
                            .add_modifier(Modifier::CROSSED_OUT)
                            .fg(Color::DarkGray),
                    ),
                    date_span,
                ];
                if let Some(due_date_span) = due_date_span {
                    spans.push(due_date_span);
                }
                ListItem::new(Line::from(spans))
            } else {
                let mut spans = vec![task_span, date_span];
                if let Some(due_date_span) = due_date_span {
                    spans.push(due_date_span);
                }
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    let sort_label = if app.sort_created_desc {
        "newest first"
    } else {
        "oldest first"
    };
    let task_title = if app.search_query.trim().is_empty() {
        format!("tasks ({sort_label})")
    } else {
        format!("tasks ({sort_label}, search)")
    };
    let task_block = if app.focus == Focus::Tasks {
        Block::default()
            .title(task_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
    } else {
        Block::default().title(task_title).borders(Borders::ALL)
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

            match app.focus {
                Focus::Workspaces => {
                    let input = Paragraph::new(app.input_buffer.value())
                        .block(
                            Block::default()
                                .title("edit workspace")
                                .borders(Borders::ALL),
                        )
                        .style(Style::default().fg(Color::Yellow));
                    f.render_widget(input, popup_area);
                    f.set_cursor_position((
                        popup_area.x + 1 + app.input_buffer.visual_cursor() as u16,
                        popup_area.y + 1,
                    ));
                }
                Focus::Tasks => {
                    let title_marker = if app.edit_field == EditField::Title {
                        "> "
                    } else {
                        "  "
                    };
                    let due_date_marker = if app.edit_field == EditField::DueDate {
                        "> "
                    } else {
                        "  "
                    };
                    let due_date_line = if app.edit_due_date_buffer.value().is_empty() {
                        Line::from(vec![
                            Span::raw(format!("{due_date_marker}due: ")),
                            Span::styled("YYYY-MM-DD", Style::default().fg(Color::DarkGray)),
                        ])
                    } else {
                        Line::from(format!(
                            "{due_date_marker}due: {}",
                            app.edit_due_date_buffer.value()
                        ))
                    };
                    let input = Paragraph::new(vec![
                        Line::from(format!(
                            "{title_marker}title: {}",
                            app.edit_title_buffer.value()
                        )),
                        due_date_line,
                    ])
                    .block(Block::default().title("edit task").borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow));
                    f.render_widget(input, popup_area);

                    let (cursor_y, cursor_x) = match app.edit_field {
                        EditField::Title => (
                            popup_area.y + 1,
                            popup_area.x
                                + 1
                                + title_marker.len() as u16
                                + 7
                                + app.edit_title_buffer.visual_cursor() as u16,
                        ),
                        EditField::DueDate => (
                            popup_area.y + 2,
                            popup_area.x
                                + 1
                                + due_date_marker.len() as u16
                                + 5
                                + app.edit_due_date_buffer.visual_cursor() as u16,
                        ),
                    };
                    f.set_cursor_position((cursor_x, cursor_y));
                }
            }
        }
        InputMode::DeleteConfirm => {
            let popup_area = centered_rect(60, 20, f.area());
            f.render_widget(Clear, popup_area);

            let target_name = app.delete_target.as_deref().unwrap_or("item");
            let confirm_text =
                format!("Delete '{target_name}'?\n\nenter/y: confirm | n/esc: cancel");
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
            let input = Paragraph::new(app.input_buffer.value())
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(Style::default().fg(Color::Green));
            f.render_widget(input, popup_area);
            f.set_cursor_position((
                popup_area.x + 1 + app.input_buffer.visual_cursor() as u16,
                popup_area.y + 1,
            ));
        }
        InputMode::Help => {
            let popup_area = centered_rect(80, 60, f.area());
            f.render_widget(Clear, popup_area);

            let help_text = r#"Navigation:
  h/l/tab: switch focus between workspaces and tasks
  j/k: navigate up/down in focused panel

Actions:
  A: add subtask (when on tasks) or workspace
  a: add new top-level task
  /: search tasks
  e: edit selected item
  tab: switch edit fields
  s: reverse creation-date sort
  x: archive completed tasks
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
        InputMode::Search => {}
        InputMode::Normal => {}
    }

    let status_text = if app.input_mode == InputMode::Search {
        format!("/{}", app.input_buffer.value())
    } else if app.search_query.trim().is_empty() {
        "q: quit | ?: help".to_string()
    } else {
        format!("search: {} | /: edit | esc: clear", app.search_query)
    };
    let status_bar = Paragraph::new(status_text).style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(status_bar, main_chunks[1]);
    if app.input_mode == InputMode::Search {
        f.set_cursor_position((
            main_chunks[1].x + 1 + app.input_buffer.visual_cursor() as u16,
            main_chunks[1].y,
        ));
    }
}

fn fuzzy_matches(text: &str, query: &str) -> bool {
    let mut query_chars = query.chars();
    let Some(mut query_char) = query_chars.next() else {
        return true;
    };

    for text_char in text.chars().flat_map(char::to_lowercase) {
        if text_char == query_char {
            match query_chars.next() {
                Some(next_query_char) => query_char = next_query_char,
                None => return true,
            }
        }
    }

    false
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

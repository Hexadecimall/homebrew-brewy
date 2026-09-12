use std::{cmp::Ordering, collections::HashSet, sync::mpsc::Sender};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::{
    brew::{self, WorkerEvent},
    model::{Catalog, Operation, Package, PackageKind, fuzzy_score, operation_for},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Browse,
    Installed,
    Outdated,
    Casks,
    Taps,
}

impl Tab {
    pub const ALL: [Self; 5] = [
        Self::Browse,
        Self::Installed,
        Self::Outdated,
        Self::Casks,
        Self::Taps,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Browse => "Browse",
            Self::Installed => "Installed",
            Self::Outdated => "Outdated",
            Self::Casks => "Casks",
            Self::Taps => "Taps",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortMode {
    Name,
    Status,
    Kind,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Status => "status",
            Self::Kind => "kind",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Name => Self::Status,
            Self::Status => Self::Kind,
            Self::Kind => Self::Name,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Overlay {
    None,
    Help,
}

pub struct App {
    pub catalog: Catalog,
    pub tab: Tab,
    pub selected: usize,
    pub query: String,
    pub overlay: Overlay,
    pub sort: SortMode,
    pub queue: Vec<Operation>,
    pub log: Vec<String>,
    pub loading: bool,
    pub running: bool,
    pub status: String,
    pub should_quit: bool,
    pub failed_operations: usize,
    pub preview_visible: bool,
    pub preview_scroll: u16,
    detail_requested: HashSet<String>,
    tx: Sender<WorkerEvent>,
}

impl App {
    pub fn new(tx: Sender<WorkerEvent>) -> Self {
        Self {
            catalog: Catalog::default(),
            tab: Tab::Browse,
            selected: 0,
            query: String::new(),
            overlay: Overlay::None,
            sort: SortMode::Name,
            queue: Vec::new(),
            log: Vec::new(),
            loading: true,
            running: false,
            status: "Loading Homebrew catalog…".into(),
            should_quit: false,
            failed_operations: 0,
            preview_visible: true,
            preview_scroll: 0,
            detail_requested: HashSet::new(),
            tx,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let mut results: Vec<(usize, i64)> = self
            .catalog
            .packages
            .iter()
            .enumerate()
            .filter(|(_, package)| match self.tab {
                Tab::Browse => true,
                Tab::Installed => package.installed,
                Tab::Outdated => package.outdated,
                Tab::Casks => package.kind == PackageKind::Cask,
                Tab::Taps => false,
            })
            .filter_map(|(index, package)| {
                fuzzy_score(&package.name, &self.query).map(|score| (index, score))
            })
            .collect();

        results.sort_unstable_by(|(left_index, left_score), (right_index, right_score)| {
            let left = &self.catalog.packages[*left_index];
            let right = &self.catalog.packages[*right_index];
            if !self.query.is_empty() {
                right_score
                    .cmp(left_score)
                    .then_with(|| left.name.cmp(&right.name))
            } else {
                self.compare_packages(left, right)
            }
        });
        results.into_iter().map(|(index, _)| index).collect()
    }

    fn compare_packages(&self, left: &Package, right: &Package) -> Ordering {
        match self.sort {
            SortMode::Name => left.name.cmp(&right.name),
            SortMode::Status => right
                .outdated
                .cmp(&left.outdated)
                .then_with(|| right.installed.cmp(&left.installed))
                .then_with(|| left.name.cmp(&right.name)),
            SortMode::Kind => format!("{:?}", left.kind)
                .cmp(&format!("{:?}", right.kind))
                .then_with(|| left.name.cmp(&right.name)),
        }
    }

    pub fn selected_package(&self) -> Option<&Package> {
        let indices = self.filtered_indices();
        indices
            .get(self.selected)
            .and_then(|index| self.catalog.packages.get(*index))
    }

    pub fn handle_key(&mut self, event: KeyEvent) {
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('q') {
            if self.running {
                self.status = "Homebrew is still running; wait for it to finish".into();
            } else {
                self.should_quit = true;
            }
            return;
        }
        match self.overlay {
            Overlay::Help => {
                if matches!(
                    event.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter
                ) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::None => self.handle_global_key(event),
        }
    }

    fn handle_global_key(&mut self, event: KeyEvent) {
        if event.modifiers.contains(KeyModifiers::ALT) {
            match event.code {
                KeyCode::Char('p') => self.preview_visible = !self.preview_visible,
                KeyCode::Char('j') => self.preview_scroll = self.preview_scroll.saturating_add(3),
                KeyCode::Char('k') => self.preview_scroll = self.preview_scroll.saturating_sub(3),
                _ => {}
            }
            return;
        }
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            match event.code {
                KeyCode::Char('a') if !self.queue.is_empty() && !self.running => self.run_queue(),
                KeyCode::Char('p') if !self.running => self.toggle_pin(),
                KeyCode::Char('r') if !self.loading && !self.running => self.refresh(),
                KeyCode::Char('s') => {
                    self.sort = self.sort.next();
                    self.selected = 0;
                }
                KeyCode::Char('u') if !self.running => {
                    self.queue = vec![Operation::Update];
                    self.run_queue();
                }
                KeyCode::Left => self.change_tab(-1),
                KeyCode::Right => self.change_tab(1),
                _ => {}
            }
            return;
        }
        match event.code {
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Tab if !self.running => self.toggle_stage(),
            KeyCode::Enter if !self.running => {
                if self.queue.is_empty() {
                    self.toggle_stage();
                }
                if !self.queue.is_empty() {
                    self.run_queue();
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            KeyCode::Down => self.move_selection(1),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Right => self.change_tab(1),
            KeyCode::Left | KeyCode::BackTab => self.change_tab(-1),
            KeyCode::Esc => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.selected = 0;
                }
            }
            KeyCode::Char(' ') if !self.running => self.toggle_stage(),
            KeyCode::Char(ch) if !ch.is_control() => {
                self.query.push(ch);
                self.selected = 0;
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollDown => self.move_selection(3),
            MouseEventKind::ScrollUp => self.move_selection(-3),
            _ => {}
        }
    }

    fn item_count(&self) -> usize {
        if self.tab == Tab::Taps {
            self.catalog.taps.len()
        } else {
            self.filtered_indices().len()
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let max = self.item_count().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
        self.preview_scroll = 0;
    }

    fn change_tab(&mut self, delta: isize) {
        let current = Tab::ALL
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(Tab::ALL.len() as isize) as usize;
        self.tab = Tab::ALL[next];
        self.selected = 0;
    }

    fn toggle_stage(&mut self) {
        let Some(package) = self.selected_package().cloned() else {
            return;
        };
        if let Some(index) = self
            .queue
            .iter()
            .position(|operation| operation.package_name() == Some(&package.name))
        {
            self.queue.remove(index);
            self.status = format!("Unstaged {}", package.name);
        } else {
            let operation = operation_for(&package);
            self.status = format!("Staged: {}", operation.summary());
            self.queue.push(operation);
        }
    }

    fn run_queue(&mut self) {
        if self.queue.is_empty() || self.running {
            return;
        }
        self.overlay = Overlay::None;
        self.running = true;
        self.failed_operations = 0;
        self.log.clear();
        let operations = std::mem::take(&mut self.queue);
        brew::run_operations(operations, self.tx.clone());
    }

    fn toggle_pin(&mut self) {
        let Some(package) = self.selected_package().cloned() else {
            return;
        };
        if !package.installed || package.kind != PackageKind::Formula {
            self.status = "Only installed formulae can be pinned".into();
            return;
        }
        self.queue
            .retain(|operation| operation.package_name() != Some(&package.name));
        self.queue.push(if package.pinned {
            Operation::Unpin {
                name: package.name.clone(),
            }
        } else {
            Operation::Pin {
                name: package.name.clone(),
            }
        });
        self.status = format!(
            "Staged: {} {}",
            if package.pinned { "unpin" } else { "pin" },
            package.name
        );
    }

    pub fn refresh(&mut self) {
        self.loading = true;
        self.status = "Refreshing Homebrew catalog…".into();
        brew::load_catalog(self.tx.clone());
    }

    pub fn request_selected_detail(&mut self) {
        if !self.preview_visible || self.loading {
            return;
        }
        let Some(name) = self.selected_package().map(|package| package.name.clone()) else {
            return;
        };
        if self.detail_requested.insert(name.clone()) {
            brew::load_detail(name, self.tx.clone());
        }
    }

    pub fn handle_worker_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::CatalogLoaded(Ok(catalog)) => {
                self.catalog = catalog;
                self.detail_requested.clear();
                self.loading = false;
                self.selected = self.selected.min(self.item_count().saturating_sub(1));
                self.status = format!(
                    "Ready · {} installed · {} outdated",
                    self.catalog.installed_count(),
                    self.catalog.outdated_count()
                );
            }
            WorkerEvent::CatalogLoaded(Err(error)) => {
                self.loading = false;
                self.status = error;
            }
            WorkerEvent::DetailLoaded {
                requested_name,
                result: Ok(details),
            } => {
                if let Some(package) = self
                    .catalog
                    .packages
                    .iter_mut()
                    .find(|package| package.name == requested_name)
                {
                    let installed = package.installed;
                    let outdated = package.outdated;
                    *package = details;
                    package.installed |= installed;
                    package.outdated |= outdated;
                }
            }
            WorkerEvent::DetailLoaded {
                requested_name,
                result: Err(_),
            } => {
                self.detail_requested.remove(&requested_name);
            }
            WorkerEvent::Log(line) => {
                self.log.push(line);
                if self.log.len() > 2_000 {
                    self.log.drain(..500);
                }
            }
            WorkerEvent::OperationStarted(summary) => self.status = format!("Running: {summary}"),
            WorkerEvent::OperationFinished { summary, success } => {
                self.log
                    .push(format!("{} {summary}", if success { "✓" } else { "✗" }));
                if !success {
                    self.failed_operations += 1;
                }
            }
            WorkerEvent::QueueFinished => {
                self.running = false;
                self.status = if self.failed_operations == 0 {
                    "Operations finished successfully · refreshing…".into()
                } else {
                    format!("Stopped after {} failed operation", self.failed_operations)
                };
                self.refresh();
            }
        }
    }

    pub fn staged_names(&self) -> HashSet<&str> {
        self.queue
            .iter()
            .filter_map(Operation::package_name)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn app_with_package() -> App {
        let (tx, _) = mpsc::channel();
        let mut app = App::new(tx);
        app.loading = false;
        app.catalog
            .packages
            .push(Package::new("jq".to_string(), PackageKind::Formula));
        app
    }

    #[test]
    fn printable_keys_search_without_entering_a_modal() {
        let mut app = app_with_package();
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert_eq!(app.query, "jq");
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn tab_selects_a_package_without_running_it() {
        let mut app = app_with_package();
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.queue.len(), 1);
        assert!(!app.running);
    }

    #[test]
    fn ctrl_q_is_the_only_control_key_that_quits() {
        let mut app = app_with_package();
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.should_quit);

        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }
}

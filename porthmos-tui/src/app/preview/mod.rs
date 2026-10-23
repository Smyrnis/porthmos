use std::sync::Arc;

use porthmos_core::preview::Preview;

use super::*;
use crate::widgets::preview_view::PreviewView;

impl App {
    pub(super) fn start_preview(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        let Some(location) = self.active_location() else {
            self.notifications.push(Severity::Warning, "Connect to a remote server first");
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        match panel.current_entry() {
            Some(entry) if entry.is_dir => self.notifications.push(Severity::Warning, "Can't preview a folder"),
            Some(entry) => {
                let (path, name) = (entry.path.clone(), entry.name.clone());
                self.next_preview_id += 1;
                self.preview = Some(PreviewView::loading(name));
                self.core.send(Command::Preview { request_id: self.next_preview_id, location, path });
            }
            None if panel.on_parent_row() => self.notifications.push(Severity::Warning, "Can't preview a folder"),
            None => {}
        }
    }

    pub(super) fn apply_preview_ready(&mut self, request_id: RequestId, result: Result<Arc<Preview>, String>) {
        if request_id != self.next_preview_id {
            return;
        }
        if let Some(view) = self.preview.as_mut().filter(|view| view.is_loading()) {
            view.finish(result);
        }
    }

    fn close_preview(&mut self) {
        if self.preview.as_ref().is_some_and(PreviewView::is_loading) {
            self.core.send(Command::CancelPreview);
        }
        self.preview = None;
    }

    pub(super) fn apply_preview_key(&mut self, key: KeyEvent) {
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | 'p') => self.close_preview(),
            _ => {
                let Some(view) = self.preview.as_mut() else {
                    return;
                };
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => view.scroll(-1),
                    KeyCode::Down | KeyCode::Char('j') => view.scroll(1),
                    KeyCode::PageUp | KeyCode::Char('b') => view.page(-1),
                    KeyCode::PageDown | KeyCode::Char(' ' | 'f') => view.page(1),
                    KeyCode::Home | KeyCode::Char('g') => view.go_top(),
                    KeyCode::End | KeyCode::Char('G') => view.go_bottom(),
                    KeyCode::Char('w') => view.toggle_wrap(),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

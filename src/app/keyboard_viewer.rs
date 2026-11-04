//! Entrada de teclado especializada para el visor de archivos.

use super::*;
use winit::keyboard::{Key, NamedKey, PhysicalKey, SmolStr};

impl TearminalApp {
    pub(super) fn handle_key_file_viewer(
        &mut self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        text: &Option<SmolStr>,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        // Si la pestaña activa es un visor de archivos, gestionar edición, navegación y atajos
        let is_viewer_active = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| t.file_viewer.is_some())
            .unwrap_or(false);
        if is_viewer_active {
            let win_h = self
                .window
                .as_ref()
                .map(|w| w.inner_size().height as usize)
                .unwrap_or(600);
            let top_offset = self.top_offset();
            let status_bar_h = self.status_bar_height();
            let row_h = self.renderer.as_ref().map(|r| r.row_height).unwrap_or(20);
            let visible_lines =
                ((win_h.saturating_sub(top_offset + status_bar_h + 38)) / row_h.max(16)).max(1);

            // Ajuste de línea global: Alt + Z
            if alt && !ctrl && !shift && key_is_char(logical_key, physical_key, 'z') {
                self.toggle_word_wrap();
                return true;
            }

            // 1. Guardar archivo: Ctrl + S
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 's') {
                self.save_active_viewer();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return true;
            }

            // 2. Cerrar visor o cancelar selección: Escape o Ctrl + W
            if matches!(logical_key, Key::Named(NamedKey::Escape)) {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    if viewer.search_open {
                        viewer.close_search();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if viewer.has_selection() {
                        viewer.clear_selection();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                }
                self.close_tab(self.active_tab_index);
                return true;
            }
            if ctrl && !shift && key_is_char(logical_key, physical_key, 'w') {
                self.close_tab(self.active_tab_index);
                return true;
            }

            // Búsqueda interna en el visor: Ctrl + F
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'f') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.open_search(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }

            // Ir a línea en el visor: Ctrl + G
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'g') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    if !viewer.is_binary {
                        viewer.open_goto();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
                return true;
            }

            // Interceptación de entrada de teclado cuando la búsqueda interna está abierta
            if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                if viewer.search_open {
                    if viewer.goto_mode && matches!(logical_key, Key::Named(NamedKey::Enter)) {
                        viewer.goto_line(visible_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if matches!(logical_key, Key::Named(NamedKey::Enter)) {
                        if shift {
                            viewer.search_prev(visible_lines);
                        } else {
                            viewer.search_next(visible_lines);
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if matches!(logical_key, Key::Named(NamedKey::F3)) {
                        if shift {
                            viewer.search_prev(visible_lines);
                        } else {
                            viewer.search_next(visible_lines);
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if matches!(logical_key, Key::Named(NamedKey::ArrowUp)) {
                        viewer.search_prev(visible_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                    if matches!(logical_key, Key::Named(NamedKey::ArrowDown)) {
                        viewer.search_next(visible_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }

                    let mut input = std::mem::take(&mut viewer.search_input);
                    let (consumed, changed) = self.handle_text_input_key(
                        &mut input,
                        logical_key,
                        physical_key,
                        text.as_deref(),
                    );
                    if let Some(ref mut v) = self.tabs[self.active_tab_index].file_viewer {
                        v.search_input = input;
                        if changed && !v.goto_mode {
                            v.update_search_matches();
                            v.jump_to_active_match(visible_lines);
                        }
                    }
                    if consumed {
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return true;
                    }
                }
            }

            // Seleccionar todo: Ctrl + A
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'a') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.select_all();
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }

            // Deshacer: Ctrl + Z
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'z') {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.undo();
                    if auto_save {
                        viewer.schedule_autosave();
                    }
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }

            // Rehacer: Ctrl + Y o Ctrl + Shift + Z
            if (ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'y'))
                || (ctrl && shift && !alt && key_is_char(logical_key, physical_key, 'z'))
            {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.redo();
                    if auto_save {
                        viewer.schedule_autosave();
                    }
                    viewer.ensure_cursor_visible(visible_lines);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                return true;
            }

            // Cortar selección: Ctrl + X
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'x') {
                let auto_save = self.config.terminal.auto_save;
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    if !viewer.is_editable() {
                        return true;
                    }
                    if let Some(sel_text) = viewer.selected_text() {
                        super::clipboard::copy_text(&mut self.clipboard, &sel_text);
                        viewer.delete_selection();
                        if auto_save {
                            viewer.schedule_autosave();
                        }
                        viewer.ensure_cursor_visible(visible_lines);
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
                return true;
            }

            // Copiar selección o línea actual: Ctrl + C
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'c') {
                if let Some(ref viewer) = self.tabs[self.active_tab_index].file_viewer {
                    let text = if let Some(sel_text) = viewer.selected_text() {
                        Some(sel_text)
                    } else {
                        viewer
                            .lines
                            .get(viewer.cursor_line)
                            .map(|line| format!("{}\n", line))
                    };
                    if let Some(txt) = text {
                        super::clipboard::copy_text(&mut self.clipboard, &txt);
                    }
                }
                return true;
            }

            // Alternar modo solo lectura / edición: Ctrl + E
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'e') {
                if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                    viewer.toggle_read_only();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }
            }

            // Pegar texto del portapapeles: Ctrl + V (el modo lectura lo bloquea el propio visor)
            if ctrl && !shift && !alt && key_is_char(logical_key, physical_key, 'v') {
                self.paste_from_clipboard();
                return true;
            }

            // 3. Edición de texto y navegación de cursor
            let global_auto_save = self.config.terminal.auto_save;
            if let Some(ref mut viewer) = self.tabs[self.active_tab_index].file_viewer {
                if viewer.is_side_by_side() {
                    let max_offset = viewer.total_display_rows().saturating_sub(visible_lines);
                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            if ctrl {
                                viewer.scroll_offset = 0;
                            } else {
                                viewer.scroll_offset = viewer.scroll_offset.saturating_sub(1);
                            }
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            if ctrl {
                                viewer.scroll_offset = max_offset;
                            } else {
                                viewer.scroll_offset = (viewer.scroll_offset + 1).min(max_offset);
                            }
                        }
                        Key::Named(NamedKey::PageUp) => {
                            viewer.scroll_offset =
                                viewer.scroll_offset.saturating_sub(visible_lines);
                        }
                        Key::Named(NamedKey::PageDown) => {
                            viewer.scroll_offset =
                                (viewer.scroll_offset + visible_lines).min(max_offset);
                        }
                        Key::Named(NamedKey::Home) => {
                            viewer.scroll_offset = 0;
                        }
                        Key::Named(NamedKey::End) => {
                            viewer.scroll_offset = max_offset;
                        }
                        _ => {}
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return true;
                }

                if !viewer.is_binary {
                    let prev_pos = (viewer.cursor_line, viewer.cursor_col);
                    let is_shift_nav = shift && !alt;

                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            if ctrl {
                                // Ctrl + Arriba: al inicio del archivo
                                viewer.cursor_line = 0;
                                viewer.cursor_col = 0;
                            } else if viewer.cursor_line > 0 {
                                viewer.cursor_line -= 1;
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if viewer.cursor_col > line_len {
                                viewer.cursor_col = line_len;
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            if ctrl {
                                // Ctrl + Abajo: al final del archivo
                                viewer.cursor_line = viewer.lines.len().saturating_sub(1);
                                viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                            } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                viewer.cursor_line += 1;
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if viewer.cursor_col > line_len {
                                viewer.cursor_col = line_len;
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            if ctrl {
                                if viewer.cursor_col > 0 {
                                    viewer.cursor_col = viewer.find_word_boundary_left();
                                } else if viewer.cursor_line > 0 {
                                    viewer.cursor_line -= 1;
                                    viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                                }
                            } else if viewer.cursor_col > 0 {
                                viewer.cursor_col -= 1;
                            } else if viewer.cursor_line > 0 {
                                viewer.cursor_line -= 1;
                                viewer.cursor_col = viewer.line_len(viewer.cursor_line);
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            let line_len = viewer.line_len(viewer.cursor_line);
                            if ctrl {
                                if viewer.cursor_col < line_len {
                                    viewer.cursor_col = viewer.find_word_boundary_right();
                                } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                    viewer.cursor_line += 1;
                                    viewer.cursor_col = 0;
                                }
                            } else {
                                if viewer.cursor_col < line_len {
                                    viewer.cursor_col += 1;
                                } else if viewer.cursor_line + 1 < viewer.lines.len() {
                                    viewer.cursor_line += 1;
                                    viewer.cursor_col = 0;
                                }
                            }
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::Home) => {
                            if ctrl {
                                viewer.cursor_line = 0;
                            }
                            viewer.cursor_col = 0;
                            viewer.ensure_cursor_visible(visible_lines);
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::End) => {
                            if ctrl {
                                viewer.cursor_line = viewer.lines.len().saturating_sub(1);
                            }
                            let line_len = viewer.line_len(viewer.cursor_line);
                            viewer.cursor_col = line_len;
                            viewer.ensure_cursor_visible(visible_lines);
                            if is_shift_nav {
                                let start = viewer.selection.map(|s| s.0).unwrap_or(prev_pos);
                                viewer.selection =
                                    Some((start, (viewer.cursor_line, viewer.cursor_col)));
                            } else {
                                viewer.clear_selection();
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::PageUp) => {
                            let page = visible_lines.saturating_sub(2).max(1);
                            viewer.cursor_line = viewer.cursor_line.saturating_sub(page);
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            let page = visible_lines.saturating_sub(2).max(1);
                            viewer.cursor_line = (viewer.cursor_line + page)
                                .min(viewer.lines.len().saturating_sub(1));
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::Enter) => {
                            if viewer.is_read_only {
                                return true;
                            }
                            viewer.insert_newline();
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            if viewer.is_read_only {
                                return true;
                            }
                            if ctrl {
                                viewer.delete_word_left();
                            } else {
                                viewer.backspace();
                            }
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::Delete) => {
                            if viewer.is_read_only {
                                return true;
                            }
                            if ctrl {
                                viewer.delete_word_right();
                            } else {
                                viewer.delete();
                            }
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::Tab) => {
                            if viewer.is_read_only {
                                return true;
                            }
                            viewer.insert_tab();
                            if global_auto_save {
                                viewer.schedule_autosave();
                            }
                            viewer.ensure_cursor_visible(visible_lines);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        _ => {
                            // Insertar caracteres de texto imprimible
                            if !ctrl && !alt {
                                if viewer.is_read_only {
                                    return true;
                                }
                                if let Some(ref t) = text {
                                    for ch in t.chars() {
                                        if !ch.is_control() {
                                            viewer.insert_char(ch);
                                        }
                                    }
                                    if global_auto_save {
                                        viewer.schedule_autosave();
                                    }
                                    viewer.ensure_cursor_visible(visible_lines);
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                } else {
                    // Modo de solo lectura binario
                    match logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            viewer.scroll_offset = viewer.scroll_offset.saturating_sub(1);
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            viewer.scroll_offset = (viewer.scroll_offset + 1)
                                .min(viewer.rows().len().saturating_sub(1));
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            return true;
        }

        false
    }
}

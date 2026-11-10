//! Estado del visor/editor de archivos en pestaña: texto, filas visuales, plegado y selección.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ViewerSnapshot {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
}

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SelectionStats {
    pub(crate) chars: usize,
    pub(crate) lines: usize,
}

/// Fila visual del visor: un tramo [start, end) de caracteres de una línea (`end == usize::MAX` = hasta el final).
/// El ajuste de línea parte una línea en varias filas y el plegado oculta líneas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RowRef {
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RowsCache {
    key: (u64, u64, usize, usize, bool, bool),
    rows: Rc<[RowRef]>,
    max_cols_gen: u64,
    max_cols: usize,
    config: (u64, usize, usize, bool, bool),
    lines_len: usize,
}

use unicode_segmentation::UnicodeSegmentation;

#[inline]
pub(super) fn char_byte_idx(s: &str, col: usize) -> usize {
    if s.is_ascii() {
        col.min(s.len())
    } else {
        s.grapheme_indices(true)
            .nth(col)
            .map_or(s.len(), |(b, _)| b)
    }
}

#[inline]
pub(super) fn char_count(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.graphemes(true).count()
    }
}

#[inline]
fn line_col_width(s: &str) -> usize {
    if s.is_ascii() {
        let tabs = s.bytes().filter(|&b| b == b'\t').count();
        s.len() + tabs * 3
    } else {
        use unicode_width::UnicodeWidthStr;
        s.graphemes(true)
            .map(|g| if g == "\t" { 4 } else { g.width().max(1) })
            .sum()
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let n = line.chars().take_while(|c| *c == '#').count();
    ((1..=6).contains(&n) && line[n..].starts_with(' ')).then_some(n)
}

pub(crate) fn indent_of(line: &str) -> Option<usize> {
    (!line.trim().is_empty()).then(|| {
        line.chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum()
    })
}

/// Sangría colgante para líneas continuadas de listas o citas en vista previa de Markdown
fn md_hanging_indent(line: &str) -> usize {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.starts_with("[x] ") || rest.starts_with("[X] ") || rest.starts_with("[ ] ") {
                return indent + 6;
            }
            return indent + 2;
        }
    }
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && trimmed[digits..].starts_with(". ") {
        return indent + digits + 2;
    }
    if trimmed.starts_with('>') {
        let depth = trimmed.chars().take_while(|c| *c == '>').count();
        return indent + depth * 2;
    }
    if indent > 0 {
        return indent;
    }
    0
}

/// Resuelve una ruta de imagen relativa al documento Markdown actual en el disco local.
pub(crate) fn resolve_image_path(
    doc_path: &std::path::Path,
    src: &str,
) -> Option<std::path::PathBuf> {
    let clean = src.split(&['?', '#'][..]).next().unwrap_or(src).trim();
    if clean.is_empty() || clean.starts_with("http://") || clean.starts_with("https://") {
        return None;
    }
    let decoded = crate::terminal::percent_decode(clean);
    let p = std::path::Path::new(&decoded);
    if p.is_absolute() {
        Some(p.to_path_buf())
    } else {
        doc_path.parent().map(|dir| dir.join(p))
    }
}

/// Parte `line` en tramos con ancho configurable para el primer tramo y los continuados.
fn wrap_line_with_indent(
    line: &str,
    first_cols: usize,
    cont_cols: usize,
    line_idx: usize,
    out: &mut Vec<RowRef>,
) {
    let first_cols = first_cols.max(8);
    let cont_cols = cont_cols.max(8);
    if line_col_width(line) <= first_cols {
        out.push(RowRef {
            line: line_idx,
            start: 0,
            end: usize::MAX,
        });
        return;
    }
    let (mut start, mut width, mut last_break) = (0usize, 0usize, None::<(usize, usize)>);
    let mut is_first = true;
    for (i, g) in line.graphemes(true).enumerate() {
        let cols = if is_first { first_cols } else { cont_cols };
        let adv = if g == "\t" {
            4
        } else {
            use unicode_width::UnicodeWidthStr;
            g.width().max(1)
        };
        if width + adv > cols && i > start {
            let (at, rem_width) = match last_break {
                Some((b, w_at_b)) if b > start + cols / 2 => (b, width - w_at_b),
                _ => (i, 0),
            };
            out.push(RowRef {
                line: line_idx,
                start,
                end: at,
            });
            start = at;
            is_first = false;
            last_break = None;
            width = rem_width;
        }
        width += adv;
        if g.chars().all(char::is_whitespace) {
            last_break = Some((i + 1, width));
        }
    }
    out.push(RowRef {
        line: line_idx,
        start,
        end: usize::MAX,
    });
}

/// Parte `line` en tramos de como mucho `cols` columnas, prefiriendo cortar tras un espacio.
fn wrap_line(line: &str, cols: usize, line_idx: usize, out: &mut Vec<RowRef>) {
    wrap_line_with_indent(line, cols, cols, line_idx, out);
}

pub(crate) use super::viewer_image::*;

#[derive(Debug, Clone)]
pub(crate) struct FileViewerState {
    pub path: std::path::PathBuf,
    pub file_name: String,
    pub lines: Vec<String>,
    pub scroll_offset: usize,
    pub scroll_offset_x: usize,
    pub is_binary: bool,
    /// Si el archivo es una imagen: se dibuja en lugar del texto (`is_binary` queda activo para el resto de rutas)
    pub image: Option<Rc<ViewerImage>>,
    pub file_size: u64,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub is_dirty: bool,
    pub undo_stack: Vec<ViewerSnapshot>,
    pub redo_stack: Vec<ViewerSnapshot>,
    pub original_lines: Vec<String>,
    /// Contenido en `HEAD` (base del diff del margen). `None` fuera de un repo: se compara con el texto al abrir/guardar.
    pub git_base: Option<Vec<String>>,
    /// Archivo nuevo en un repositorio Git (untracked o recién añadido, sin versión previa en `HEAD`).
    pub git_new_file: bool,
    /// Modo de diff activo (Lado a lado, Unificado, o ninguno)
    pub diff_mode: DiffMode,
    /// Con el diff unificado «HEAD vs ahora» a la vista: guarda el estado real del visor para volver a él
    pub diff_stash: Option<Box<DiffStash>>,
    pub hovered_diff_btn: bool,
    pub hovered_diff_mode_btn: bool,
    /// Filas sincronizadas de diff lado a lado en caché por `edit_gen`
    pub side_by_side_cache: RefCell<Option<(u64, Rc<[SideBySideRow]>)>>,
    /// Marca de cada línea frente a la base y si existen cambios, en caché por `edit_gen`
    pub git_marks: RefCell<Option<(u64, bool, GitMarks)>>,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub remote_destination: Option<String>,
    pub remote_branch: Option<String>,
    pub is_read_only: bool,
    /// Fin de línea y salto final del archivo al abrirlo: se conservan al guardar
    pub crlf: bool,
    pub final_newline: bool,
    /// El archivo no era UTF-8: se muestra con sustitutos y nunca se escribe de vuelta
    pub non_utf8: bool,
    /// Último fallo de guardado (se muestra en la cabecera hasta que un guardado funcione)
    pub save_error: Option<String>,
    pub render_markdown: bool,
    pub hovered_mode_btn: bool,
    pub hovered_md_btn: bool,
    pub hovered_scrollbar: bool,
    pub hovered_hscrollbar: bool,
    pub hovered_gutter: bool,
    /// Columnas de texto visibles (las fija el render según el ancho real de la pestaña)
    pub visible_cols: usize,
    /// Altura de fila en píxeles (la sincroniza el render para el cálculo de bloques visuales)
    pub row_height: usize,
    /// Imágenes Markdown en caché: mapa de URL -> ViewerImage decodificada
    pub md_images: RefCell<rustc_hash::FxHashMap<String, Option<Rc<ViewerImage>>>>,
    /// Ajuste de línea (lo sincroniza el render desde la configuración global)
    pub word_wrap: bool,
    /// Líneas plegadas (primera línea de cada región oculta)
    pub folded: BTreeSet<usize>,
    /// Fecha de modificación del archivo local al abrirlo o guardarlo (`None` si es remoto o aún no existe):
    /// si en disco es otra, un agente u otro programa lo cambió y no se sobrescribe sin avisar
    pub disk_mtime: Option<std::time::SystemTime>,
    pub needs_autosave: bool,
    pub last_edit_time: Instant,
    pub edit_gen: u64,
    pub fold_gen: u64,
    pub rows_cache: RefCell<Option<RowsCache>>,
    /// Línea editada recientemente para actualización incremental O(1) de filas visuales
    pub edited_line: Option<usize>,
    /// Barra flotante de búsqueda interna (`Ctrl + F`)
    pub search_open: bool,
    /// La misma barra, en modo «Ir a línea» (`Ctrl + G`): `search_input` es `línea[:columna]`, sin coincidencias
    pub goto_mode: bool,
    pub search_input: crate::ui::TextInput,
    /// Coincidencias de búsqueda: `(line_idx, start_col, end_col)` en coordenadas de grafemas
    pub search_matches: Vec<(usize, usize, usize)>,
    pub search_active_idx: Option<usize>,
    pub hovered_search_prev: bool,
    pub hovered_search_next: bool,
    pub hovered_search_close: bool,
}

/// Texto de un archivo tal como lo ve el visor, más lo necesario para escribirlo igual que se leyó
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DecodedText {
    pub lines: Vec<String>,
    pub crlf: bool,
    pub final_newline: bool,
    pub non_utf8: bool,
}

impl DecodedText {
    /// Decodifica texto desde cualquier flujo de lectura genérico (std::io::Read)
    #[cfg(test)]
    pub(crate) fn from_reader(mut reader: impl std::io::Read) -> std::io::Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(Self::decode(&bytes))
    }

    pub(crate) fn decode(bytes: &[u8]) -> Self {
        let (text, non_utf8) = match std::str::from_utf8(bytes) {
            Ok(t) => (std::borrow::Cow::Borrowed(t), false),
            Err(_) => (String::from_utf8_lossy(bytes), true),
        };
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            crlf: text.find('\n').is_some_and(|i| text[..i].ends_with('\r')),
            final_newline: text.ends_with('\n'),
            non_utf8,
        }
    }
}

impl From<&[u8]> for DecodedText {
    #[inline]
    fn from(bytes: &[u8]) -> Self {
        Self::decode(bytes)
    }
}

impl FileViewerState {
    /// Única puerta de las mutaciones: en modo lectura (o binario) el texto no puede cambiar por ninguna vía.
    pub(crate) fn is_editable(&self) -> bool {
        !self.is_binary && !self.is_read_only && !self.non_utf8 && !self.is_diff()
    }

    /// Alterna lectura/edición; un archivo que no es UTF-8 no puede pasar a edición (se corrompería al guardar)
    pub(crate) fn toggle_read_only(&mut self) {
        if !self.non_utf8 {
            self.is_read_only = !self.is_read_only;
        }
    }

    pub(crate) fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
    }

    /// El ajuste de línea está activo si se solicitó explícitamente o en vista previa de Markdown
    pub(crate) fn wrap_active(&self) -> bool {
        if self.is_side_by_side() {
            return false;
        }
        self.word_wrap || (self.render_markdown && self.is_markdown())
    }

    /// Detecta si el archivo utiliza sangría de 2 o 4 espacios (por defecto 4).
    pub(crate) fn indent_unit(&self) -> usize {
        let mut count_2 = 0;
        let mut count_4 = 0;
        for line in self.lines.iter().take(150) {
            if let Some(ind) = indent_of(line) {
                if ind > 0 {
                    if ind % 4 != 0 && ind % 2 == 0 {
                        count_2 += 1;
                    } else if ind % 4 == 0 {
                        count_4 += 1;
                    }
                    if count_4 >= 20 || count_2 >= 20 {
                        break;
                    }
                }
            }
        }
        if count_2 > count_4 {
            2
        } else {
            4
        }
    }

    /// Obtiene o carga de disco una imagen local referenciada en un documento Markdown.
    pub(crate) fn get_or_load_md_image(&self, src: &str) -> Option<Rc<ViewerImage>> {
        if let Some(cached) = self.md_images.borrow().get(src) {
            return cached.clone();
        }
        let img = resolve_image_path(&self.path, src)
            .filter(|p| p.is_file())
            .and_then(|resolved| {
                let bytes = std::fs::read(&resolved).ok()?;
                ViewerImage::decode(&resolved, &bytes)
            });
        self.md_images
            .borrow_mut()
            .insert(src.to_string(), img.clone());
        img
    }

    /// Calcula la cantidad de filas visuales que ocupará una imagen en el visor de Markdown.
    pub(crate) fn calc_md_image_rows(&self, img_w: u32, img_h: u32) -> usize {
        let row_h = self.row_height.max(16);
        let avail_w = (self.visible_cols * 8).clamp(120, 800);
        let max_h = 360usize;
        let scale = (avail_w as f32 / img_w as f32)
            .min(max_h as f32 / img_h as f32)
            .min(1.0);
        let display_h = ((img_h as f32 * scale).round() as usize).max(32);
        (display_h.div_ceil(row_h) + 1).clamp(2, 24)
    }

    /// Filas visuales (ajuste de línea + plegado), en caché mientras no cambien texto, pliegues, ancho ni modo.
    pub(crate) fn rows(&self) -> Rc<[RowRef]> {
        let wrap = self.wrap_active();
        let is_md_preview = self.render_markdown && self.is_markdown();
        let key = (
            self.edit_gen,
            self.fold_gen,
            self.visible_cols,
            self.row_height,
            wrap,
            is_md_preview,
        );
        let config = (
            self.fold_gen,
            self.visible_cols,
            self.row_height,
            wrap,
            is_md_preview,
        );

        // 1. Si la clave coincide exactamente o se puede actualizar de forma incremental
        let incremental = {
            let borrow = self.rows_cache.borrow();
            if let Some(c) = &*borrow {
                if c.key == key {
                    return c.rows.clone();
                }
                if c.config == config
                    && c.lines_len == self.lines.len()
                    && self.folded.is_empty()
                    && self.edited_line.is_some()
                {
                    Some((c.rows.clone(), c.max_cols))
                } else {
                    None
                }
            } else {
                None
            }
        };

        // 2. Ruta rápida incremental: si solo se editó una línea existente y la configuración
        // no cambió (sin pliegues activos ni cambios de tamaño), actualizar solo esa línea en O(1)
        if let Some((prev_rows, prev_max_cols)) = incremental {
            let edited_idx = self.edited_line.unwrap_or(0);
            if edited_idx < self.lines.len() {
                let r_start = prev_rows.partition_point(|r| r.line < edited_idx);
                let r_end = prev_rows.partition_point(|r| r.line <= edited_idx);

                let line = &self.lines[edited_idx];
                let is_table = is_md_preview && line.trim_start().starts_with('|');
                let is_img = is_md_preview
                    && crate::renderer::md_preview::extract_markdown_image(line).is_some();
                let mut line_rows = Vec::with_capacity(2);
                if is_img {
                    let img_info = crate::renderer::md_preview::extract_markdown_image(line)
                        .and_then(|(_, src)| self.get_or_load_md_image(&src));
                    let img_rows = if let Some(img) = img_info {
                        self.calc_md_image_rows(img.width, img.height)
                    } else {
                        1
                    };
                    for r in 0..img_rows {
                        line_rows.push(RowRef {
                            line: edited_idx,
                            start: r,
                            end: usize::MAX,
                        });
                    }
                } else if wrap && !is_table {
                    if is_md_preview {
                        let hanging = md_hanging_indent(line);
                        let cont_cols = self.visible_cols.saturating_sub(hanging).max(8);
                        wrap_line_with_indent(
                            line,
                            self.visible_cols,
                            cont_cols,
                            edited_idx,
                            &mut line_rows,
                        );
                    } else {
                        wrap_line(line, self.visible_cols, edited_idx, &mut line_rows);
                    }
                } else {
                    line_rows.push(RowRef {
                        line: edited_idx,
                        start: 0,
                        end: usize::MAX,
                    });
                }

                let mut updated_rows = prev_rows.to_vec();
                updated_rows.splice(r_start..r_end, line_rows);
                if updated_rows.is_empty() {
                    updated_rows.push(RowRef {
                        line: 0,
                        start: 0,
                        end: usize::MAX,
                    });
                }

                let edited_w = line_col_width(line);
                let max_cols = if edited_w >= prev_max_cols {
                    edited_w
                } else {
                    prev_max_cols
                };

                let rows: Rc<[RowRef]> = Rc::from(updated_rows);
                *self.rows_cache.borrow_mut() = Some(RowsCache {
                    key,
                    rows: rows.clone(),
                    max_cols_gen: self.edit_gen,
                    max_cols,
                    config,
                    lines_len: self.lines.len(),
                });
                return rows;
            }
        }

        // 3. Reconstrucción completa (apertura, redimensionado, cambios multilínea o pliegues)
        let prev_max_cols = {
            let borrow = self.rows_cache.borrow();
            borrow
                .as_ref()
                .and_then(|c| (c.max_cols_gen == self.edit_gen).then_some(c.max_cols))
        };
        let mut rows = Vec::with_capacity(self.lines.len());
        let mut i = 0;
        while i < self.lines.len() {
            let line = &self.lines[i];
            let is_table = is_md_preview && line.trim_start().starts_with('|');
            let is_img = is_md_preview
                && crate::renderer::md_preview::extract_markdown_image(line).is_some();
            if is_img {
                let img_info = crate::renderer::md_preview::extract_markdown_image(line)
                    .and_then(|(_, src)| self.get_or_load_md_image(&src));
                let img_rows = if let Some(img) = img_info {
                    self.calc_md_image_rows(img.width, img.height)
                } else {
                    1
                };
                for r in 0..img_rows {
                    rows.push(RowRef {
                        line: i,
                        start: r,
                        end: usize::MAX,
                    });
                }
            } else if wrap && !is_table {
                if is_md_preview {
                    let hanging = md_hanging_indent(line);
                    let cont_cols = self.visible_cols.saturating_sub(hanging).max(8);
                    wrap_line_with_indent(line, self.visible_cols, cont_cols, i, &mut rows);
                } else {
                    wrap_line(line, self.visible_cols, i, &mut rows);
                }
            } else {
                rows.push(RowRef {
                    line: i,
                    start: 0,
                    end: usize::MAX,
                });
            }
            i = match self.folded.contains(&i).then(|| self.fold_end(i)).flatten() {
                Some(end) => end + 1,
                None => i + 1,
            };
        }
        if rows.is_empty() {
            rows.push(RowRef {
                line: 0,
                start: 0,
                end: usize::MAX,
            });
        }
        let max_cols = prev_max_cols.unwrap_or_else(|| {
            self.lines
                .iter()
                .map(|l| line_col_width(l))
                .max()
                .unwrap_or(0)
        });
        let rows: Rc<[RowRef]> = Rc::from(rows);
        *self.rows_cache.borrow_mut() = Some(RowsCache {
            key,
            rows: rows.clone(),
            max_cols_gen: self.edit_gen,
            max_cols,
            config,
            lines_len: self.lines.len(),
        });
        rows
    }

    /// Texto del tramo de una fila visual
    pub(crate) fn row_text(&self, row: &RowRef) -> &str {
        let line = self.lines.get(row.line).map_or("", |l| l.as_str());
        if row.start == 0 && row.end == usize::MAX {
            return line;
        }
        let from = char_byte_idx(line, row.start);
        let to = if row.end == usize::MAX {
            line.len()
        } else {
            char_byte_idx(line, row.end)
        };
        &line[from..to]
    }

    /// (línea, columna) bajo la fila visual `row_idx` y la coordenada X del ratón
    pub(crate) fn hit_test(
        &self,
        row_idx: usize,
        mx: usize,
        lay: &crate::renderer::FileViewerLayout,
    ) -> Option<(usize, usize)> {
        let rows = self.rows();
        let row = rows.get(row_idx).or(rows.last())?;
        if self.render_markdown
            && self.is_markdown()
            && self
                .lines
                .get(row.line)
                .is_some_and(|l| crate::renderer::md_preview::extract_markdown_image(l).is_some())
        {
            return Some((row.line, 0));
        }
        let seg = self.row_text(row);
        let scroll_x = if self.wrap_active() {
            0
        } else {
            self.scroll_offset_x
        };
        let mut col = lay.col_at(seg, mx, scroll_x);
        if row.end != usize::MAX {
            col = col.min(char_count(seg));
        }
        Some((row.line, row.start + col))
    }

    /// Índice de la fila visual que contiene (línea, columna); `None` si la línea está plegada
    pub(crate) fn row_of(&self, line: usize, col: usize) -> Option<usize> {
        self.rows()
            .iter()
            .rposition(|r| r.line == line && r.start <= col)
    }

    /// Última línea de la región plegable que empieza en `start` (títulos en Markdown, sangría en el resto)
    pub(crate) fn fold_end(&self, start: usize) -> Option<usize> {
        let line = self.lines.get(start)?;
        if self.is_markdown() {
            if let Some(level) = heading_level(line) {
                let mut end = start;
                for (i, cur_line) in self.lines.iter().enumerate().skip(start + 1) {
                    if heading_level(cur_line).is_some_and(|l| l <= level) {
                        break;
                    }
                    if !cur_line.trim().is_empty() {
                        end = i;
                    }
                }
                return (end > start).then_some(end);
            }
        }
        let base = indent_of(line)?;
        let mut end = None;
        for (i, cur_line) in self.lines.iter().enumerate().skip(start + 1) {
            match indent_of(cur_line) {
                None => continue,
                Some(ind) if ind > base => end = Some(i),
                _ => break,
            }
        }
        end
    }

    /// Plega o despliega la región que empieza en `line`
    pub(crate) fn toggle_fold(&mut self, line: usize) {
        if !self.folded.remove(&line) {
            let Some(end) = self.fold_end(line) else {
                return;
            };
            self.folded.insert(line);
            if self.cursor_line > line && self.cursor_line <= end {
                self.cursor_line = line;
                self.cursor_col = 0;
            }
            self.selection = None;
        }
        self.fold_gen += 1;
        self.edited_line = None;
    }

    pub(crate) fn fold_all(&mut self) {
        let starts: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, _)| self.fold_end(i).is_some().then_some(i))
            .collect();
        self.folded.extend(starts);
        self.fold_gen += 1;
        self.edited_line = None;
        self.selection = None;
        // si el cursor quedó oculto, sube al encabezado visible más externo que lo contiene
        let hidden_by = self.folded.iter().copied().find(|&s| {
            s < self.cursor_line && self.fold_end(s).is_some_and(|e| self.cursor_line <= e)
        });
        if let Some(top) = hidden_by {
            self.cursor_line = top;
            self.cursor_col = 0;
        }
        self.scroll_offset = self.scroll_offset.min(self.rows().len().saturating_sub(1));
    }

    pub(crate) fn unfold_all(&mut self) {
        if !self.folded.is_empty() {
            self.folded.clear();
            self.fold_gen += 1;
            self.edited_line = None;
        }
    }

    /// Despliega las regiones que ocultan `line` (el cursor nunca queda en una línea oculta)
    pub(crate) fn reveal_line(&mut self, line: usize) {
        let hiding: Vec<usize> = self
            .folded
            .iter()
            .copied()
            .filter(|&s| s < line && self.fold_end(s).is_some_and(|e| line <= e))
            .collect();
        if !hiding.is_empty() {
            for s in hiding {
                self.folded.remove(&s);
            }
            self.fold_gen += 1;
        }
    }

    pub(crate) fn selection_ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let (p1, p2) = self.selection?;
        if p1 <= p2 {
            Some((p1, p2))
        } else {
            Some((p2, p1))
        }
    }

    /// Ancho en columnas de la línea más larga (tabulador = 4), para el scroll horizontal.
    pub(crate) fn max_cols(&self) -> usize {
        if let Some(c) = &*self.rows_cache.borrow() {
            if c.max_cols_gen == self.edit_gen {
                return c.max_cols;
            }
        }
        let mut max = self
            .lines
            .iter()
            .map(|l| line_col_width(l))
            .max()
            .unwrap_or(0);
        if self.is_side_by_side() {
            if let Some(base) = &self.git_base {
                if let Some(base_max) = base.iter().map(|l| line_col_width(l)).max() {
                    max = max.max(base_max);
                }
            }
        }
        if let Some(c) = &mut *self.rows_cache.borrow_mut() {
            c.max_cols_gen = self.edit_gen;
            c.max_cols = max;
        }
        max
    }

    /// Máximo `scroll_offset_x`: deja 2 columnas libres para el cursor al final de la línea más larga.
    /// Con ajuste de línea activo (`wrap_active`), todo el contenido se adapta al ancho y no hay scroll horizontal.
    pub(crate) fn max_scroll_x(&self) -> usize {
        if self.wrap_active() {
            return 0;
        }
        (self.max_cols() + 2).saturating_sub(self.visible_cols.max(1))
    }

    /// Resumen de la selección: caracteres y líneas (sin alojar memoria)
    pub(crate) fn selection_stats(&self) -> Option<SelectionStats> {
        let ((l1, c1), (l2, c2)) = self.selection_ordered()?;
        if (l1 == l2 && c1 == c2) || l1 >= self.lines.len() {
            return None;
        }
        let mut chars = 0usize;
        let mut parts = 0usize;
        for line_idx in l1..=l2.min(self.lines.len().saturating_sub(1)) {
            let line = &self.lines[line_idx];
            let char_len = char_count(line);
            let start = if line_idx == l1 { c1.min(char_len) } else { 0 };
            let end = if line_idx == l2 {
                c2.min(char_len)
            } else {
                char_len
            };
            if start < end {
                chars += end - start;
                parts += 1;
            } else if line_idx != l2 {
                parts += 1;
            }
        }
        chars += parts.saturating_sub(1);
        Some(SelectionStats {
            chars,
            lines: l2 - l1 + 1,
        })
    }

    pub(crate) fn has_selection(&self) -> bool {
        if let Some((p1, p2)) = self.selection_ordered() {
            p1 != p2
        } else {
            false
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let ((l1, c1), (l2, c2)) = self.selection_ordered()?;
        if l1 == l2 && c1 == c2 {
            return None;
        }
        if l1 >= self.lines.len() {
            return None;
        }

        let end_idx = l2.min(self.lines.len().saturating_sub(1));
        let num_lines = (end_idx + 1).saturating_sub(l1);
        let mut res = Vec::with_capacity(num_lines);
        for line_idx in l1..=end_idx {
            let line = &self.lines[line_idx];
            let len = char_count(line);
            let start = if line_idx == l1 { c1.min(len) } else { 0 };
            let end = if line_idx == l2 { c2.min(len) } else { len };

            if start < end {
                let s = &line[char_byte_idx(line, start)..char_byte_idx(line, end)];
                res.push(s);
            } else if line_idx != l2 {
                res.push("");
            }
        }
        Some(res.join("\n"))
    }
}

pub(crate) use super::viewer_diff::*;

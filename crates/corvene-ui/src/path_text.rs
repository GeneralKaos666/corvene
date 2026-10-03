//! GHD `PathText` (`ui/lib/path-text.tsx`): a repository path drawn as a
//! dimmed directory and the file name, shortened by characters from the
//! middle of the directory (`truncatePath`, keeping the file name and the
//! last separator) to the longest form that fits the row's width, found by
//! measuring like GHD's `resizeIfNecessary` binary search.
//!
//! Deviation: GHD shows the full path in a tooltip when it is truncated;
//! Corvene's rows have no truncation tooltip.

use gpui_kit::*;

/// GHD `truncateMid(value, length)`: `value` cut to `length` characters by
/// replacing its middle with an ellipsis (`foo bar`, 6 → `fo…bar`); empty
/// for zero or less, `…` for one.
pub fn truncate_mid(value: &str, length: isize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if isize::try_from(chars.len()).is_ok_and(|n| n <= length) {
        return value.to_string();
    }
    if length <= 0 {
        return String::new();
    }
    if length == 1 {
        return "…".into();
    }
    let mid = (length - 1) as f64 / 2.;
    let pre = mid.floor() as usize;
    let post = mid.ceil() as usize;
    let mut out: String = chars[..pre].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - post..]);
    out
}

/// GHD `truncatePath(path, length)`: `path` cut to `length` characters,
/// taking them out of the directory part (`alfa/bravo/…/delta.txt`) while
/// the file name and `…` plus the last separator fit, else
/// [`truncate_mid`]. Uses the platform's separator (`Path.sep`).
pub fn truncate_path(path: &str, length: isize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if isize::try_from(chars.len()).is_ok_and(|n| n <= length) {
        return path.to_string();
    }
    if length <= 0 {
        return String::new();
    }
    if length == 1 {
        return "…".into();
    }
    let Some(last_separator) = chars.iter().rposition(|c| *c == std::path::MAIN_SEPARATOR) else {
        // no directory prefix: middle ellipsis
        return truncate_mid(path, length);
    };
    let file_name_length = chars.len() - last_separator - 1;
    // the file name after `…/` would be too long: middle ellipsis
    let Some(pre) = (length as usize).checked_sub(file_name_length + 2) else {
        return truncate_mid(path, length);
    };
    let mut out: String = chars[..pre].iter().collect();
    out.push('…');
    out.extend(&chars[last_separator..]);
    out
}

/// GHD `createPathDisplayState(normalizedPath, length)`: the directory and
/// file name texts of `directory` + `file_name` cut to `length`
/// characters. The directory text runs as far as the truncated path
/// matches the directory, plus a `…` and the separator after it.
pub fn display_state(directory: &str, file_name: &str, length: usize) -> (String, String) {
    let path = format!("{directory}{file_name}");
    let total = path.chars().count();
    if length == 0 {
        return (String::new(), String::new());
    }
    if length >= total {
        return (directory.to_string(), file_name.to_string());
    }
    let truncated: Vec<char> = truncate_path(&path, isize::try_from(length).unwrap_or(isize::MAX))
        .chars()
        .collect();
    let mut directory_length = 0;
    for (normalized, truncated_char) in directory.chars().zip(truncated.iter().copied()) {
        if normalized == truncated_char {
            directory_length += 1;
            continue;
        }
        // `…` and `…/` count towards the directory, an aesthetic choice
        if truncated_char == '…' {
            directory_length += 1;
            if truncated.get(directory_length) == Some(&std::path::MAIN_SEPARATOR) {
                directory_length += 1;
            }
        }
        break;
    }
    (
        truncated[..directory_length].iter().collect(),
        truncated[directory_length..].iter().collect(),
    )
}

/// A [`PathText`] for a path split into its directory (with the trailing
/// separator, platform separators) and file name, bolding the characters
/// at `matches` (char indices into the whole path, GHD `matches.title`),
/// the directory in `directory_color`.
pub fn path_text(
    directory: impl Into<String>,
    file_name: impl Into<String>,
    matches: Vec<usize>,
    directory_color: Hsla,
) -> PathText {
    PathText {
        directory: directory.into(),
        file_name: file_name.into(),
        matches,
        directory_color,
        half: false,
    }
}

/// GHD `PathText`; see [`path_text`].
pub struct PathText {
    directory: String,
    file_name: String,
    matches: Vec<usize>,
    directory_color: Hsla,
    /// One side of a rename (`PathLabel`'s `segmentWidth`): at most half
    /// the label's width.
    half: bool,
}

impl PathText {
    /// At most half the parent's width, as each side of a rename in GHD
    /// `PathLabel` (`availableWidth / 2 - ResizeArrowPadding`).
    pub fn half(mut self, half: bool) -> Self {
        self.half = half;
        self
    }

    /// The text, colour and bold runs for `length` characters of the path.
    fn runs(&self, length: usize, base: &TextStyle) -> (SharedString, Vec<TextRun>) {
        let total = self.directory.chars().count() + self.file_name.chars().count();
        let (directory, file) = display_state(&self.directory, &self.file_name, length);
        let directory_length = directory.chars().count();
        let file_length = file.chars().count();
        let file_offset = total - file_length;
        let text = format!("{directory}{file}");
        let mut runs: Vec<TextRun> = Vec::new();
        let mut last: Option<(bool, bool)> = None;
        for (ix, c) in text.chars().enumerate() {
            let in_directory = ix < directory_length;
            // GHD maps the matches onto the shown directory text as they
            // are, and onto the file name from its end
            let bold = if in_directory {
                self.matches.contains(&ix)
            } else {
                self.matches
                    .contains(&(ix - directory_length + file_offset))
            };
            let key = (in_directory, bold);
            match runs.last_mut() {
                Some(run) if last == Some(key) => run.len += c.len_utf8(),
                _ => {
                    let mut run = base.to_run(c.len_utf8());
                    if in_directory {
                        run.color = self.directory_color;
                    }
                    if bold {
                        run.font.weight = FontWeight::BOLD;
                    }
                    runs.push(run);
                }
            }
            last = Some(key);
        }
        (text.into(), runs)
    }
}

impl IntoElement for PathText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PathText {
    /// The line height and the whole path's width.
    type RequestLayoutState = (Pixels, Pixels);
    /// The shaped (possibly shortened) path.
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = window.text_style();
        let rem = window.rem_size();
        let font_size = style.font_size.to_pixels(rem);
        let line_height = style.line_height_in_pixels(rem);
        let path: SharedString = format!("{}{}", self.directory, self.file_name).into();
        let width = window
            .text_system()
            .shape_line(path.clone(), font_size, &[style.to_run(path.len())], None)
            .width;
        // `.path-text-component`: shrinks with the row (`min-width: 0`)
        let mut layout = Style {
            flex_shrink: 1.,
            ..Default::default()
        };
        layout.min_size.width = px(0.).into();
        if self.half {
            layout.max_size.width = relative(0.5).into();
        }
        let layout_id = window.request_measured_layout(layout, move |known, _, _, _| {
            size(known.width.unwrap_or(width), line_height)
        });
        (layout_id, (line_height, width))
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (_, full_width): &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        // layout snapping can take a fraction of a pixel off the box
        let available = bounds.size.width + px(0.5);
        let total = self.directory.chars().count() + self.file_name.chars().count();
        let length = if *full_width <= available {
            total
        } else {
            // the longest truncation that fits (GHD `resizeIfNecessary`)
            let fits = |length: usize, window: &mut Window| {
                let (text, _) = self.runs(length, &style);
                let run = style.to_run(text.len());
                window
                    .text_system()
                    .shape_line(text, font_size, &[run], None)
                    .width
                    <= available
            };
            let (mut fit, mut non_fit) = (0, total);
            while non_fit - fit > 1 {
                let mid = (fit + non_fit) / 2;
                if fits(mid, window) {
                    fit = mid;
                } else {
                    non_fit = mid;
                }
            }
            fit
        };
        let (text, runs) = self.runs(length, &style);
        Some(
            window
                .text_system()
                .shape_line(text, font_size, &runs, None),
        )
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (line_height, _): &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(line) = line {
            // a failed glyph raster leaves the path blank, like `StyledText`
            let _ = line.paint(
                bounds.origin,
                *line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    // not `super::*`: GPUI's `test` attribute would shadow the standard one
    use super::display_state;

    #[test]
    fn display_state_counts_the_ellipsis_to_the_directory() {
        let sep = std::path::MAIN_SEPARATOR;
        let directory = format!("alfa{sep}bravo{sep}charlie{sep}");
        assert_eq!(
            display_state(&directory, "delta.txt", 22),
            (
                format!("alfa{sep}bravo{sep}…{sep}"),
                "delta.txt".to_string()
            )
        );
        assert_eq!(
            display_state(&directory, "delta.txt", 100),
            (directory.clone(), "delta.txt".to_string())
        );
        // the file name no longer fits: a middle ellipsis over the path
        assert_eq!(
            display_state(&directory, "delta.txt", 6),
            ("al…".to_string(), "txt".to_string())
        );
    }
}

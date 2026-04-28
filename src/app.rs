use crate::tree::{visible_indices, NodeKind, XmlNode};

pub struct App {
    pub nodes: Vec<XmlNode>,
    pub visible: Vec<usize>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub search_query: String,
    pub search_matches: Vec<usize>, // indices into visible
    pub search_match_pos: usize,
    pub mode: Mode,
    pub viewport_height: usize,
}

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Search,
}

impl App {
    pub fn new(nodes: Vec<XmlNode>) -> Self {
        let visible = visible_indices(&nodes);
        App {
            nodes,
            visible,
            cursor: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_pos: 0,
            mode: Mode::Normal,
            viewport_height: 20,
        }
    }

    pub fn refresh_visible(&mut self) {
        self.visible = visible_indices(&self.nodes);
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.cursor + 1 - self.viewport_height;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.visible.len() {
            self.cursor += 1;
            self.clamp_scroll();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.clamp_scroll();
        }
    }

    pub fn page_down(&mut self) {
        let step = self.viewport_height.saturating_sub(1).max(1);
        self.cursor = (self.cursor + step).min(self.visible.len().saturating_sub(1));
        self.clamp_scroll();
    }

    pub fn page_up(&mut self) {
        let step = self.viewport_height.saturating_sub(1).max(1);
        self.cursor = self.cursor.saturating_sub(step);
        self.clamp_scroll();
    }

    pub fn go_top(&mut self) {
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn go_bottom(&mut self) {
        self.cursor = self.visible.len().saturating_sub(1);
        self.clamp_scroll();
    }

    pub fn toggle_collapse(&mut self) {
        if let Some(&node_idx) = self.visible.get(self.cursor) {
            let node = &mut self.nodes[node_idx];
            if node.has_children {
                node.collapsed = !node.collapsed;
                self.refresh_visible();
            }
        }
    }

    pub fn collapse_current(&mut self) {
        if let Some(&node_idx) = self.visible.get(self.cursor) {
            let node = &mut self.nodes[node_idx];
            if node.has_children && !node.collapsed {
                node.collapsed = true;
                self.refresh_visible();
            } else {
                // Move to parent
                self.move_to_parent();
            }
        }
    }

    pub fn expand_current(&mut self) {
        if let Some(&node_idx) = self.visible.get(self.cursor) {
            let node = &mut self.nodes[node_idx];
            if node.has_children && node.collapsed {
                node.collapsed = false;
                self.refresh_visible();
            }
        }
    }

    pub fn move_to_parent(&mut self) {
        if let Some(&node_idx) = self.visible.get(self.cursor) {
            let target_depth = self.nodes[node_idx].depth;
            if target_depth == 0 {
                return;
            }
            // Walk backwards in visible list to find a node with depth == target_depth - 1
            for vis_pos in (0..self.cursor).rev() {
                let idx = self.visible[vis_pos];
                if self.nodes[idx].depth == target_depth - 1 {
                    self.cursor = vis_pos;
                    self.clamp_scroll();
                    return;
                }
            }
        }
    }

    pub fn collapse_all(&mut self) {
        for node in &mut self.nodes {
            if node.has_children {
                node.collapsed = true;
            }
        }
        self.refresh_visible();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn expand_all(&mut self) {
        for node in &mut self.nodes {
            node.collapsed = false;
        }
        self.refresh_visible();
    }

    // Search helpers
    pub fn enter_search(&mut self) {
        self.mode = Mode::Search;
        self.search_query.clear();
        self.search_matches.clear();
    }

    pub fn push_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_matches();
    }

    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.update_search_matches();
    }

    pub fn commit_search(&mut self) {
        self.mode = Mode::Normal;
        if !self.search_matches.is_empty() {
            // Jump to first match at or after cursor
            let next = self
                .search_matches
                .iter()
                .position(|&m| m >= self.cursor)
                .unwrap_or(0);
            self.search_match_pos = next;
            self.cursor = self.search_matches[next];
            self.clamp_scroll();
        }
    }

    pub fn cancel_search(&mut self) {
        self.mode = Mode::Normal;
        self.search_matches.clear();
    }

    pub fn next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_pos = (self.search_match_pos + 1) % self.search_matches.len();
        self.cursor = self.search_matches[self.search_match_pos];
        self.clamp_scroll();
    }

    pub fn prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.search_match_pos == 0 {
            self.search_match_pos = self.search_matches.len() - 1;
        } else {
            self.search_match_pos -= 1;
        }
        self.cursor = self.search_matches[self.search_match_pos];
        self.clamp_scroll();
    }

    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }
        let q = self.search_query.to_lowercase();
        for (vis_pos, &node_idx) in self.visible.iter().enumerate() {
            if node_label(&self.nodes[node_idx]).to_lowercase().contains(&q) {
                self.search_matches.push(vis_pos);
            }
        }
    }
}

pub fn node_label(node: &XmlNode) -> String {
    match &node.kind {
        NodeKind::Element { name, attrs } => {
            if attrs.is_empty() {
                format!("<{name}>")
            } else {
                let attr_str: String = attrs
                    .iter()
                    .map(|(k, v)| format!(" {k}=\"{v}\""))
                    .collect();
                format!("<{name}{attr_str}>")
            }
        }
        NodeKind::CloseElement { name } => format!("</{name}>"),
        NodeKind::Text(t) => t.clone(),
        NodeKind::Comment(t) => format!("<!-- {t} -->"),
        NodeKind::CData(t) => format!("<![CDATA[{t}]]>"),
    }
}

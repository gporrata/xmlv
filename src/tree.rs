use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct XmlNode {
    pub kind: NodeKind,
    pub depth: usize,
    pub collapsed: bool,
    pub has_children: bool,
    pub child_count: usize, // populated after build
    pub line: usize,        // 1-indexed line in original source
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Element {
        name: String,
        attrs: Vec<(String, String)>,
    },
    CloseElement {
        name: String,
    },
    Text(String),
    Comment(String),
    CData(String),
}

pub fn parse(content: &[u8], include_close_tags: bool) -> Result<Vec<XmlNode>, String> {
    // Build a lookup from byte offset → 1-indexed line number.
    let mut line_starts: Vec<usize> = vec![0];
    for (i, &b) in content.iter().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let byte_to_line = |pos: u64| -> usize {
        let pos = pos as usize;
        match line_starts.binary_search(&pos) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        }
    };

    let mut xml = Reader::from_reader(content);
    xml.config_mut().trim_text(true);

    let mut nodes: Vec<XmlNode> = Vec::new();
    let mut buf = Vec::new();
    let mut depth: usize = 0;
    // stack holds (node_index, element_name) for open tags
    let mut stack: Vec<(usize, String)> = Vec::new();

    loop {
        let pos = xml.buffer_position();
        match xml.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let line = byte_to_line(pos);
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let k = String::from_utf8_lossy(a.key.local_name().as_ref()).to_string();
                        let v = a.unescape_value().map(|v| v.to_string()).unwrap_or_default();
                        (k, v)
                    })
                    .collect();
                let idx = nodes.len();
                nodes.push(XmlNode {
                    kind: NodeKind::Element { name: name.clone(), attrs },
                    depth,
                    collapsed: false,
                    has_children: true,
                    child_count: 0,
                    line,
                });
                stack.push((idx, name));
                depth += 1;
            }
            Ok(Event::End(_)) => {
                let line = byte_to_line(pos);
                depth = depth.saturating_sub(1);
                if let Some((open_idx, name)) = stack.pop() {
                    let had_children = nodes.len() > open_idx + 1;
                    if had_children && include_close_tags {
                        nodes.push(XmlNode {
                            kind: NodeKind::CloseElement { name },
                            depth,
                            collapsed: false,
                            has_children: false,
                            child_count: 0,
                            line,
                        });
                    } else if !had_children {
                        nodes[open_idx].has_children = false;
                    }
                    let count = nodes.len() - open_idx - 1;
                    nodes[open_idx].child_count = count;
                }
            }
            Ok(Event::Empty(e)) => {
                let line = byte_to_line(pos);
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let k = String::from_utf8_lossy(a.key.local_name().as_ref()).to_string();
                        let v = a.unescape_value().map(|v| v.to_string()).unwrap_or_default();
                        (k, v)
                    })
                    .collect();
                nodes.push(XmlNode {
                    kind: NodeKind::Element { name, attrs },
                    depth,
                    collapsed: false,
                    has_children: false,
                    child_count: 0,
                    line,
                });
            }
            Ok(Event::Text(e)) => {
                let line = byte_to_line(pos);
                let text = e.unescape().map(|t| t.to_string()).unwrap_or_default();
                if !text.trim().is_empty() {
                    nodes.push(XmlNode {
                        kind: NodeKind::Text(text),
                        depth,
                        collapsed: false,
                        has_children: false,
                        child_count: 0,
                        line,
                    });
                }
            }
            Ok(Event::Comment(e)) => {
                let line = byte_to_line(pos);
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                nodes.push(XmlNode {
                    kind: NodeKind::Comment(text),
                    depth,
                    collapsed: false,
                    has_children: false,
                    child_count: 0,
                    line,
                });
            }
            Ok(Event::CData(e)) => {
                let line = byte_to_line(pos);
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                nodes.push(XmlNode {
                    kind: NodeKind::CData(text),
                    depth,
                    collapsed: false,
                    has_children: false,
                    child_count: 0,
                    line,
                });
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(nodes)
}

/// Returns the flat list of visible node indices given collapsed state.
pub fn visible_indices(nodes: &[XmlNode]) -> Vec<usize> {
    let mut result = Vec::new();
    let mut skip_until_depth: Option<usize> = None;

    for (i, node) in nodes.iter().enumerate() {
        if let Some(skip_depth) = skip_until_depth {
            if node.depth > skip_depth {
                continue;
            }
            // We've exited the collapsed section; check if this is the closing tag
            if node.depth == skip_depth {
                // This is either a sibling or the closing tag — show it but don't skip anymore
                // Actually for the close tag at same depth we show it
                skip_until_depth = None;
            }
        }
        result.push(i);
        if node.collapsed && node.has_children {
            skip_until_depth = Some(node.depth);
        }
    }
    result
}

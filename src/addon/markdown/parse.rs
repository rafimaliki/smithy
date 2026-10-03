//! Markdown text into a small block tree. No gpui and no theme here, so it is
//! unit-tested directly; `render.rs` turns the tree into elements.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Inline emphasis, as a set of flags: nested emphasis is merged into one span, so
/// the spans of a paragraph never overlap and can become one styled text run each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// An inline image; `text` is the alt text and `url` the source.
    pub image: bool,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub style: Style,
    /// Link target, or image source.
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub spans: Vec<Span>,
    /// Task list marker, when the item has one.
    pub checked: Option<bool>,
    /// Blocks that follow the item's own text: a nested list, a code block.
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    Code(String),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Item>,
    },
    Table {
        head: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Quote(Vec<Block>),
    Rule,
}

/// Render a whole document. Unknown constructs (raw HTML, footnotes) are skipped.
pub fn parse(text: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    Doc::default().run(Parser::new_ext(text, options))
}

enum Frame {
    Doc {
        blocks: Vec<Block>,
    },
    Quote {
        blocks: Vec<Block>,
    },
    Para {
        spans: Vec<Span>,
    },
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Code {
        text: String,
    },
    Item {
        spans: Vec<Span>,
        checked: Option<bool>,
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Item>,
    },
    Table {
        head: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
        row: Vec<Vec<Span>>,
        cell: Vec<Span>,
        in_head: bool,
    },
}

#[derive(Default)]
struct Doc {
    stack: Vec<Frame>,
    /// Inline state, pushed and popped by the span-level tags.
    style: Style,
    url: Option<String>,
}

impl Doc {
    fn run(mut self, events: Parser) -> Vec<Block> {
        self.stack.push(Frame::Doc { blocks: Vec::new() });
        for event in events {
            self.event(event);
        }
        match self.stack.pop() {
            Some(Frame::Doc { blocks }) => blocks,
            _ => Vec::new(),
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                let style = Style {
                    code: true,
                    ..self.style
                };
                let url = self.url.clone();
                if let Some(spans) = self.sink() {
                    push(spans, &code, style, url);
                }
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.text("\n"),
            Event::Rule => self.push_block(Block::Rule),
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item { checked: slot, .. }) = self.stack.last_mut() {
                    *slot = Some(checked);
                }
            }
            // Raw HTML is never rendered, and footnotes are out of scope.
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.stack.push(Frame::Para { spans: Vec::new() }),
            Tag::Heading { level, .. } => self.stack.push(Frame::Heading {
                level: level as u8,
                spans: Vec::new(),
            }),
            Tag::CodeBlock(_) => self.stack.push(Frame::Code {
                text: String::new(),
            }),
            Tag::BlockQuote(_) => self.stack.push(Frame::Quote { blocks: Vec::new() }),
            Tag::Item => self.stack.push(Frame::Item {
                spans: Vec::new(),
                checked: None,
                blocks: Vec::new(),
            }),
            Tag::List(start) => self.stack.push(Frame::List {
                ordered: start.is_some(),
                start: start.unwrap_or(1),
                items: Vec::new(),
            }),
            Tag::Table(_) => self.stack.push(Frame::Table {
                head: Vec::new(),
                rows: Vec::new(),
                row: Vec::new(),
                cell: Vec::new(),
                in_head: false,
            }),
            Tag::TableHead => {
                if let Some(Frame::Table { in_head, row, .. }) = self.stack.last_mut() {
                    *in_head = true;
                    row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(Frame::Table { row, .. }) = self.stack.last_mut() {
                    row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(Frame::Table { cell, .. }) = self.stack.last_mut() {
                    cell.clear();
                }
            }
            Tag::Emphasis => self.style.italic = true,
            Tag::Strong => self.style.bold = true,
            Tag::Strikethrough => self.style.strike = true,
            Tag::Link { dest_url, .. } => self.url = Some(dest_url.into_string()),
            Tag::Image { dest_url, .. } => {
                self.style.image = true;
                self.url = Some(dest_url.into_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if let Some(Frame::Para { spans }) = self.stack.pop() {
                    self.push_block(Block::Paragraph(spans));
                }
            }
            TagEnd::Heading(_) => {
                if let Some(Frame::Heading { level, spans }) = self.stack.pop() {
                    self.push_block(Block::Heading { level, spans });
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Frame::Code { text }) = self.stack.pop() {
                    let text = text.trim_end_matches('\n').to_string();
                    self.push_block(Block::Code(text));
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Frame::Quote { blocks }) = self.stack.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::Item => {
                if let Some(Frame::Item {
                    spans,
                    checked,
                    blocks,
                }) = self.stack.pop()
                {
                    let item = Item {
                        spans,
                        checked,
                        blocks,
                    };
                    if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                        items.push(item);
                    }
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List {
                    ordered,
                    start,
                    items,
                }) = self.stack.pop()
                {
                    self.push_block(Block::List {
                        ordered,
                        start,
                        items,
                    });
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table { head, rows, .. }) = self.stack.pop() {
                    self.push_block(Block::Table { head, rows });
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::Table {
                    head, row, in_head, ..
                }) = self.stack.last_mut()
                {
                    *head = std::mem::take(row);
                    *in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table {
                    head,
                    rows,
                    row,
                    in_head,
                    ..
                }) = self.stack.last_mut()
                {
                    let done = std::mem::take(row);
                    // A header row can also arrive as a `TableRow`.
                    if *in_head {
                        *head = done;
                    } else {
                        rows.push(done);
                    }
                }
            }
            TagEnd::TableCell => {
                if let Some(Frame::Table { row, cell, .. }) = self.stack.last_mut() {
                    row.push(std::mem::take(cell));
                }
            }
            TagEnd::Emphasis => self.style.italic = false,
            TagEnd::Strong => self.style.bold = false,
            TagEnd::Strikethrough => self.style.strike = false,
            TagEnd::Link => self.url = None,
            TagEnd::Image => {
                self.style.image = false;
                self.url = None;
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        let style = self.style;
        let url = self.url.clone();
        if let Some(Frame::Code { text: code }) = self.stack.last_mut() {
            code.push_str(text);
        } else if let Some(spans) = self.sink() {
            push(spans, text, style, url);
        }
    }

    /// Where inline text goes: the innermost text container, if there is one.
    fn sink(&mut self) -> Option<&mut Vec<Span>> {
        self.stack.iter_mut().rev().find_map(|frame| match frame {
            Frame::Para { spans } | Frame::Heading { spans, .. } | Frame::Item { spans, .. } => {
                Some(spans)
            }
            Frame::Table { cell, .. } => Some(cell),
            _ => None,
        })
    }

    /// Attach a finished block to the container that is open around it.
    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::Doc { blocks }) | Some(Frame::Quote { blocks }) => blocks.push(block),
            Some(Frame::Item { blocks, .. }) => blocks.push(block),
            _ => {}
        }
    }
}

fn push(spans: &mut Vec<Span>, text: &str, style: Style, url: Option<String>) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == style && last.url == url {
            last.text.push_str(text);
            return;
        }
    }
    spans.push(Span {
        text: text.to_string(),
        style,
        url,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(blocks: &[Block]) -> &[Span] {
        match &blocks[0] {
            Block::Paragraph(spans) => spans,
            _ => panic!("not a paragraph: {blocks:?}"),
        }
    }

    #[test]
    fn headings_paragraphs_and_emphasis() {
        let blocks = parse("# Title\n\nBody **bold** and *it* and `c`.");
        match &blocks[0] {
            Block::Heading { level, spans } => {
                assert_eq!(*level, 1);
                assert_eq!(spans[0].text, "Title");
            }
            other => panic!("{other:?}"),
        }
        let spans = spans(&blocks[1..]);
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "Body bold and it and c.");
        assert!(spans.iter().any(|s| s.style.bold && s.text == "bold"));
        assert!(spans.iter().any(|s| s.style.italic && s.text == "it"));
        assert!(spans.iter().any(|s| s.style.code && s.text == "c"));
    }

    #[test]
    fn fenced_code_keeps_its_text_and_drops_the_trailing_newline() {
        let blocks = parse("```rust\nfn main() {}\n```\n");
        match &blocks[0] {
            Block::Code(text) => assert_eq!(text, "fn main() {}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn gfm_table_has_a_head_and_rows() {
        let blocks = parse("| Target | Value |\n| --- | --- |\n| Idle RAM | 150 MB |\n");
        match &blocks[0] {
            Block::Table { head, rows } => {
                assert_eq!(head.len(), 2);
                assert_eq!(head[0][0].text, "Target");
                assert_eq!(head[1][0].text, "Value");
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0][0][0].text, "Idle RAM");
                assert_eq!(rows[0][1][0].text, "150 MB");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nested_list_stays_inside_its_item() {
        let blocks = parse("- one\n  - sub\n- two\n");
        match &blocks[0] {
            Block::List { ordered, items, .. } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].spans[0].text, "one");
                match &items[0].blocks[0] {
                    Block::List { items, .. } => assert_eq!(items[0].spans[0].text, "sub"),
                    other => panic!("{other:?}"),
                }
                assert_eq!(items[1].spans[0].text, "two");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ordered_list_keeps_its_start_number() {
        let blocks = parse("3. three\n4. four\n");
        match &blocks[0] {
            Block::List {
                ordered,
                start,
                items,
            } => {
                assert!(ordered);
                assert_eq!(*start, 3);
                assert_eq!(items.len(), 2);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn task_list_marker_is_kept() {
        let blocks = parse("- [x] done\n- [ ] todo\n");
        match &blocks[0] {
            Block::List { items, .. } => {
                assert_eq!(items[0].checked, Some(true));
                assert_eq!(items[1].checked, Some(false));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn links_carry_their_target_and_images_their_source() {
        let blocks = parse("See [the docs](https://example.com/a) and ![logo](img/logo.png).");
        let spans = spans(&blocks);
        let link = spans.iter().find(|s| s.text == "the docs").unwrap();
        assert_eq!(link.url.as_deref(), Some("https://example.com/a"));
        let image = spans.iter().find(|s| s.style.image).unwrap();
        assert_eq!(image.text, "logo");
        assert_eq!(image.url.as_deref(), Some("img/logo.png"));
    }

    #[test]
    fn a_quote_holds_its_paragraphs() {
        let blocks = parse("> quoted text\n");
        match &blocks[0] {
            Block::Quote(inner) => match &inner[0] {
                Block::Paragraph(spans) => assert_eq!(spans[0].text, "quoted text"),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn soft_breaks_become_spaces() {
        let blocks = parse("one\ntwo\n");
        let text: String = spans(&blocks).iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "one two");
    }
}

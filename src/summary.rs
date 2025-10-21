use std::borrow::Cow;
use std::fmt;

#[derive(Debug)]
pub(crate) enum Node {
    Headings(Box<[Heading]>),
    List(List),
}

impl Node {
    pub(crate) fn display(&self) -> impl fmt::Display {
        NodeDisplay {
            node: self,
            depth: 1,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Heading {
    pub(crate) name: Cow<'static, str>,
    pub(crate) contents: Box<Node>,
}

pub(crate) type List = Box<[ListItem]>;

#[derive(Debug)]
pub(crate) struct ListItem {
    pub(crate) contents: Paragraph,
    pub(crate) sublist: List,
}

pub(crate) type Paragraph = Box<[ParagraphFragment]>;
pub(crate) type ParagraphFragment = Cow<'static, str>;

macro_rules! paragraph {
    (_impl [$($t:tt)*]) => {
        std::vec::Vec::<$crate::summary::ParagraphFragment>::into_boxed_slice(std::vec![$($t)*])
    };
    (_impl [$($head:tt)*] link($text:expr, $href:expr) $($tail:tt)*) => {
        $crate::summary::paragraph!(_impl [
            $($head)*
            std::borrow::Cow::Borrowed("["),
            std::borrow::Cow::from($text),
            std::borrow::Cow::Borrowed("]"),
            std::borrow::Cow::Borrowed("("),
            std::borrow::Cow::from($href),
            std::borrow::Cow::Borrowed(")"),
        ] $($tail)*)
    };
    (_impl [$($head:tt)*] $s:literal $($tail:tt)*) => {
        $crate::summary::paragraph!(_impl [$($head)* std::borrow::Cow::Borrowed($s),] $($tail)*)
    };
    (_impl [$($head:tt)*] {$e:expr} $($tail:tt)*) => {
        $crate::summary::paragraph!(_impl [$($head)* std::borrow::Cow::from($e),] $($tail)*)
    };
    ($($t:tt)*) => {
        $crate::summary::paragraph!(_impl [] $($t)*)
    };
}
pub(crate) use paragraph;

struct NodeDisplay<'n> {
    node: &'n Node,
    depth: u8,
}

impl fmt::Display for NodeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match &self.node {
            Node::Headings(headings) => {
                for (i, heading) in headings.iter().enumerate() {
                    if i != 0 {
                        writeln!(f)?;
                    }

                    let prefix = "#".repeat(self.depth.into());
                    writeln!(f, "{prefix} {}\n", heading.name)?;
                    let contents = NodeDisplay {
                        node: &heading.contents,
                        depth: self.depth + 1,
                    };
                    contents.fmt(f)?;
                }
                Ok(())
            }
            Node::List(list) => display_list(f, list, 0),
        }
    }
}

fn display_list(f: &mut fmt::Formatter, list: &[ListItem], depth: usize) -> Result<(), fmt::Error> {
    let indent = "  ".repeat(depth);
    for item in list {
        write!(f, "{indent}- ")?;
        for fragment in &item.contents {
            write!(f, "{fragment}")?;
        }
        writeln!(f)?;
        display_list(f, &item.sublist, depth + 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_empty() {
        let p = paragraph!();
        assert!(p.is_empty());
    }

    #[test]
    fn paragraph_simple() {
        let p = paragraph!("simple");
        assert_eq!(&*p, &[Cow::Borrowed("simple")])
    }

    #[test]
    fn paragraph_expression() {
        let f = || "from fn";
        let p = paragraph!({ f() });
        assert_eq!(&*p, &[Cow::Borrowed("from fn")])
    }

    #[test]
    fn paragraph_link() {
        let p = paragraph!(link("text", "url"));
        let p = p.into_iter().collect::<String>();
        assert_eq!(p, "[text](url)");
    }

    #[test]
    fn paragraph_mixed() {
        let p = paragraph!("some text " link("label", "https://example.com") " more text");
        let p = p.into_iter().collect::<String>();
        assert_eq!(p, "some text [label](https://example.com) more text");
    }
}

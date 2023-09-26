use mdbook::book::{Book, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::utils::new_cmark_parser;
use pulldown_cmark::{CowStr, Event, Tag};

#[derive(Default)]
pub struct Classy;

impl Classy {
    pub fn new() -> Classy {
        Classy
    }
}

impl Preprocessor for Classy {
    fn name(&self) -> &str {
        "classy"
    }
    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        book.for_each_mut(|book| {
            if let mdbook::BookItem::Chapter(chapter) = book {
                if let Err(e) = classy(chapter) {
                    eprintln!("classy error: {:?}", e);
                }
            }
        });
        Ok(book)
    }
    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

#[derive(Debug)]
struct ClassAnnotation {
    pub class: String,
    pub _index: usize,
    pub paragraph_start: usize,
    pub paragraph_end: Option<usize>,
}

/// This is where the markdown transformation actually happens.
/// Take paragraphs beginning with `{:.class-name}` and give them special rendering.
/// Mutation: the payload here is that it edits chapter.content.
fn classy(chapter: &mut Chapter) -> Result<(), Error> {
    // 1. Parse the inbound markdown into an Event vector.
    let incoming_events: Vec<Event> = new_cmark_parser(&chapter.content, false).collect();

    // 2. Find paragraphs beginning with the class annotator `{:.class-name}` and record their information in
    // a vector of ClassAnnotation structs.
    let mut class_annotations: Vec<ClassAnnotation> = vec![];
    for i in 0..incoming_events.len() {
        let event = &incoming_events[i];
        match *event {
            Event::Text(CowStr::Borrowed(text)) => {
                if i > 0 {
                    if let Event::Start(Tag::Paragraph) = incoming_events[i - 1] {
                        let v: Vec<_> = text.split("").collect();
                        let len_v = v.len();

                        if len_v > 4 && v[..4].join("") == "{:." && v[(len_v - 2)..].join("") == "}"
                        {
                            // TODO: Look into why cmark splits paragraphs that start with '['

                            let class = v[4..(len_v - 2)].join("");
                            class_annotations.push(ClassAnnotation {
                                class,
                                _index: i,
                                paragraph_start: i - 1,
                                paragraph_end: None,
                            })
                        }
                    }
                }
            }
            Event::End(Tag::Paragraph) => {
                let last = class_annotations.last_mut();
                if let Some(class_command) = last {
                    if class_command.paragraph_end.is_none() {
                        class_command.paragraph_end = Some(i);
                    }
                }
            }
            _ => {}
        }
    }

    // 3. Construct a new_events vector with <div class="class-name">\n \n</div> around the annotated paragraphs
    // (and with the class annotation removed).
    let mut slices = vec![];
    let mut last_end = 0;
    let div_starts: Vec<Event> = class_annotations
        .iter()
        .map(|ca| Event::Html(CowStr::from(format!("<div class=\"{}\">", ca.class))))
        .collect();
    let div_end: Vec<Event> = vec![Event::Html(CowStr::from("</div>"))];
    for (i, ca) in class_annotations.iter().enumerate() {
        // Add unclassed events.
        slices.push(&incoming_events[last_end..ca.paragraph_start]);

        last_end = ca.paragraph_end.unwrap_or(incoming_events.len() - 1);

        let paragraph = &incoming_events[ca.paragraph_start..(last_end + 1)];

        // Add <div class="class-name">
        slices.push(&div_starts[i..i + 1]);

        // Add paragraph opener.
        slices.push(&paragraph[0..1]);

        // Add the rest of the paragraph, skipping the class annotation.
        slices.push(&paragraph[2..]);

        // Add </div>.
        slices.push(&div_end[..]);
    }
    slices.push(&incoming_events[last_end..]);
    let new_events = slices.concat();

    // 4. Update chapter.content using markdown generated from the new event vector.
    let mut buf = String::with_capacity(chapter.content.len() + 128);
    pulldown_cmark_to_cmark::cmark(new_events.into_iter(), &mut buf)
        .expect("unable to re-render cmark");
    chapter.content = buf;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_book(content: &str) -> Book {
        serde_json::from_value(json!({
            "sections": [
                {
                    "Chapter": {
                        "name": "Chapter 1",
                        "content": content,
                        "number": [1],
                        "sub_items": [],
                        "path": "chapter_1.md",
                        "source_path": "chapter_1.md",
                        "parent_names": []
                    }
                }
            ],
            "__non_exhaustive": null
        }))
        .unwrap()
    }

    fn mock_context() -> PreprocessorContext {
        let value = json!({
            "root": "/path/to/book",
            "config": {
                "book": {
                    "authors": ["AUTHOR"],
                    "language": "en",
                    "multilingual": false,
                    "src": "src",
                    "title": "TITLE"
                },
                "preprocessor": {
                    "classy": "classy",
                }
            },
            "renderer": "html",
            "mdbook_version": "0.4.34"
        });

        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn basic_usage() {
        let content = r#"<style>
    .red{color: red;}
</style>
{:.red}
red text"#;
        let expected_content = r#"<style>
    .red{color: red;}
</style>
<div class="red">

red text

</div>"#;
        let ctx = mock_context();
        let book = mock_book(content);
        let expected_book = mock_book(expected_content);

        assert_eq!(Classy::new().run(&ctx, book).unwrap(), expected_book)
    }

    #[test]
    fn no_change_preprocessor_run() {
        let content = r#####"# Chapter 1\n"#####;

        let ctx: PreprocessorContext = mock_context();
        let book: Book = mock_book(content);

        let expected_book = book.clone();
        let result = Classy::new().run(&ctx, book);
        assert!(result.is_ok());

        // The classy preprocessor should not have made any changes to the book content.
        let actual_book = result.unwrap();
        assert_eq!(actual_book, expected_book);
    }

    /// FIXME: due to pulldown_cmark losing information, lossless round trip markdown -> html -> markdown is currently not possible.
    /// See https://github.com/Byron/pulldown-cmark-to-cmark/issues/20
    /// This test is designed to detect if the upstream crates fix the issue
    #[test]
    fn no_change_round_trip_fail_preprocessor_run() {
        let content = r#####"# Markdown

mdBook's [parser](https://github.com/raphlinus/pulldown-cmark) adheres to the [CommonMark](https://commonmark.org/) specification with some extensions described below.
You can take a quick [tutorial](https://commonmark.org/help/tutorial/),
or [try out](https://spec.commonmark.org/dingus/) CommonMark in real time. A complete Markdown overview is out of scope for 
this documentation, but below is a high level overview of some of the basics. For a more in-depth experience, check out the
[Markdown Guide](https://www.markdownguide.org).

## Text and Paragraphs

Text is rendered relatively predictably: 

```markdown
Here is a line of text.

This is a new line.
```

Will look like you might expect:

Here is a line of text.

This is a new line.

## Headings

Headings use the `#` marker and should be on a line by themselves. More `#` mean smaller headings:

```markdown
### A heading 

Some text.

#### A smaller heading 

More text.
```

### A heading 

Some text.

#### A smaller heading 

More text.

## Lists

Lists can be unordered or ordered. Ordered lists will order automatically:

```markdown
* milk
* eggs
* butter

1. carrots
1. celery
1. radishes
```

* milk
* eggs
* butter

1. carrots
1. celery
1. radishes

## Links

Linking to a URL or local file is easy:

```markdown
Use [mdBook](https://github.com/rust-lang/mdBook). 

Read about [mdBook](mdbook.md).

A bare url: <https://www.rust-lang.org>.
```

Use [mdBook](https://github.com/rust-lang/mdBook). 

Read about [mdBook](mdbook.md).

A bare url: <https://www.rust-lang.org>.

----

Relative links that end with `.md` will be converted to the `.html` extension.
It is recommended to use `.md` links when possible.
This is useful when viewing the Markdown file outside of mdBook, for example on GitHub or GitLab which render Markdown automatically.

Links to `README.md` will be converted to `index.html`.
This is done since some services like GitHub render README files automatically, but web servers typically expect the root file to be called `index.html`.

You can link to individual headings with `#` fragments.
For example, `mdbook.md#text-and-paragraphs` would link to the [Text and Paragraphs](#text-and-paragraphs) section above.
The ID is created by transforming the heading such as converting to lowercase and replacing spaces with dashes.
You can click on any heading and look at the URL in your browser to see what the fragment looks like.

## Images

Including images is simply a matter of including a link to them, much like in the _Links_ section above. The following markdown
includes the Rust logo SVG image found in the `images` directory at the same level as this file:

```markdown
![The Rust Logo](images/rust-logo-blk.svg)
```

Produces the following HTML when built with mdBook:

```html
<p><img src="images/rust-logo-blk.svg" alt="The Rust Logo" /></p>
```

Which, of course displays the image like so:

![The Rust Logo](images/rust-logo-blk.svg)

## Extensions

mdBook has several extensions beyond the standard CommonMark specification.

### Strikethrough

Text may be rendered with a horizontal line through the center by wrapping the
text with one or two tilde characters on each side:

```text
An example of ~~strikethrough text~~.
```

This example will render as:

> An example of ~~strikethrough text~~.

This follows the [GitHub Strikethrough extension][strikethrough].

### Footnotes

A footnote generates a small numbered link in the text which when clicked
takes the reader to the footnote text at the bottom of the item. The footnote
label is written similarly to a link reference with a caret at the front. The
footnote text is written like a link reference definition, with the text
following the label. Example:

```text
This is an example of a footnote[^note].

[^note]: This text is the contents of the footnote, which will be rendered
    towards the bottom.
```

This example will render as:

> This is an example of a footnote[^note].
>
> [^note]: This text is the contents of the footnote, which will be rendered
>     towards the bottom.

The footnotes are automatically numbered based on the order the footnotes are
written.

### Tables

Tables can be written using pipes and dashes to draw the rows and columns of
the table. These will be translated to HTML table matching the shape. Example:

```text
| Header1 | Header2 |
|---------|---------|
| abc     | def     |
```

This example will render similarly to this:

| Header1 | Header2 |
|---------|---------|
| abc     | def     |

See the specification for the [GitHub Tables extension][tables] for more
details on the exact syntax supported.

### Task lists

Task lists can be used as a checklist of items that have been completed.
Example:

```md
- [x] Complete task
- [ ] Incomplete task
```

This will render as:

> - [x] Complete task
> - [ ] Incomplete task

See the specification for the [task list extension] for more details.

### Smart punctuation

Some ASCII punctuation sequences will be automatically turned into fancy Unicode
characters:

| ASCII sequence | Unicode |
|----------------|---------|
| `--`           | –       |
| `---`          | —       |
| `...`          | …       |
| `"`            | “ or ”, depending on context |
| `'`            | ‘ or ’, depending on context |

So, no need to manually enter those Unicode characters!

This feature is disabled by default.
To enable it, see the [`output.html.curly-quotes`] config option.

[strikethrough]: https://github.github.com/gfm/#strikethrough-extension-
[tables]: https://github.github.com/gfm/#tables-extension-
[task list extension]: https://github.github.com/gfm/#task-list-items-extension-
[`output.html.curly-quotes`]: configuration/renderers.md#html-renderer-options

### Heading attributes

Headings can have a custom HTML ID and classes. This lets you maintain the same ID even if you change the heading's text, it also lets you add multiple classes in the heading.

Example:
```md
# Example heading { #first .class1 .class2 }
```

This makes the level 1 heading with the content `Example heading`, ID `first`, and classes `class1` and `class2`. Note that the attributes should be space-separated.

More information can be found in the [heading attrs spec page](https://github.com/raphlinus/pulldown-cmark/blob/master/specs/heading_attrs.txt)."#####;

        let ctx: PreprocessorContext = mock_context();
        let book: Book = mock_book(content);

        let expected_book = book.clone();
        let result = Classy::new().run(&ctx, book);
        assert!(result.is_ok());

        // The nop-preprocessor should not have made any changes to the book content.
        let actual_book = result.unwrap();
        assert_ne!(actual_book, expected_book);
    }

    /// FIXME: this test uses the raw round trip output from pulldown_cmark::cmark -> pulldown_cmark_to_cmark::cmark
    /// due to pulldown_cmark losing information, lossless round trip markdown -> html -> markdown is currently not possible.
    /// See https://github.com/Byron/pulldown-cmark-to-cmark/issues/20
    /// If the upstream crates fix the issue, this test can be updated
    #[test]
    fn no_change_round_trip_preprocessor_run() {
        let content = "# Markdown\n\nmdBook's [parser](https://github.com/raphlinus/pulldown-cmark) adheres to the [CommonMark](https://commonmark.org/) specification with some extensions described below.\nYou can take a quick [tutorial](https://commonmark.org/help/tutorial/),\nor [try out](https://spec.commonmark.org/dingus/) CommonMark in real time. A complete Markdown overview is out of scope for \nthis documentation, but below is a high level overview of some of the basics. For a more in-depth experience, check out the\n[Markdown Guide](https://www.markdownguide.org).\n\n## Text and Paragraphs\n\nText is rendered relatively predictably: \n\n````markdown\nHere is a line of text.\n\nThis is a new line.\n````\n\nWill look like you might expect:\n\nHere is a line of text.\n\nThis is a new line.\n\n## Headings\n\nHeadings use the `#` marker and should be on a line by themselves. More `#` mean smaller headings:\n\n````markdown\n### A heading \n\nSome text.\n\n#### A smaller heading \n\nMore text.\n````\n\n### A heading\n\nSome text.\n\n#### A smaller heading\n\nMore text.\n\n## Lists\n\nLists can be unordered or ordered. Ordered lists will order automatically:\n\n````markdown\n* milk\n* eggs\n* butter\n\n1. carrots\n1. celery\n1. radishes\n````\n\n* milk\n* eggs\n* butter\n\n1. carrots\n1. celery\n1. radishes\n\n## Links\n\nLinking to a URL or local file is easy:\n\n````markdown\nUse [mdBook](https://github.com/rust-lang/mdBook). \n\nRead about [mdBook](mdbook.md).\n\nA bare url: <https://www.rust-lang.org>.\n````\n\nUse [mdBook](https://github.com/rust-lang/mdBook). \n\nRead about [mdBook](mdbook.md).\n\nA bare url: <https://www.rust-lang.org>.\n\n---\n\nRelative links that end with `.md` will be converted to the `.html` extension.\nIt is recommended to use `.md` links when possible.\nThis is useful when viewing the Markdown file outside of mdBook, for example on GitHub or GitLab which render Markdown automatically.\n\nLinks to `README.md` will be converted to `index.html`.\nThis is done since some services like GitHub render README files automatically, but web servers typically expect the root file to be called `index.html`.\n\nYou can link to individual headings with `#` fragments.\nFor example, `mdbook.md#text-and-paragraphs` would link to the [Text and Paragraphs](#text-and-paragraphs) section above.\nThe ID is created by transforming the heading such as converting to lowercase and replacing spaces with dashes.\nYou can click on any heading and look at the URL in your browser to see what the fragment looks like.\n\n## Images\n\nIncluding images is simply a matter of including a link to them, much like in the *Links* section above. The following markdown\nincludes the Rust logo SVG image found in the `images` directory at the same level as this file:\n\n````markdown\n![The Rust Logo](images/rust-logo-blk.svg)\n````\n\nProduces the following HTML when built with mdBook:\n\n````html\n<p><img src=\"images/rust-logo-blk.svg\" alt=\"The Rust Logo\" /></p>\n````\n\nWhich, of course displays the image like so:\n\n![The Rust Logo](images/rust-logo-blk.svg)\n\n## Extensions\n\nmdBook has several extensions beyond the standard CommonMark specification.\n\n### Strikethrough\n\nText may be rendered with a horizontal line through the center by wrapping the\ntext with one or two tilde characters on each side:\n\n````text\nAn example of ~~strikethrough text~~.\n````\n\nThis example will render as:\n\n > \n > An example of ~~strikethrough text~~.\n\nThis follows the [GitHub Strikethrough extension](https://github.github.com/gfm/#strikethrough-extension-).\n\n### Footnotes\n\nA footnote generates a small numbered link in the text which when clicked\ntakes the reader to the footnote text at the bottom of the item. The footnote\nlabel is written similarly to a link reference with a caret at the front. The\nfootnote text is written like a link reference definition, with the text\nfollowing the label. Example:\n\n````text\nThis is an example of a footnote[^note].\n\n[^note]: This text is the contents of the footnote, which will be rendered\n    towards the bottom.\n````\n\nThis example will render as:\n\n > \n > This is an example of a footnote[^note].\n > \n > [^note]: This text is the contents of the footnote, which will be rendered\n > towards the bottom.\n\nThe footnotes are automatically numbered based on the order the footnotes are\nwritten.\n\n### Tables\n\nTables can be written using pipes and dashes to draw the rows and columns of\nthe table. These will be translated to HTML table matching the shape. Example:\n\n````text\n| Header1 | Header2 |\n|---------|---------|\n| abc     | def     |\n````\n\nThis example will render similarly to this:\n\n|Header1|Header2|\n|-------|-------|\n|abc|def|\n\nSee the specification for the [GitHub Tables extension](https://github.github.com/gfm/#tables-extension-) for more\ndetails on the exact syntax supported.\n\n### Task lists\n\nTask lists can be used as a checklist of items that have been completed.\nExample:\n\n````md\n- [x] Complete task\n- [ ] Incomplete task\n````\n\nThis will render as:\n\n > \n > * [x] Complete task\n > * [ ] Incomplete task\n\nSee the specification for the [task list extension] for more details.\n\n### Smart punctuation\n\nSome ASCII punctuation sequences will be automatically turned into fancy Unicode\ncharacters:\n\n|ASCII sequence|Unicode|\n|--------------|-------|\n|`--`|–|\n|`---`|—|\n|`...`|…|\n|`\"`|“ or ”, depending on context|\n|`'`|‘ or ’, depending on context|\n\nSo, no need to manually enter those Unicode characters!\n\nThis feature is disabled by default.\nTo enable it, see the [`output.html.curly-quotes`] config option.\n\n### Heading attributes\n\nHeadings can have a custom HTML ID and classes. This lets you maintain the same ID even if you change the heading's text, it also lets you add multiple classes in the heading.\n\nExample:\n\n````md\n# Example heading { #first .class1 .class2 }\n````\n\nThis makes the level 1 heading with the content `Example heading`, ID `first`, and classes `class1` and `class2`. Note that the attributes should be space-separated.\n\nMore information can be found in the [heading attrs spec page](https://github.com/raphlinus/pulldown-cmark/blob/master/specs/heading_attrs.txt).\n\n[task list extension]: https://github.github.com/gfm/#task-list-items-extension-\n[`output.html.curly-quotes`]: configuration/renderers.md#html-renderer-options";

        let ctx: PreprocessorContext = mock_context();
        let book: Book = mock_book(content);

        let expected_book = book.clone();
        let result = Classy::new().run(&ctx, book);
        assert!(result.is_ok());

        // The nop-preprocessor should not have made any changes to the book content.
        let actual_book = result.unwrap();
        assert_eq!(actual_book, expected_book);
    }
}

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use tera::Tera;
use tera::Context;
use markdown::{
    mdast::{ Node, Root, Yaml },
    Constructs, Options, ParseOptions, CompileOptions
};
use serde::Deserialize;

/// A struct that holds and syncs together the different names that refer to a page 
/// `src` is the path to a page's source .md file
/// `out` is the path to the page's compiled .html file
/// `uri` is the path to access it from a server
#[derive(Debug)]
struct PageTriple {
    src: PathBuf,
    out: PathBuf,
    uri: PathBuf,
}
impl PageTriple {
    fn new(src: PathBuf, src_dir: &Path, out_dir: &Path) -> Self {
        let file_stem = src.file_stem().unwrap();
        let is_index = file_stem.to_string_lossy() == "index";

        let irene = { 
            let base = src.strip_prefix(src_dir).unwrap()
                .parent().unwrap().to_owned();
            if is_index {
                base
            } else {
                base.join(file_stem)
            }
        };
        let uri = Path::new("/").join(&irene);
        let out = out_dir.join(&irene).join("index.html");

        Self {
            src,
            out,
            uri,
        }
    }
}


#[derive(Debug, Deserialize)]
struct FrontMatter {
    title: String,
    template: String,
}

#[derive(Debug)]
struct Page {
    title: String,
    template: String,
    content: String,
}
impl Page {
    fn new(triple: &PageTriple) -> Self {

        let parseopts = 
        ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let content = fs::read_to_string(&triple.src).unwrap();

        let Node::Root(Root {
            children,
            ..
        }) = markdown::to_mdast(&content, &parseopts).unwrap() else {
            panic!("cannot parse mdast")
        }; 

        let FrontMatter {
            title,
            template,
        } = children.get(0)
            .map(|node| match node {
                Node::Yaml(Yaml { value, .. }) => Some(value),
                _ => None
            })
            .flatten()
            .map(|s| serde_yaml::from_str::<FrontMatter>(s).ok())
            .flatten()
            .unwrap_or(FrontMatter {
                title: "NO TITLE".to_string(),
                template: "base-1.html".to_string(),
            });

        println!("title: {}, template: {}", title, template);

        Self {
            title,
            template,
            content,
        }
    }
}

fn render(tera: &Tera, page: Page, triple: PageTriple) -> std::io::Result<()> {

    // markdown-rs options
    let mdopts = Options {
        parse: ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                html_flow: true,
                html_text: true,
                ..Default::default()
            },
            ..Default::default()
        },
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    println!("{:?}", triple.src);

    println!("-- creating directories...");
    fs::create_dir_all(triple.out.parent().unwrap())?;

    println!("-- rendering markdown...");
    let content_html = match markdown::to_html_with_options(&page.content, &mdopts) {
        Ok(s) => s,
        Err(s) => s,
    };

    println!("-- rendering template...");
    let mut context = Context::new();
    context.insert("title", &page.title);
    context.insert("content", &content_html);

    let rendered = tera.render(&page.template, &context).unwrap();

    println!("-- writing to {:?}...", triple.out);
    fs::write(&triple.out, rendered)?;
    println!("-- done!");
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let src_dir = args.get(1)
        .map(String::clone)
        .unwrap_or(String::from("src"));

    let src_dir = Path::new(&src_dir);
    let out_dir = Path::new("public");

    if !out_dir.is_dir() {
        fs::create_dir(out_dir)?;
    }
    
    let mut tera = Tera::new("templates/**/*.html").unwrap();
    tera.autoescape_on(vec![]);

    let srcs = get_all_src(src_dir)?;
    for src in srcs {
        let triple = PageTriple::new(src, src_dir, out_dir);
        let page = Page::new(&triple);
        render(&tera, page, triple)?;
    }
   
    Ok(())
}


/// Returns the paths of all src files in a directory and all of its subdirectories
fn get_all_src(src_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = vec![];

    let mut dirstack: Vec<PathBuf> = vec![src_dir.to_owned()];
    while let Some(dir) = dirstack.pop() {
        // push subdirectories to stack
        let subdirs = fs::read_dir(&dir)?
            .filter_map(|entry| entry.ok()) 
            .map(|entry| entry.path())
            .filter(|path| path.is_dir());
        dirstack.extend(subdirs);

        // collect .md files
        let md_iter = fs::read_dir(&dir)? 
            .filter_map(|entry| entry.ok()) 
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| path
                        .extension()
                        .map(OsStr::to_str)
                        .flatten() == Some("md"));
        out.extend(md_iter);
    }

    Ok(out)
}


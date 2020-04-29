use scraper::{Html, Selector, ElementRef};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use scraper::html::Select;
use std::fmt::{Display, Formatter, Write};
use scraper::node::Node::Document;
use clap::{App, Arg, ArgSettings};
use std::process::exit;

fn main() {
    let a = App::new("Horrible Subs Downloader")
        .version("1.0")
        .author("powerboat9")
        .about("Gets a list of 1080p magnet links for shows on horriblesubs.info")
        .arg(
            Arg::with_name("only-links")
                .short("l")
                .help("only prints out magnet links, intended for scripts")
        )
        .arg(
            Arg::with_name("use-id")
                .short("i")
                .help("accepts a show id instead of a url")
        )
        .arg(
            Arg::with_name("SHOW")
                .help("the show to download, by default the show's url")
                .required(true)
                .index(1)
        )
        .get_matches();
    let id: u32 = if a.is_present("use-id") {
        match a.value_of("SHOW").unwrap().parse() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[ERROR] SHOW expected positive numeric id: {}", e);
                exit(-2)
            }
        }
    } else {
        match get_show_id(a.value_of("SHOW").unwrap()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ERROR]: {}", e);
                exit(-2)
            }
        }
    };
    let list = get_show_torrents(id, get_epoch()).unwrap();
    for e in list {
        if a.is_present("only-links") {
            println!("{}", e.1);
        } else {
            println!("name: {}", e.0);
            println!("link: {}", e.1);
        }
    }
}

#[derive(Debug)]
enum PageError {
    PageResponseError(u16),
    IoError(std::io::Error),
    ParseError
}

impl Display for PageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            PageError::PageResponseError(c) => f.write_fmt(format_args!("Response Error: {}", c)),
            PageError::IoError(e) => f.write_fmt(format_args!("IO Error: {}", e)),
            PageError::ParseError => f.write_str("Parsing Failure")
        }
    }
}

impl std::error::Error for PageError {
}

fn download_page(url: &str) -> Result<String, PageError> {
    let r = ureq::get(url)
        .timeout_connect(5000)
        .call();
    if r.ok() {
        r.into_string().map_err(|v| PageError::IoError(v))
    } else {
        Err(PageError::PageResponseError(r.status()))
    }
}

fn get_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("back in time").as_secs()
}

fn get_show_torrents(id: u32, epoch: u64) -> Result<Vec<(String, String)>, PageError> {
    let mut idx = 0;
    let mut ls = Vec::new();
    loop {
        let data = download_page(format!("https://horriblesubs.info/api.php?method=getshows&type=show&showid={}&nextid={}&_={}", id, idx, epoch).as_str())?;
        idx += 1;
        if data.as_str() == "DONE" {
            break
        }
        let div_find = Selector::parse(".rls-info-container").unwrap();
        let name_find = Selector::parse(".rls-label").unwrap();
        let link_find = Selector::parse(".link-1080p").unwrap();
        let link_extract = Selector::parse("a").unwrap();
        let h = Html::parse_document(data.as_str());
        for de in h.select(&div_find) {
            (|| {
                let name = de
                    .select(&name_find)
                    .next()?.text().collect::<String>();
                let link = de
                    .select(&link_find)
                    .next()?.select(&link_extract)
                    .next()?.value().attrs().filter_map(|v| {
                    if v.0 == "href" {
                        Some(v.1)
                    } else {
                        None
                    }
                }).next()?;
                ls.push((name, String::from(link)));
                Some(())
            })().ok_or(PageError::ParseError)?;
        }
    }
    Ok(ls)
}

fn get_name_from_tag(e: &ElementRef) -> String {
    println!("{:?}", e);
    e.html()
}

fn get_show_id(url: &str) -> Result<u32, PageError> {
    let data = download_page(url)?;
    let id_finder = Regex::new("<script type=\"text/javascript\">var hs_showid = (\\d+);</script>").unwrap();
    let id = id_finder.captures(data.as_str()).unwrap().get(1).ok_or(PageError::ParseError)?;
    id.as_str().parse().map_err(|_| PageError::ParseError)
}
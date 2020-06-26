use scraper::{Html, Selector};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::{Display, Formatter};
use clap::{App, Arg, SubCommand};
use std::process::exit;

extern crate ureq;
extern crate scraper;
extern crate regex;
extern crate clap;
#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), PageError> {
    let a = App::new("Horrible Subs Downloader")
        .version("1.0")
        .author("powerboat9")
        .about("Gets a list of 1080p magnet links for shows on horriblesubs.info")
        .subcommand(
            SubCommand::with_name("getmags")
                .about("Gets a list of magnet links from a show")
                .arg(
                    Arg::with_name("SHOW")
                        .help("the show to download, by default the show's url")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::with_name("batch")
                        .short("b")
                        .help("shows batches instead of episodes")
                )
                .arg(
                    Arg::with_name("res")
                        .short("r")
                        .value_name("RESOLUTION")
                        .help("sets the resolution to download in: 1080 | 720 | 480 | auto")
                        .takes_value(true)
                )
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List shows available on HorribleSubs")
        )
        .subcommand(
            SubCommand::with_name("getid")
                .about("Gets the id associated with a show url")
                .arg(
                    Arg::with_name("SHOW")
                        .help("the show to download, by default the show's url")
                        .required(true)
                        .index(1)
                )
        )
        .arg(
            Arg::with_name("only-links")
                .short("l")
                .help("only prints out urls, intended for scripts")
        )
        .arg(
            Arg::with_name("use-id")
                .short("i")
                .help("accept/display show ids instead of urls")
        )
        .get_matches();
    if let Some(submatch) = a.subcommand_matches("getmags") {
        let id = submatch.value_of("SHOW").unwrap();
        let id: u32 = if a.is_present("use-id") {
            match id.parse() {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("[ERROR] SHOW expected positive numeric id: {}", e);
                    exit(-2)
                }
            }
        } else {
            get_show_id(id)?
        };
        let link_type = match submatch.value_of("res") {
            Some("1080") => LinkType::Res1080,
            Some("720") => LinkType::Res720,
            Some("480") => LinkType::Res480,
            None | Some("auto") => LinkType::Auto,
            _ => return Err(PageError::CommandLineError("unrecognized resolution"))
        };
        let list = get_show_torrents(id, get_epoch(), link_type, submatch.is_present("batch"))?;
        let only_links = a.is_present("only-links");
        for e in list {
            if only_links {
                println!("{}", e.1);
            } else {
                println!("name: {}", e.0);
                println!("link: {}", e.1);
            }
        }
    } else if a.subcommand_matches("list").is_some() {
        let list = get_show_list()?;
        let only_links = a.is_present("only-links");
        let show_id = a.is_present("use-id");
        for e in list.iter() {
            match (only_links, show_id) {
                (false, false) => println!("{}: {}", e.0, e.1),
                (false, true) => println!("{}: {}", e.0, get_show_id(e.1.as_str())?),
                (true, false) => println!("{}", e.1),
                (true, true) => println!("{}", get_show_id(e.1.as_str())?)
            }
        }
    } else if let Some(submatch) = a.subcommand_matches("getid") {
        let id = submatch.value_of("SHOW").unwrap();
        let id: u32 = if a.is_present("use-id") {
            match id.parse() {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("[ERROR] SHOW expected positive numeric id: {}", e);
                    exit(-2)
                }
            }
        } else {
            get_show_id(id)?
        };
        println!("{}", id);
    } else {
        panic!("subcommand error")
    }
    Ok(())
}

#[derive(Debug)]
enum PageError {
    PageResponseError(u16),
    IoError(std::io::Error),
    ParseError(&'static str),
    CommandLineError(&'static str)
}

impl Display for PageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            PageError::PageResponseError(c) => f.write_fmt(format_args!("Response Error: {}", c)),
            PageError::IoError(e) => f.write_fmt(format_args!("IO Error: {}", e)),
            PageError::ParseError(s) => f.write_fmt(format_args!("Parsing Failure: {}", *s)),
            PageError::CommandLineError(s) => f.write_fmt(format_args!("Command Line Failure: {}", *s))
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

#[derive(Copy, Clone)]
enum LinkType {
    Res1080,
    Res720,
    Res480,
    Auto
}

fn get_show_torrents(id: u32, epoch: u64, link_type: LinkType, use_batches: bool) -> Result<Vec<(String, String)>, PageError> {
    let mut idx = 0;
    let mut ls = Vec::new();
    loop {
        let data = download_page(format!("https://horriblesubs.info/api.php?method=getshows&type={}&showid={}&nextid={}&_={}", if use_batches {
            "batch"
        } else {
            "show"
        }, id, idx, epoch).as_str())?;
        idx += 1;
        if data.as_str() == "DONE" || data.as_str() == "There are no batches for this show yet" {
            break
        }
        let div_find = Selector::parse(".rls-info-container").unwrap();
        let name_find = Selector::parse(".rls-label").unwrap();
        let link_1080_find = Selector::parse(".link-1080p").unwrap();
        let link_720_find = Selector::parse(".link-720p").unwrap();
        let link_480_find = Selector::parse(".link-480p").unwrap();
        let link_extract = Selector::parse("a").unwrap();
        let h = Html::parse_document(data.as_str());
        for de in h.select(&div_find) {
            (|| {
                let name = de
                    .select(&name_find)
                    .next()?.text().collect::<String>();
                let link = match link_type {
                    LinkType::Res1080 => de.select(&link_1080_find).next(),
                    LinkType::Res720 => de.select(&link_720_find).next(),
                    LinkType::Res480 => de.select(&link_480_find).next(),
                    LinkType::Auto => {
                        de.select(&link_1080_find).next()
                            .or_else(|| de.select(&link_720_find).next())
                            .or_else(|| de.select(&link_480_find).next())
                    }
                }?;
                let link = link
                    .select(&link_extract)
                    .next()?.value().attrs().filter_map(|v| {
                    if v.0 == "href" {
                        Some(v.1)
                    } else {
                        None
                    }
                }).next()?;
                ls.push((name, String::from(link)));
                Some(())
            })().ok_or(PageError::ParseError("failed to extract magnet link"))?;
        }
    }
    Ok(ls)
}

lazy_static! {
    static ref ID_FINDER: Regex = Regex::new("<script type=\"text/javascript\">var hs_showid = (\\d+);</script>").unwrap();
}

fn get_show_id(url: &str) -> Result<u32, PageError> {
    let data = download_page(url)?;
    let id = ID_FINDER.captures(data.as_str()).unwrap().get(1).ok_or(PageError::ParseError("failed to extract show id"))?;
    id.as_str().parse().map_err(|_| PageError::ParseError("failed to parse extracted show id"))
}

lazy_static! {
    static ref LINK_FINDER: Regex = Regex::new("<a href=\"([^\"]+)\" title=\"([^\"]+)\">").unwrap();
}

fn get_show_list() -> Result<Vec<(String, String)>, PageError> {
    let data = download_page("https://horriblesubs.info/shows")?;
    Ok(LINK_FINDER.captures_iter(data.as_str()).map(|caps| (caps.get(2).unwrap().as_str().to_owned(), format!("https://horriblesubs.info{}", caps.get(1).unwrap().as_str()))).collect())
}
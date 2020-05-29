use scraper::{Html, Selector};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::{Display, Formatter};
use clap::{App, Arg};
use std::process::exit;

fn main() {
    let a = App::new("Horrible Subs Downloader")
        .version("1.0")
        .author("powerboat9")
        .about("Gets a list of 1080p magnet links for shows on horriblesubs.info")
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
        .arg(
            Arg::with_name("batch")
                .short("b")
                .help("shows batches instead of episodes")
        )
        .arg(
            Arg::with_name("SHOW")
                .help("the show to download, by default the show's url, \"list\" to list shows")
                .required(true)
                .index(1)
        )
        .get_matches();
    let id = a.value_of("SHOW").unwrap();
    if id == "list" {
        let list = get_show_list().unwrap();
        let only_links = a.is_present("only-links");
        let show_id = a.is_present("use-id");
        for e in list.iter() {
            match (only_links, show_id) {
                (false, false) => println!("{}: {}", e.0, e.1),
                (false, true) => println!("{}: {}", e.0, get_show_id(e.1.as_str()).unwrap()),
                (true, false) => println!("{}", e.1),
                (true, true) => println!("{}", get_show_id(e.1.as_str()).unwrap())
            }
        }
    } else {
        let id: u32 = if a.is_present("use-id") {
            match id.parse() {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("[ERROR] SHOW expected positive numeric id: {}", e);
                    exit(-2)
                }
            }
        } else {
            match get_show_id(id) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[ERROR]: {}", e);
                    exit(-2)
                }
            }
        };
        let list = get_show_torrents(id, get_epoch(), a.is_present("batch")).unwrap();
        let only_links = a.is_present("only-links");
        for e in list {
            if only_links {
                println!("{}", e.1);
            } else {
                println!("name: {}", e.0);
                println!("link: {}", e.1);
            }
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

fn get_show_torrents(id: u32, epoch: u64, use_batches: bool) -> Result<Vec<(String, String)>, PageError> {
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

fn get_show_id(url: &str) -> Result<u32, PageError> {
    let data = download_page(url)?;
    let id_finder = Regex::new("<script type=\"text/javascript\">var hs_showid = (\\d+);</script>").unwrap();
    let id = id_finder.captures(data.as_str()).unwrap().get(1).ok_or(PageError::ParseError)?;
    id.as_str().parse().map_err(|_| PageError::ParseError)
}

fn get_show_list() -> Result<Vec<(String, String)>, PageError> {
    let data = download_page("https://horriblesubs.info/shows")?;
    let link_finder = Regex::new("<a href=\"([^\"]+)\" title=\"([^\"]+)\">").unwrap();
    Ok(link_finder.captures_iter(data.as_str()).map(|caps| (caps.get(2).unwrap().as_str().to_owned(), format!("https://horriblesubs.info{}", caps.get(1).unwrap().as_str()))).collect())
}
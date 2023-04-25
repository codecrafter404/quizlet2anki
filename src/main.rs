use std::time::{SystemTime, UNIX_EPOCH};

use color_eyre::eyre::ContextCompat;
use genanki_rs::{Deck, Note};
use reqwest::header::{HeaderMap, HeaderValue};
use scraper::Selector;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let url = std::env::args().nth(1).context(get_help_message())?;
    let out = std::env::args().nth(2);
    if url.is_empty() {
        println!("{}", get_help_message());
        return Err(color_eyre::eyre::eyre!("Invalid url"));
    }
    let client = reqwest::Client::new();
    println!("Connecting to quizlet");
    let resp = client
        .get(&url)
        .headers(get_stealth_headers())
        .send()
        .await?
        .text()
        .await?;
    println!("Parsing html");
    let (cards, title) = parse_html(&resp)?;
    let mut out_file = match out {
        Some(x) => x,
        None => match title.trim().strip_suffix(" Flashcards | Quizlet") {
            Some(title) => title.to_owned(),
            None => title.clone(),
        },
    };
    // ensure file ends with .apkg
    if !out_file.ends_with(".apkg") {
        out_file.push_str(".apkg");
    }
    println!("Parsed {} cards", cards.len());
    write_cards_to_file(&out_file, &title, &cards)?;
    println!("Wrote cards to '{}'", out_file);
    Ok(())
}
// -> Vec<(Question, Awnser)>
fn parse_html(html: &str) -> color_eyre::Result<(Vec<(String, String)>, String)> {
    let document = scraper::Html::parse_document(html);

    let section_selector = Selector::parse("section.SetPageTerms-termsList").unwrap();
    let term_selector = Selector::parse(r#"div[aria-label="Term"]"#).unwrap();
    let term_smallside = Selector::parse("div.SetPageTerm-smallSide").unwrap();
    let term_largeside = Selector::parse("div.SetPageTerm-largeSide").unwrap();
    let invisible_div =
        Selector::parse(r#"div.SetPage-terms > div[style="display:none"]"#).unwrap();
    let sel_span = Selector::parse("span").unwrap();
    let title_sel = Selector::parse("title").unwrap();

    let section = document
        .select(&section_selector)
        .next()
        .context("Can't select the termlist")?;

    let mut results = Vec::new();
    for div in section.select(&term_selector) {
        let smallside = div
            .select(&term_smallside)
            .next()
            .context("Failed to find smallside")?
            .text()
            .next()
            .context("smallside dosn't contain text")?;
        let largeside = div
            .select(&term_largeside)
            .next()
            .context("Failed to find largeside")?
            .text()
            .next()
            .context("largeside dosn't contain text")?;
        results.push((smallside.to_owned(), largeside.to_owned()))
    }
    let div = document
        .select(&invisible_div)
        .next()
        .context("Failed to find invisible div")?;
    for x in div.select(&sel_span).collect::<Vec<_>>().chunks(2) {
        results.push((
            x[0].text().next().context("Couldn't find text")?.to_owned(),
            x[1].text().next().context("Couldn't find text")?.to_owned(),
        ));
    }
    Ok((
        results,
        document
            .select(&title_sel)
            .next()
            .context("Couldn't find title")?
            .text()
            .next()
            .context("Title has no text")?
            .to_owned(),
    ))
}

fn get_help_message<'a>() -> &'a str {
    "Usage: ./quizlet-anki-export <Quizlet-Url> <(Output-File.apkg)>"
}
fn get_stealth_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.5615.50 Safari/537.36)"));
    headers
}
fn write_cards_to_file(file: &str, title: &str, cards: &Vec<(String, String)>) -> color_eyre::Result<()> {
    let model = genanki_rs::basic_and_reversed_card_model();
    let notes = cards
        .clone()
        .into_iter()
        .map(|x| Note::new(model.clone(), vec![&x.0, &x.1]).expect("Invalid card"))
        .collect::<Vec<_>>();
    let mut deck = Deck::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64() as i64,
        title,
        &format!("Exported from quizlet '{}'", file),
    );
    notes.into_iter().for_each(|x| deck.add_note(x));
    deck.write_to_file(file)?;
    Ok(())
}

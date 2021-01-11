use std::borrow::Cow;
use std::error::Error;

use handlebars::*;
use once_cell::sync::Lazy;
use serde::Serialize;
use woothee::parser::Parser;

use crate::server;
use crate::Config;

static AGENT_PARSER: Lazy<Parser> = Lazy::new(|| Parser::new());

#[derive(Debug, Serialize)]
struct Browser<'a> {
    name: &'a str,
    category: &'a str,
    os: &'a str,
    os_version: Cow<'a, str>,
    browser_type: &'a str,
    version: &'a str,
    vendor: &'a str,
}

impl Browser<'_> {
    fn parse(agent: &str) -> Option<Browser<'_>> {
        AGENT_PARSER.parse(agent).map(|w| Browser {
            name: w.name,
            category: w.category,
            os: w.os,
            os_version: w.os_version,
            browser_type: w.browser_type,
            version: w.version,
            vendor: w.vendor,
        })
    }
}

#[derive(Debug, Serialize)]
struct Vars<'a, 'b> {
    browser: Option<Browser<'a>>,
    sizes: &'b Vec<String>,
}

pub fn build(config: &Config, agent: String) -> Result<String, Box<dyn Error + Sync + Send>> {
    let mut hbs = Handlebars::new();
    if let Some(file) = config.index.file.as_ref() {
        hbs.register_template_file("index", file)?;
    } else {
        let index = include_str!("index.hbs");
        hbs.register_template_string("index", index)?;
    }

    handlebars_helper!(size: |sz: str| {
        server::size(sz).unwrap_or(0)
    });
    handlebars_helper!(contains: |haystack: str, needle: str| {
        haystack.contains(needle)
    });
    hbs.register_helper("size", Box::new(size));
    hbs.register_helper("contains", Box::new(contains));

    let vars = Vars {
        browser: Browser::parse(&agent),
        sizes: &config.index.sizes,
    };

    Ok(hbs.render("index", &vars)?)
}

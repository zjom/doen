use std::collections::BTreeMap;

use crate::app::App;
use crate::cli::args::TagsCmd;
use crate::error::Result;
use crate::filter::{Query, StatusFilter};
use crate::item::Place;

pub fn run(app: &App, c: TagsCmd) -> Result<()> {
    let q = if c.all {
        Query {
            places: vec![Place::Root, Place::Archive],
            status: StatusFilter::Any,
            ..Query::default()
        }
    } else {
        Query {
            places: vec![Place::Root],
            ..Query::default()
        }
    };
    let store = app.store(&q.places)?;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for i in store.items.iter().filter(|i| q.accepts(i, &app.clock)) {
        for t in &i.meta.tags {
            *counts.entry(t).or_default() += 1;
        }
    }
    if c.json {
        let v: Vec<serde_json::Value> = counts
            .iter()
            .map(|(t, n)| serde_json::json!({"tag": t, "count": n}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&v).expect("JSON"));
    } else if c.porcelain {
        for (t, n) in &counts {
            println!("{t}\t{n}");
        }
    } else {
        let w = counts
            .keys()
            .map(|t| t.chars().count() + 1)
            .max()
            .unwrap_or(0);
        for (t, n) in &counts {
            println!(
                "{}  {n}",
                app.paint("36", &format!("{:<w$}", format!("#{t}")))
            );
        }
    }
    Ok(())
}

//! `show` and `tags`.

use std::collections::BTreeMap;

use crate::cli::args::{ShowCmd, TagsCmd};
use crate::commands::{App, expand_ids};
use crate::error::Result;
use crate::filter::{Query, StatusFilter};
use crate::ids;
use crate::item::{Item, Place};
use crate::output::{print_machine, summary};
use crate::tags::TagFilter;

pub fn show(app: &App, c: ShowCmd) -> Result<()> {
    let store = app.store()?;
    let idx = ids::resolve_all(&store, &expand_ids(&c.id)?, true)?;
    let items: Vec<&Item> = idx.iter().map(|&n| &store.items[n]).collect();
    if print_machine(&items, &c.format) {
        return Ok(());
    }
    if c.path {
        for i in &items {
            println!("{}", i.path.display());
        }
        return Ok(());
    }
    for (k, i) in items.iter().enumerate() {
        if c.raw {
            if items.len() > 1 {
                if k > 0 {
                    println!();
                }
                println!("==> {} <==", i.path.display());
            }
            print!("{}", std::fs::read_to_string(&i.path)?);
            continue;
        }
        if k > 0 {
            println!();
        }
        println!("{}", app.paint("1", i.title()));
        println!("{}", app.paint("2", &summary(i, app)));
        let content = i.content();
        if !content.is_empty() {
            println!();
            println!("{content}");
        }
    }
    Ok(())
}

pub fn tags(app: &App, c: TagsCmd) -> Result<()> {
    let store = app.store()?;
    let q = if c.all {
        Query::scope(
            vec![Place::Root, Place::Archive],
            StatusFilter::Any,
            TagFilter::default(),
        )
    } else {
        Query::scope(vec![Place::Root], StatusFilter::Open, TagFilter::default())
    };
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

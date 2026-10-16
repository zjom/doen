use crate::app::App;
use crate::cli::args::ShowCmd;
use crate::commands::resolve_ids;
use crate::error::Result;
use crate::item::Item;
use crate::output::{print_machine, summary};

pub fn show(app: &App, c: ShowCmd) -> Result<()> {
    let store = app.store()?;
    let idx = resolve_ids(&store, &c.id, true)?;
    let items: Vec<&Item> = idx.iter().map(|&n| &store.items[n]).collect();
    if print_machine(&items, c.format.format()) {
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

extern crate steam;

use clap::{App, Arg};
use failure::Error;
use serde_json;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use steam::{app_info::AppInfo, steam_game::SteamGame};

fn main() -> Result<(), Error> {
    let matches = App::new("steam")
        .about("List and launch games from your local Steam library")
        .arg(
            Arg::with_name("list")
                .long("list")
                .short("l")
                .help("List games"),
        )
        .arg(
            Arg::with_name("raw-list")
                .long("raw-list")
                .help("List all apps in the steam metadata"),
        )
        .arg(
            Arg::with_name("type")
                .long("type")
                .short("t")
                .takes_value(true)
                .default_value("game")
                .help("Dump game metadata"),
        )
        .arg(
            Arg::with_name("max")
                .long("max")
                .short("m")
                .takes_value(true)
                .help("Dump game metadata"),
        )
        .arg(
            Arg::with_name("dump")
                .long("dump")
                .short("d")
                .takes_value(true)
                .multiple(true)
                .help("Dump game metadata"),
        )
        .arg(
            Arg::with_name("prop")
                .long("prop")
                .short("p")
                .takes_value(true)
                .help("Retrieve the specified property"),
        )
        .arg(
            Arg::with_name("json")
                .long("json")
                .short("j")
                .help("Display output as json"),
        )
        .arg(
            Arg::with_name("installed")
                .long("installed")
                .short("i")
                .takes_value(true)
                .help("Only show installed or uninstalled games"),
        )
        .get_matches();

    let mut count = 0;
    let max = usize::from_str(matches.value_of("max").unwrap_or("1000"))
        .expect("Unable to parse 'max' parameter.");

    let app_infos = AppInfo::load()?;
    let mut games = SteamGame::from(&app_infos)?;
    if matches.is_present("list") {
        games.sort_unstable_by(|e1, e2| e1.title.cmp(&e2.title));
        if let Some(installed) = matches.value_of("installed") {
            let installed = bool::from_str(installed)?;
            games = games
                .into_iter()
                .filter(|g| g.installed == installed)
                .collect();
        }
        if matches.is_present("json") {
            let games_to_export: Vec<&SteamGame> = games.iter().take(max).collect();
            println!("{}", serde_json::to_string(&games_to_export)?);
        } else {
            for game in games.iter().take(max) {
                println!(
                    "{} {} {:?} {}",
                    game.id, game.title, game.logo, game.installed
                );
            }
        }
    }
    let path: Option<Vec<&str>> = match matches.value_of("prop") {
        None => None,
        Some(prop) => Some(prop.split(",").collect()),
    };
    if let Some(ids) = matches.values_of("dump") {
        for id in ids {
            println!("{}", id);
            let id = u32::from_str(id)?;
            for app_info in &app_infos {
                if app_info.u32_entry(&["appinfo", "appid"]).unwrap() == id {
                    println!("State: {:#X}", app_info.state);
                    if path.is_some() {
                        app_info.print_entry(path.as_ref().unwrap());
                    } else {
                        app_info.print_props(100);
                    }
                }
            }
        }
    }

    if matches.is_present("raw-list") {
        for app_info in &app_infos {
            count += 1;
            println!(
                "{} {} {} {}",
                app_info.u32_entry(&["appinfo", "appid"]).unwrap_or(0),
                app_info
                    .string_entry(&["appinfo", "common", "type"])
                    .unwrap_or("none".to_string()),
                app_info
                    .string_entry(&["appinfo", "common", "name"])
                    .unwrap_or("none".to_string()),
                if path.is_some() {
                    app_info.format_entry(path.as_ref().unwrap())
                } else {
                    "-".to_string()
                }
            );
            //app_info.print_props(100);
            if count > max {
                break;
            }
        }
    }

    Ok(())
}

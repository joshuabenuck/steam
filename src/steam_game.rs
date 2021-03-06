use crate::app_info::AppInfo;
use crate::package_info::PackageInfo;
use anyhow::Error;
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Serialize)]
pub struct SteamGame {
    pub id: u32,
    pub title: String,
    pub logo: Option<String>,
    pub installed: bool,
}

impl SteamGame {
    pub fn from(
        app_infos: &Vec<AppInfo>,
        pkg_infos: &Vec<PackageInfo>,
    ) -> Result<Vec<SteamGame>, Error> {
        let lib_folders_vdf =
            fs::File::open("c:/program files (x86)/steam/steamapps/libraryfolders.vdf")?;
        let mut lib_folders = Vec::new();
        lib_folders.push(PathBuf::from("c:/program files (x86)/steam/steamapps/"));
        for line in BufReader::new(lib_folders_vdf).lines() {
            let mut line = line?;
            line = line.trim().to_string();
            let mut parts = line.split("\t").filter(|p| p.len() > 0);
            let name = parts.next().unwrap().replace("\"", "");
            if usize::from_str(&name).is_ok() {
                let value = parts.next().unwrap().replace("\"", "");
                lib_folders.push(PathBuf::from(value.replace("\\\\", "\\")).join("steamapps"));
            }
        }
        eprintln!("Additional library folders to check: {:#?}", &lib_folders);
        let mut games = Vec::new();
        let owned_games = {
            let mut owned_games = Vec::<u32>::new();
            for pkg_info in pkg_infos {
                // let id = pkg_info.id;
                let app_ids = pkg_info.map_entry(&["appids"]).unwrap();
                if app_ids.keys().len() > 1 {
                    // continue;
                }
                let game_id = pkg_info.u32_entry(&["appids", "0"]).unwrap();
                owned_games.push(game_id);
            }
            owned_games
        };
        for app_info in app_infos {
            let app_id = app_info.u32_entry(&["appinfo", "appid"]).unwrap();
            if !owned_games.contains(&app_id) {
                continue;
            }
            let name = app_info.string_entry(&["appinfo", "common", "name"]);
            if name.is_none() {
                continue;
            }
            let r#type = app_info.string_entry(&["appinfo", "common", "type"]);
            if r#type.is_none()
                || !(r#type.as_ref().unwrap() == "Game" || r#type.as_ref().unwrap() == "game")
            {
                continue;
            }
            let name = name.unwrap();
            //let logo = app_info.string_entry(&["appinfo", "common", "logo"]);
            let mut logo = Some(format!(
                "c:/program files (x86)/steam/appcache/librarycache/{}_library_600x900.jpg",
                app_id.to_string()
            ));
            if !PathBuf::from(logo.as_ref().unwrap()).exists() {
                logo = None;
            }
            let mut installed = false;
            for folder in &lib_folders {
                if folder
                    .join(format!("appmanifest_{}.acf", app_id.to_string()))
                    .exists()
                {
                    installed = true;
                }
            }
            games.push(SteamGame {
                id: app_id,
                title: name,
                logo,
                installed,
            });
        }
        Ok(games)
    }
}

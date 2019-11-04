// This is a port of my unpublished Go VDF parser.
// Parsing is based on information found in various places.

// VDF parsing info
// https://github.com/manveru/go-steam/vdf/vdf.go
// https://github.com/ValvePython/vdf/issues/13
// http://developers-club.com/posts/268921/#VDF
// https://github.com/SkaceKamen/Wox.Plugin.Steam/blob/master/WoxSteam/BinaryVdf/Reader.cs
// https://github.com/leovp/steamfiles/blob/master/steamfiles/appinfo.py
// https://github.com/barneygale/bvdf/blob/master/bvdf.py
// https://github.com/Theo47/Depressurizer/blob/dev/src/Depressurizer/VdfFile/VdfFileNode.cs

// appinfo.vdf has client icon guid
// file is stored in steam/steam/games/guid.ico

use clap::{App, Arg};
use failure::{err_msg, Error};
use glob::glob;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::str::FromStr;

fn u8(buf: &[u8], pos: &mut usize) -> u8 {
    let value = buf[*pos];
    *pos += 1;
    value
}

fn be_u16(buf: &[u8], pos: &mut usize) -> u16 {
    let value = u16::from_be_bytes(buf[*pos..*pos + 2].try_into().unwrap());
    *pos += 2;
    value
}

fn le_u32(buf: &[u8], pos: &mut usize) -> u32 {
    let value = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    value
}

fn le_u64(buf: &[u8], pos: &mut usize) -> u64 {
    let value = u64::from_le_bytes(buf[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;
    value
}

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
                .help("Dump game metadata"),
        )
        .get_matches();

    let mut count = 0;
    let max = usize::from_str(matches.value_of("max").unwrap_or("1000"))
        .expect("Unable to parse 'max' parameter.");

    let app_infos = AppInfo::load()?;
    let mut games = SteamGame::from(&app_infos)?;
    if matches.is_present("list") {
        games.sort_unstable_by(|e1, e2| e1.title.cmp(&e2.title));
        for game in games.iter().filter(|g| g.installed) {
            println!("{} {} {:?}", game.id, game.title, game.logo);
            count += 1;
            if count > max {
                break;
            }
        }
    }
    if let Some(id) = matches.value_of("dump") {
        let id = u32::from_str(id)?;
        for app_info in &app_infos {
            if app_info.u32_entry(&["appinfo", "appid"]).unwrap() == id {
                app_info.print_props(100);
            }
        }
    }

    /*println!("{} {}", &file.display(), &id);
    let file = fs::File::open(file)?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.contains("name") {
            let name = line
                .split("\"name\"")
                .collect::<Vec<&str>>()
                .last()
                .unwrap()
                .trim();
            println!("name: {}", name);
            break;
        }
    }*/

    Ok(())
}

#[derive(Debug)]
enum Property {
    Uint32(u32),
    Uint64(u64),
    Map(HashMap<String, Property>),
    String(String),
}

#[derive(Debug)]
struct AppInfo {
    state: u32,
    last_updated: u32,
    access_token: u64,
    checksum: [u8; 20],
    change_no: u32,
    props: HashMap<String, Property>,
}

impl AppInfo {
    pub fn load() -> Result<Vec<AppInfo>, Error> {
        let mut buf = Vec::new();
        fs::File::open("appinfo.vdf")?.read_to_end(&mut buf)?;
        let mut pos = 0;
        println!("appinfo: {} bytes", buf.len());
        let version = u8(&buf, &mut pos);
        // Doc only knows about 24 and 26. My file has 27. What other diffs are there?
        if version != 0x24 && version != 0x26 && version != 0x27 {
            return Err(err_msg(format!("Unknown version: {:x}", version)));
        }
        let type_sig = be_u16(&buf, &mut pos);
        if type_sig != 0x4456 {
            // DV
            return Err(err_msg(format!(
                "File doesn't contain type sig 'DV': 0x{:x}",
                type_sig
            )));
        }
        let version = u8(&buf, &mut pos);
        if version != 0x06 && version != 0x07 {
            return Err(err_msg(format!("Unknown version2: 0x{:x}", version)));
        }
        let version = le_u32(&buf, &mut pos);
        if version != 0x01 {
            return Err(err_msg(format!("Version3 must be 0x01: 0x{:x}", version)));
        }
        let mut app_infos = Vec::new();
        loop {
            let app_id = le_u32(&buf, &mut pos);
            if app_id == 0x00 {
                break;
            }
            let size: usize = le_u32(&buf, &mut pos) as usize;
            app_infos.push(parse_app_info(&buf[pos..pos + size])?);
            pos += size;
        }
        Ok(app_infos)
    }
    pub fn print_props(&self, depth: usize) {
        self.print_props_helper(&self.props, depth, &"".to_owned());
    }

    // internal helper
    fn print_props_helper(&self, props: &HashMap<String, Property>, depth: usize, prefix: &str) {
        for key in props.keys() {
            let value = props.get(key).unwrap();
            if let Property::Map(nested_props) = value {
                println!("{}{} (map)", prefix, key);
                if depth > 0 {
                    self.print_props_helper(
                        nested_props,
                        depth - 1,
                        format!("{}\t", prefix).as_str(),
                    );
                }
            } else {
                println!("{}{} {:?}", prefix, key, value);
            }
        }
    }

    fn string_entry(&self, path: &[&str]) -> Option<String> {
        match self.entry(path) {
            Some(Property::String(string)) => Some(string.to_owned()),
            _ => None,
        }
    }

    fn u32_entry(&self, path: &[&str]) -> Option<u32> {
        match self.entry(path) {
            Some(Property::Uint32(uint32)) => Some(*uint32),
            _ => None,
        }
    }

    fn u64(&self, path: &[&str]) -> Option<u64> {
        match self.entry(path) {
            Some(Property::Uint64(uint64)) => Some(*uint64),
            _ => None,
        }
    }

    fn entry(&self, path: &[&str]) -> Option<&Property> {
        let mut props = &self.props;
        let mut value = None;
        let mut terminal = false;
        for segment in path {
            if terminal {
                // We've reached a terminal property before reaching the
                // last path segment.
                return None;
            }
            value = props.get(*segment);
            if value.is_none() {
                // Unable to find a path segment.
                return None;
            }
            match value.unwrap() {
                Property::Map(nested_props) => props = nested_props,
                _ => terminal = true,
            }
        }
        value
    }
}

fn string(buf: &[u8], pos: &mut usize) -> Result<String, Error> {
    let begin = *pos;
    loop {
        if buf[*pos] == 0x00 {
            break;
        }
        *pos += 1;
    }
    let value = String::from_utf8(buf[begin..*pos].to_vec())?;
    *pos += 1;
    Ok(value)
}

fn parse_app_info(buf: &[u8]) -> Result<AppInfo, Error> {
    let mut pos = 0;
    let state = le_u32(&buf, &mut pos);
    let last_updated = le_u32(&buf, &mut pos);
    let access_token = le_u64(&buf, &mut pos);
    let checksum = buf[pos..pos + 20].try_into().unwrap();
    pos += 20;
    let change_no = le_u32(&buf, &mut pos);
    let mut nesting_level = 0;
    let mut top_level_props = HashMap::new();
    let mut props = &mut top_level_props;
    let mut path = Vec::<String>::new();
    loop {
        let r#type = u8(&buf, &mut pos);
        //println!("type: 0x{:x}", r#type);
        match r#type {
            0x00 => {
                // begin map
                nesting_level += 1;
                let name = string(&buf, &mut pos)?;
                &path.push(name.to_owned());
                props.insert(name.to_owned(), Property::Map(HashMap::new()));
                match props.get_mut(&name).unwrap() {
                    Property::Map(nested_props) => {
                        props = nested_props;
                    }
                    _ => {
                        panic!("Unable to get nested properties.");
                    }
                }
            }
            0x08 => {
                // end map
                nesting_level -= 1;
                let _unused = &path.pop();
                props = &mut top_level_props;
                for name in &path {
                    props = match props.get_mut(name).unwrap() {
                        Property::Map(nested_props) => nested_props,
                        _ => panic!("Unable to walk back"),
                    }
                }
            }
            0x01 => {
                // string
                let name = string(&buf, &mut pos)?;
                let value = string(&buf, &mut pos)?;
                props.insert(name, Property::String(value));
            }
            0x02 => {
                // uint32
                let name = string(&buf, &mut pos)?;
                let value = le_u32(&buf, &mut pos);
                props.insert(name, Property::Uint32(value));
            }
            0x07 => {
                // uint64 (unimplemented)
            }
            _ => {
                println!("Unknown section type: 0x{:x}", r#type);
            }
        }
        if nesting_level == 0 && r#type == 0x08 {
            break;
        }
    }
    Ok(AppInfo {
        state,
        last_updated,
        access_token,
        checksum,
        change_no,
        props: top_level_props,
    })
}

struct SteamGame {
    id: u32,
    title: String,
    logo: Option<String>,
    installed: bool,
}

impl SteamGame {
    fn from(app_infos: &Vec<AppInfo>) -> Result<Vec<SteamGame>, Error> {
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
        println!("Additional library folders to check: {:#?}", &lib_folders);
        let mut games = Vec::new();
        for app_info in app_infos {
            let app_id = app_info.u32_entry(&["appinfo", "appid"]).unwrap();
            let name = app_info.string_entry(&["appinfo", "common", "name"]);
            if name.is_none() {
                continue;
            }
            let r#type = app_info.string_entry(&["appinfo", "common", "type"]);
            if r#type.is_none() || r#type.unwrap() != "Game" {
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

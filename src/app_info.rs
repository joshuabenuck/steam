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

// userdata/id/config/shortcuts.vdf - executables for all non-Steam
// userdata/id/760/screenshots.vdf - names and ids for all non-Steam
// userdata/id/config/grid/*.jpg - logo images for all non-Steam

// https://github.com/michikora/Wox.Plugin.SteamLAUNCHER/blob/master/launcher.py
// https://github.com/SkaceKamen/Wox.Plugin.Steam/blob/master/WoxSteam/Game.cs

use anyhow::{anyhow, Error};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::io::Read;

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

#[derive(Debug)]
pub enum Property {
    Uint32(u32),
    Uint64(u64),
    Map(HashMap<String, Property>),
    String(String),
}

#[derive(Debug)]
pub struct AppInfo {
    pub state: u32,
    pub last_updated: u32,
    pub access_token: u64,
    pub checksum: [u8; 20],
    pub change_no: u32,
    pub props: HashMap<String, Property>,
}

impl AppInfo {
    pub fn load() -> Result<Vec<AppInfo>, Error> {
        let mut buf = Vec::new();
        fs::File::open("c:/program files (x86)/steam/appcache/appinfo.vdf")?
            .read_to_end(&mut buf)?;
        let mut pos = 0;
        let version = u8(&buf, &mut pos);
        // Doc only knows about 24 and 26. My file has 27. What other diffs are there?
        if version != 0x24 && version != 0x26 && version != 0x27 && version != 0x28 {
            return Err(anyhow!("Unknown version: {:x}", version));
        }
        let type_sig = be_u16(&buf, &mut pos);
        if type_sig != 0x4456 {
            // DV
            return Err(anyhow!(
                "File doesn't contain type sig 'DV': 0x{:x}",
                type_sig
            ));
        }
        let version = u8(&buf, &mut pos);
        if version != 0x06 && version != 0x07 {
            return Err(anyhow!("Unknown version2: 0x{:x}", version));
        }
        let version = le_u32(&buf, &mut pos);
        if version != 0x01 {
            return Err(anyhow!("Version3 must be 0x01: 0x{:x}", version));
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
    pub fn print_props_helper(
        &self,
        props: &HashMap<String, Property>,
        depth: usize,
        prefix: &str,
    ) {
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

    pub fn string_entry(&self, path: &[&str]) -> Option<String> {
        match self.entry(path) {
            Some(Property::String(string)) => Some(string.to_owned()),
            _ => None,
        }
    }

    pub fn u32_entry(&self, path: &[&str]) -> Option<u32> {
        match self.entry(path) {
            Some(Property::Uint32(uint32)) => Some(*uint32),
            _ => None,
        }
    }

    pub fn u64(&self, path: &[&str]) -> Option<u64> {
        match self.entry(path) {
            Some(Property::Uint64(uint64)) => Some(*uint64),
            _ => None,
        }
    }

    pub fn format_entry(&self, path: &[&str]) -> String {
        match self.entry(path) {
            None => format!("None"),
            Some(Property::Uint32(uint32)) => format!("{}", uint32),
            Some(Property::Uint64(uint64)) => format!("{}", uint64),
            Some(Property::String(string)) => format!("{}", string),
            Some(Property::Map(map)) => "(map)".to_string(),
        }
    }

    pub fn print_entry(&self, path: &[&str]) {
        match self.entry(path) {
            None => println!("None"),
            Some(Property::Uint32(uint32)) => println!("{}", uint32),
            Some(Property::Uint64(uint64)) => println!("{}", uint64),
            Some(Property::String(string)) => println!("{}", string),
            Some(Property::Map(map)) => self.print_props_helper(&map, 1000, ""),
        }
    }

    pub fn entry(&self, path: &[&str]) -> Option<&Property> {
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

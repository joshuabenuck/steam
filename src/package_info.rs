// This is a packageinfo.vdf parser based on data gathered from various places

// https://github.com/leovp/steamfiles/issues/3
// https://github.com/ValvePython/vdf/issues/13

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

#[derive(Debug)]
pub struct PackageInfo {
    pub id: u32,
    pub props: HashMap<String, Property>,
}

impl PackageInfo {
    pub fn load() -> Result<Vec<PackageInfo>, Error> {
        let mut buf = Vec::new();
        fs::File::open("c:/program files (x86)/steam/appcache/packageinfo.vdf")?
            .read_to_end(&mut buf)?;
        let mut pos = 0;
        let version = u8(&buf, &mut pos);
        // Doc only knows about 24 and 26. My file has 27. What other diffs are there?
        if version != 0x24 && version != 0x26 && version != 0x27 && version != 0x28 {
            return Err(anyhow!("Unknown version: {:x}", version));
        }
        let type_sig = be_u16(&buf, &mut pos);
        if type_sig != 0x5556 {
            // DV
            return Err(anyhow!(
                "File doesn't contain type sig 'DV': 0x{:x}",
                type_sig
            ));
        }
        let version2 = u8(&buf, &mut pos);
        if version2 != 0x06 && version2 != 0x07 {
            return Err(anyhow!("Unknown version2: 0x{:x}", version2));
        }
        let version3 = le_u32(&buf, &mut pos);
        if version3 != 0x01 {
            return Err(anyhow!("Version3 must be 0x01: 0x{:x}", version3));
        }
        let mut package_infos = Vec::new();
        loop {
            let pkg_id = le_u32(&buf, &mut pos);
            // println!("{} {:#X}", pkg_id, pkg_id);
            if pkg_id == 0xFFFFFFFF {
                break;
            }
            // version 28, skip 28... otherwise skip 20
            if version == 0x28 {
                pos += 28;
            } else {
                pos += 20;
            }
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
                    pos += 1;
                    break;
                }
            }
            let root_key = top_level_props.keys().next().unwrap().clone();
            let real_root_map = match top_level_props.remove(&root_key).unwrap() {
                Property::Map(map) => map,
                _ => panic!("Unable to get root property"),
            };
            package_infos.push(PackageInfo {
                id: pkg_id,
                props: real_root_map,
            });
        }
        Ok(package_infos)
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

    pub fn map_entry(&self, path: &[&str]) -> Option<&HashMap<String, Property>> {
        match self.entry(path) {
            Some(Property::Map(map)) => Some(map),
            _ => None,
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
}

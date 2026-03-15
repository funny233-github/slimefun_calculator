use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::de::from_str;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io::Read;

#[derive(Debug, Subcommand)]
enum Purpose {
    Recipe,
    Use,
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The File of Recipe Table
    file: String,

    query: String,

    #[command(subcommand)]
    purpose: Purpose,

    output: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum MaterialEntry {
    WithCount((String, u32)),
    Single((String,)),
}

impl MaterialEntry {
    fn name(&self) -> &str {
        match self {
            MaterialEntry::WithCount(tuple) => &tuple.0,
            MaterialEntry::Single(tuple) => &tuple.0,
        }
    }

    fn count(&self) -> u32 {
        match self {
            MaterialEntry::WithCount(tuple) => tuple.1,
            MaterialEntry::Single(_) => 1,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Recipe {
    machine: String,
    #[serde(rename = "materialList")]
    material_list: Vec<MaterialEntry>,
    count: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct RecipeTable {
    #[serde(rename = "craftTable")]
    craft_table: BTreeMap<String, Recipe>,
}

impl RecipeTable {
    fn is_base_material(&self, name: &str) -> bool {
        self.craft_table.iter().all(|(n, _)| *n != name)
    }

    fn get_base_material_nums(&self, name: &str, num: u32) -> BTreeMap<String, u32> {
        let recipe = self.craft_table.get(name).unwrap();
        let mut res = BTreeMap::new();
        recipe.material_list.iter().for_each(|material_entry| {
            let name = material_entry.name();
            let count = material_entry.count();

            if self.is_base_material(name) {
                res.insert(name.to_string(), count * num);
                return;
            }

            // push base material to res
            let base_material = self.get_base_material_nums(name, count * num);
            base_material.iter().for_each(|(name, count)| {
                match res.get_mut(name) {
                    Some(c) => *c += count,
                    None => {
                        res.insert(name.to_string(), *count);
                    }
                };
            });
        });

        res
    }

    fn material_list(&self, name: &str) -> Option<Vec<MaterialEntry>> {
        self.craft_table.get(name).cloned().map(|x| x.material_list)
    }

    #[allow(unused)]
    fn material_names(&self, name: &str) -> Vec<String> {
        let mut material: Vec<(String, bool)> = Vec::new();
        material.push((name.to_string(), false));
        while material.iter().any(|(_, calced)| !calced) {
            let mut buf = Vec::new();
            for (name, calced) in material.iter_mut().filter(|(_, calced)| !calced) {
                *calced = true;
                if self.is_base_material(name) {
                    continue;
                }
                let list = &self.material_list(name).unwrap();
                for i in list {
                    buf.push((i.name().to_string(), false))
                }
            }

            for (name, cacled) in buf {
                material
                    .iter_mut()
                    .filter(|(n, _)| *n == name)
                    .for_each(|(_, c)| *c = cacled);
                if material.iter().all(|(n, _)| *n != name) {
                    material.push((name, cacled));
                }
            }
        }
        material.iter().cloned().map(|(name, _)| name).collect()
    }

    fn _calc_material_inner(
        &self,
        material: &mut Vec<(String, u32, bool)>,
        surplus: &mut BTreeMap<String, u32>,
    ) -> Vec<(String, u32)> {
        let mut res = Vec::new();
        let mut buf = Vec::new();
        let uncalced_material = material.iter_mut().filter(|(_, _, calced)| !calced);

        for (name, num, calced) in uncalced_material {
            *calced = true;
            if self.is_base_material(name) {
                continue;
            }
            let material_list = &self.material_list(name).unwrap();
            for i in material_list {
                if self.is_base_material(i.name()) {
                    buf.push((i.name().to_string(), i.count() * *num, false));
                    continue;
                }
                let recipe = self.craft_table.get(i.name()).unwrap();
                match recipe.count {
                    Some(count) => {
                        let cnt = f32::ceil((i.count() * *num) as f32 / count as f32) as u32;
                        let rest = cnt * count - i.count() * *num;
                        match surplus.get_mut(i.name()) {
                            Some(c) => *c += rest,
                            None => {
                                surplus.insert(i.name().to_string(), rest);
                            }
                        }

                        buf.push((i.name().to_string(), cnt, false));
                    }
                    None => buf.push((i.name().to_string(), i.count() * *num, false)),
                }
            }
        }

        material.append(&mut buf);
        if material.iter().any(|(_, _, calced)| !calced) {
            self._calc_material_inner(material, surplus);
        }

        for (name, count, _) in material.iter() {
            res.iter_mut()
                .filter(|(n, _)| *n == *name)
                .for_each(|(_, num)| *num += count);
            if res.iter().all(|(n, _)| *n != *name) {
                res.push((name.to_string(), *count));
            }
        }

        println!("surplus: {:?}", surplus);

        res
    }

    fn calc_material(&self, mut material: Vec<(String, u32, bool)>) -> Vec<(String, u32)> {
        let mut surplus = BTreeMap::new();
        self._calc_material_inner(&mut material, &mut surplus)
    }

    fn print_base_material(&self, material_table: &Vec<(String, u32)>) -> Result<String> {
        let mut res = String::new();
        writeln!(&mut res, "= [基础材料列表]")?;
        for (name, num) in material_table {
            if self.is_base_material(name) {
                writeln!(&mut res, "[ ] {}:{}", name, num)?;
            }
        }
        Ok(res)
    }

    fn print_single_material(&self, name: &str, num: u32) -> Result<String> {
        let mut res = String::new();
        let material_entry = self.craft_table.get(name).unwrap();
        let material_list = &material_entry.material_list;
        let machine = &material_entry.machine;
        let count = self.craft_table.get(name).and_then(|x| x.count);
        match count {
            Some(count) => writeln!(
                &mut res,
                "== [{}] 数量[{}] 通过 [{}]",
                name,
                count * num,
                machine
            )?,
            None => writeln!(&mut res, "== [{}] 数量[{}] 通过 [{}]", name, num, machine)?,
        }
        for entry in material_list {
            writeln!(&mut res, "[ ] {}:{}", entry.name(), entry.count() * num)?
        }

        Ok(res)
    }

    fn print_material(&self, name: &str) -> Result<String> {
        let mut res = String::new();

        let material_table = vec![(name.to_string(), 1, false)];
        let material_table = self.calc_material(material_table);

        writeln!(&mut res, "{}", self.print_base_material(&material_table)?)?;

        for (name, num) in material_table.iter().rev() {
            if self.is_base_material(name) {
                continue;
            }

            writeln!(&mut res, "{}", self.print_single_material(name, *num)?)?
        }
        Ok(res)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut file = File::open(args.file)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;

    let table: RecipeTable = from_str(&buf)?;
    let query = args.query;

    if table.craft_table.iter().all(|(name, _)| *name != query) {
        return Err(anyhow::anyhow!("can't find {}", query));
    }

    let mut res: String = String::new();

    match args.purpose {
        Purpose::Recipe => {
            write!(&mut res, "{}", &table.print_material(&query)?)?;
        }
        Purpose::Use => todo!(),
    }

    match args.output {
        Some(output) => {
            fs::write(output, res)?;
        }
        None => println!("{res}"),
    }

    Ok(())
}

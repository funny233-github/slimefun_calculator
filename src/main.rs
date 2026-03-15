use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;

/// CLI command purpose
#[derive(Debug, Subcommand)]
enum Purpose {
    /// Calculate recipe requirements
    Recipe,
    /// Calculate material usage (not implemented)
    Use,
}

/// Command line arguments for the SlimeFun recipe calculator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the recipe table JSON file
    file: String,

    /// Name of the item to calculate
    query: String,

    #[command(subcommand)]
    purpose: Purpose,

    /// Optional output file path
    output: Option<String>,
}

/// Material entry in a recipe, supporting both single materials and materials with counts
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum MaterialEntry {
    /// Material with explicit count
    WithCount((String, u32)),
    /// Single material (implies count = 1)
    Single((String,)),
}

impl MaterialEntry {
    /// Get the material name
    fn name(&self) -> &str {
        match self {
            MaterialEntry::WithCount(tuple) => &tuple.0,
            MaterialEntry::Single(tuple) => &tuple.0,
        }
    }

    /// Get the material count (defaults to 1 for Single)
    fn count(&self) -> u32 {
        match self {
            MaterialEntry::WithCount(tuple) => tuple.1,
            MaterialEntry::Single(_) => 1,
        }
    }
}

/// A crafting recipe
#[derive(Debug, Deserialize, Serialize, Clone)]
struct Recipe {
    /// The machine required for crafting
    machine: String,
    /// List of materials needed
    #[serde(rename = "materialList")]
    material_list: Vec<MaterialEntry>,
    /// Optional batch production count
    count: Option<u32>,
}

/// Complete recipe table for all items
#[derive(Debug, Deserialize, Serialize, Clone)]
struct RecipeTable {
    /// Mapping of item names to their recipes
    #[serde(rename = "craftTable")]
    craft_table: BTreeMap<String, Recipe>,
}

impl RecipeTable {
    /// Check if a material is a base material (not craftable)
    fn is_base_material(&self, name: &str) -> bool {
        !self.craft_table.contains_key(name)
    }

    /// Get material list for an item
    fn material_list(&self, name: &str) -> Option<Vec<MaterialEntry>> {
        self.craft_table
            .get(name)
            .map(|recipe| recipe.material_list.clone())
    }

    /// Calculate required materials with surplus tracking
    fn calc_material(&self, initial_materials: Vec<(String, u32, bool)>) -> Vec<(String, u32)> {
        let mut materials = initial_materials;
        let mut surplus = BTreeMap::new();

        self.calc_material_inner(&mut materials, &mut surplus);

        // Aggregate materials while preserving insertion order (closer to target = earlier)
        let mut aggregated: IndexMap<String, u32> = IndexMap::new();
        for (name, count, _) in &materials {
            *aggregated.entry(name.clone()).or_insert(0) += count;
        }

        aggregated.into_iter().collect()
    }

    /// Inner recursive calculation with surplus tracking
    fn calc_material_inner(
        &self,
        materials: &mut Vec<(String, u32, bool)>,
        surplus: &mut BTreeMap<String, u32>,
    ) {
        let mut new_materials = Vec::new();

        // Process all uncalculated materials
        for (name, num, calced) in materials.iter_mut() {
            if *calced || self.is_base_material(name) {
                *calced = true;
                continue;
            }

            *calced = true;

            if let Some(recipe) = self.material_list(name) {
                for entry in &recipe {
                    self.process_material_entry(entry, *num, &mut new_materials, surplus);
                }
            }
        }

        // Add new materials and continue if needed
        materials.append(&mut new_materials);

        if materials.iter().any(|(_, _, calced)| !calced) {
            self.calc_material_inner(materials, surplus);
        }
    }

    /// Process a single material entry and add to the queue
    fn process_material_entry(
        &self,
        entry: &MaterialEntry,
        multiplier: u32,
        new_materials: &mut Vec<(String, u32, bool)>,
        surplus: &mut BTreeMap<String, u32>,
    ) {
        let name = entry.name().to_string();
        let mut needed = entry.count() * multiplier;

        if self.is_base_material(&name) {
            new_materials.push((name, needed, false));
            return;
        }

        // First try to use materials from surplus inventory
        if let Some(available) = surplus.get(&name).copied() {
            if available >= needed {
                // Surplus is sufficient, no production needed
                *surplus.get_mut(&name).unwrap() -= needed;
                return;
            } else {
                // Surplus is not enough, use all surplus and produce the rest
                needed -= available;
                surplus.remove(&name);
            }
        }

        if let Some(recipe) = self.craft_table.get(&name) {
            if let Some(batch_count) = recipe.count {
                // Calculate how many batches need to be produced
                let batches = Self::calculate_batches(needed, batch_count);
                let produced = batches * batch_count;

                // Calculate new surplus
                let surplus_amount = produced - needed;

                // Update surplus
                *surplus.entry(name.clone()).or_insert(0) += surplus_amount;

                new_materials.push((name, batches, false));
            } else {
                new_materials.push((name, needed, false));
            }
        }
    }

    /// Calculate number of batches needed to meet requirements
    fn calculate_batches(needed: u32, batch_size: u32) -> u32 {
        f32::ceil(needed as f32 / batch_size as f32) as u32
    }

    /// Format and print base materials section
    fn print_base_material(&self, material_table: &[(String, u32)]) -> Result<String> {
        let mut result = String::new();
        writeln!(result, "= [基础材料列表]")?;

        for (name, num) in material_table {
            if self.is_base_material(name) {
                writeln!(result, "[ ] {}:{}", name, num)?;
            }
        }

        Ok(result)
    }

    /// Format and print a single crafting step
    fn print_single_material(&self, name: &str, num: u32) -> Result<String> {
        let mut result = String::new();

        let recipe = self
            .craft_table
            .get(name)
            .context(format!("Recipe not found for: {}", name))?;
        let total_count = recipe.count.map_or(num, |batch| batch * num);

        writeln!(
            result,
            "== [{}] 数量[{}] 通过 [{}]",
            name, total_count, recipe.machine
        )?;

        for entry in &recipe.material_list {
            writeln!(result, "[ ] {}:{}", entry.name(), entry.count() * num)?;
        }

        Ok(result)
    }

    /// Calculate and format complete material requirements
    fn print_material(&self, name: &str) -> Result<String> {
        let initial_materials = vec![(name.to_string(), 1, false)];
        let material_table = self.calc_material(initial_materials);

        let mut result = String::new();

        // Print base materials first
        writeln!(result, "{}", self.print_base_material(&material_table)?)?;

        // Print crafting steps in reverse order (topological sort)
        for (name, num) in material_table.iter().rev() {
            if !self.is_base_material(name) {
                writeln!(result, "{}", self.print_single_material(name, *num)?)?;
            }
        }

        Ok(result)
    }

    /// Find and format all items that can be crafted using the given material
    fn print_use(&self, material_name: &str) -> Result<String> {
        let mut result = String::new();
        writeln!(result, "= {}可以用于", material_name)?;

        // Find all recipes that include this material
        for (item_name, recipe) in &self.craft_table {
            if recipe
                .material_list
                .iter()
                .any(|entry| entry.name() == material_name)
            {
                writeln!(result, "- {}", item_name)?;
            }
        }

        Ok(result)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read and parse recipe table
    let recipe_content =
        fs::read_to_string(&args.file).context(format!("Failed to read file: {}", args.file))?;
    let table: RecipeTable =
        from_str(&recipe_content).context("Failed to parse recipe table JSON")?;

    // Validate query exists in recipes
    if !table.craft_table.contains_key(&args.query) {
        return Err(anyhow::anyhow!("Item not found in recipes: {}", args.query));
    }

    // Calculate based on purpose
    let result = match args.purpose {
        Purpose::Recipe => table.print_material(&args.query)?,
        Purpose::Use => table.print_use(&args.query)?,
    };

    // Output result
    match args.output {
        Some(output_path) => {
            fs::write(&output_path, result)
                .context(format!("Failed to write to file: {}", output_path))?;
        }
        None => println!("{}", result),
    }

    Ok(())
}

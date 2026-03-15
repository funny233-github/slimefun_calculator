# CODEBUDDY.md

This file provides guidance to CodeBuddy Code when working with code in this repository.

## Project Overview

This is a Rust command-line calculator for SlimeFun (Minecraft mod) crafting recipes. It recursively calculates all base materials needed to craft a specific item, handling batch production and surplus material tracking.

## Development Commands

- **Build**: `cargo build`
- **Run**: `cargo run -- <FILE> <QUERY> recipe [OUTPUT]`
  - `<FILE>`: Path to table.json (recipe database)
  - `<QUERY>`: Item name to calculate (e.g., "金光闪闪的背包")
  - `[OUTPUT]`: Optional output file path
  - `recipe`: Calculate recipe materials (implemented)
  - `use`: Calculate item usage (not implemented yet)
- **Lint**: `cargo clippy`
- **Format**: `cargo fmt`
- **Test**: `cargo test` (no tests currently exist in the project)

## Architecture

### Single-File Structure
The entire implementation is in `src/main.rs` (~8.6KB). Key components:

#### Data Structures
- **`RecipeTable`**: Main container for all crafting recipes, loaded from `table.json`
  - Uses `BTreeMap<String, Recipe>` for ordered iteration
- **`Recipe`**: Individual crafting recipe with:
  - `machine`: Machine type (e.g., "高级工作台", "冶炼炉")
  - `material_list`: Vector of `MaterialEntry`
  - `count`: Optional batch size (how many items this recipe produces)
- **`MaterialEntry`**: Enum for materials using `#[serde(untagged)]`:
  - `WithCount((String, u32))`: Material with explicit quantity
  - `Single((String,))`: Material with implicit quantity of 1

#### Core Algorithms
- **`is_base_material(&self, name: &str) -> bool`**: Checks if a material has no recipe (is a base item)
- **`get_base_material_nums(&self, name: &str, num: u32) -> BTreeMap<String, u32>`**: Recursively calculates total base materials needed (currently unused/dead code)
- **`calc_material(&self, material: Vec<(String, u32, bool)>) -> Vec<(String, u32)>`**: Main calculation algorithm:
  - Takes materials with `(name, quantity, calculated)` tuples
  - Recursively resolves recipes until all base materials are found
  - Tracks surplus materials when batch production creates extras
  - Returns consolidated base material counts
- **`_calc_material_inner`**: Internal recursive function that handles:
  - Calculating how many recipe runs are needed based on `count` field
  - Tracking surplus when `count * num < required_quantity`
  - Using ceiling division to determine batch counts

### Data Format (table.json)

The recipe database (`table.json`) is a large JSON file (6520 lines) with this structure:

```json
{
  "craftTable": {
    "Item Name": {
      "machine": "Machine Type",
      "materialList": [
        ["Material Name", quantity],
        ["Another Material"]  // quantity defaults to 1
      ],
      "count": optional_batch_size  // how many items this recipe produces
    }
  }
}
```

### Output Format

The calculator outputs in a specific Chinese format matching `test.md`:

```
= [基础材料列表]
[ ] Material Name:quantity
[ ] Another Material:quantity

== [Intermediate Item] 数量[quantity] 通过 [Machine Type]
[ ] Required Material:required_quantity
[ ] Another Required Material:required_quantity
```

Base materials are listed first, followed by intermediate recipes in reverse dependency order.

## Key Implementation Details

### Surplus Material Tracking
When a recipe produces multiple items (`count` field is set), the calculator:
1. Calculates how many recipe runs are needed: `ceil(required / count)`
2. Tracks surplus: `surplus += (runs * count) - required`
3. This surplus can theoretically be used in subsequent calculations (currently not fully implemented)

### Serialization
- Uses `serde` and `serde_json` for JSON parsing
- `serde_with` for additional serialization helpers
- Chinese field names in JSON: `craftTable`, `materialList`

### Error Handling
- Uses `anyhow` for ergonomic error handling
- Returns `Result<()>` from main with descriptive errors

### CLI Framework
- Uses `clap` 4.6.0 with derive macros
- Two subcommands: `recipe` and `use`
- `use` command is currently unimplemented (returns `todo!()`)

## Current State

- **Implemented**: Full recipe calculation with surplus tracking
- **Unimplemented**: `use` command (inverse calculation - what can you make with given materials?)
- **Tests**: No test suite exists
- **Dead Code**: `get_base_material_nums` method is defined but never used
- **Debug Output**: `surplus` map is printed during calculation (debug statement in src/main.rs:188)

## Important Notes

- All item names and output are in Chinese
- Material names must exactly match entries in `table.json`
- The algorithm uses ceiling division for batch calculations: `f32::ceil((i.count() * *num) as f32 / count as f32) as u32`
- Output materials are sorted using `BTreeMap` for consistent ordering

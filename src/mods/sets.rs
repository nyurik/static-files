/*!
Support for module based generations. Use it for large data sets (more than 128 Mb).
 */
use std::{
    fs::{self, File, Metadata},
    io::{self, Write},
    path::{Path, PathBuf},
};

use super::resource::{
    collect_resources, generate_function_end, generate_function_header, generate_resource_insert,
    generate_uses, generate_variable_header, generate_variable_return, DEFAULT_VARIABLE_NAME,
};

/// Defines the split strategie.
pub trait SetSplitStrategie {
    /// Register next file from resources.
    fn register(&mut self, path: &Path, metadata: &Metadata);
    /// Determine, should we split modules now.
    fn should_split(&self) -> bool;
    /// Resets internal counters after split.
    fn reset(&mut self);
}

/// Split modules by files count.
pub struct SplitByCount {
    current: usize,
    max: usize,
}

impl SplitByCount {
    #[must_use]
    pub fn new(max: usize) -> Self {
        Self { current: 0, max }
    }
}

impl SetSplitStrategie for SplitByCount {
    fn register(&mut self, _path: &Path, _metadata: &Metadata) {
        self.current += 1;
    }

    fn should_split(&self) -> bool {
        self.current >= self.max
    }

    fn reset(&mut self) {
        self.current = 0;
    }
}

/// Generate resources for `project_dir` using `filter`
/// breaking them into separate modules using `set_split_strategy` (recommended for large > 128 Mb setups).
///
/// Result saved in module named `module_name`. It exports
/// only one function named `fn_name`. It is then exported from
/// `generated_filename`. `generated_filename` is also used to determine
/// the parent directory for the module.
///
/// in `build.rs`:
/// ```rust
///
/// use std::{env, path::Path};
/// use static_files::sets::{generate_resources_sets, SplitByCount};
///
/// fn main() {
///     let out_dir = env::var("OUT_DIR").unwrap();
///     let generated_filename = Path::new(&out_dir).join("generated_sets.rs");
///     generate_resources_sets(
///         "./tests",
///         None,
///         generated_filename,
///         "sets",
///         "generate",
///         &mut SplitByCount::new(2),
///     )
///     .unwrap();
/// }
/// ```
///
/// in `main.rs`:
/// ```rust
/// include!(concat!(env!("OUT_DIR"), "/generated_sets.rs"));
///
/// fn main() {
///     let generated_file = generate();
///
///     assert_eq!(generated_file.len(), 4);
///
/// }
/// ```
pub fn generate_resources_sets<P, G, S>(
    project_dir: P,
    filter: Option<fn(p: &Path) -> bool>,
    generated_filename: G,
    module_name: &str,
    fn_name: &str,
    set_split_strategy: &mut S,
) -> io::Result<()>
where
    P: AsRef<Path>,
    G: AsRef<Path>,
    S: SetSplitStrategie,
{
    let resources = collect_resources(&project_dir, filter)?;

    let mut generated_file = File::create(&generated_filename)?;

    let module_dir = generated_filename.as_ref().parent().map_or_else(
        || PathBuf::from(module_name),
        |parent| parent.join(module_name),
    );
    fs::create_dir_all(&module_dir)?;

    let mut module_file = File::create(module_dir.join("mod.rs"))?;

    generate_uses(&mut module_file)?;
    writeln!(
        module_file,
        "\
use ::std::collections::HashMap;
use ::static_files::Resource;"
    )?;

    let mut modules_count = 1;

    let mut set_file = create_set_module_file(&module_dir, modules_count)?;
    let mut should_split = set_split_strategy.should_split();

    for resource in &resources {
        let (path, metadata) = &resource;
        if should_split {
            set_split_strategy.reset();
            modules_count += 1;
            generate_function_end(&mut set_file)?;
            set_file = create_set_module_file(&module_dir, modules_count)?;
        }
        set_split_strategy.register(path, metadata);
        should_split = set_split_strategy.should_split();

        generate_resource_insert(&mut set_file, &project_dir, DEFAULT_VARIABLE_NAME, resource)?;
    }

    generate_function_end(&mut set_file)?;

    for module_index in 1..=modules_count {
        writeln!(module_file, "mod set_{module_index};")?;
    }

    generate_function_header(&mut module_file, fn_name)?;

    generate_variable_header(&mut module_file, DEFAULT_VARIABLE_NAME)?;

    for module_index in 1..=modules_count {
        writeln!(
            module_file,
            "set_{module_index}::generate(&mut {DEFAULT_VARIABLE_NAME});",
        )?;
    }

    generate_variable_return(&mut module_file, DEFAULT_VARIABLE_NAME)?;

    generate_function_end(&mut module_file)?;

    writeln!(
        generated_file,
        "\
mod {module_name};
pub use {module_name}::{fn_name};",
    )?;

    Ok(())
}

fn create_set_module_file(module_dir: &Path, module_index: usize) -> io::Result<File> {
    let mut set_module = File::create(module_dir.join(format!("set_{module_index}.rs")))?;

    writeln!(
        set_module,
        "\
#[allow(clippy::wildcard_imports)]
use super::*;
#[allow(clippy::unreadable_literal)]
pub(crate) fn generate({DEFAULT_VARIABLE_NAME}: &mut HashMap<&'static str, Resource>) {{",
    )?;

    Ok(set_module)
}

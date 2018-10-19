use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::SystemTime;
use wabt::script::{Action, Command, CommandKind, ModuleBinary, ScriptParser, Value};
use wabt::wasm2wat;

static BANNER: &str = "// Rust test file autogenerated with cargo build (src/build_spectests.rs).
// Please do NOT modify it by hand, as it will be reseted on next build.\n";

const TESTS: [&str; 6] = [
    "spectests/br_if.wast",
    "spectests/call.wast",
    "spectests/i32_.wast",
    "spectests/memory.wast",
    "spectests/set_local.wast",
    "spectests/types.wast",
];

struct WastTestGenerator {
    last_module: i32,
    last_line: u64,
    filename: String,
    script_parser: ScriptParser,
    buffer: String,
}

fn wabt2rust_type(v: &Value) -> String {
    match v {
        Value::I32(v) => format!("i32"),
        Value::I64(v) => format!("i64"),
        Value::F32(v) => format!("f32"),
        Value::F64(v) => format!("f64"),
    }
}

fn wabt2rust_value(v: &Value) -> String {
    match v {
        Value::I32(v) => format!("{:?} as i32", v),
        Value::I64(v) => format!("{:?} as i64", v),
        Value::F32(v) => format!("{:?} as f32", v),
        Value::F64(v) => format!("{:?} as f64", v),
    }
}

impl WastTestGenerator {
    fn new(path: &PathBuf) -> Self {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let source = fs::read(&path).unwrap();
        let mut script: ScriptParser =
            ScriptParser::from_source_and_name(&source, filename).unwrap();
        let mut buffer = String::new();
        WastTestGenerator {
            last_module: 0,
            last_line: 0,
            filename: filename.to_string(),
            script_parser: script,
            buffer: buffer,
        }
    }

    fn consume(&mut self) {
        self.buffer.push_str(BANNER);
        self.buffer.push_str(&format!(
            "// Test based on spectests/{}

use crate::webassembly::{{instantiate, compile, ImportObject, ResultObject, VmCtx, Export}};
use wabt::wat2wasm;\n\n",
            self.filename
        ));
        while let Some(Command { line, kind }) = &self.script_parser.next().unwrap() {
            self.last_line = line.clone();
            self.buffer
                .push_str(&format!("\n// Line {}\n", self.last_line));
            self.visit_command(&kind);
            // &self.buffer.push_str(convert_command(kind));
        }
    }

    fn visit_module(&mut self, module: &ModuleBinary, name: &Option<String>) {
        let wasm_binary: Vec<u8> = module.clone().into_vec();
        let wast_string = wasm2wat(wasm_binary).expect("Can't convert back to wasm");
        self.last_module = self.last_module + 1;
        self.buffer.push_str(
            format!(
                "fn create_module_{}() -> ResultObject {{
    let module_str = \"{}\";
    let wasm_binary = wat2wasm(module_str.as_bytes()).expect(\"WAST not valid or malformed\");
    instantiate(wasm_binary, ImportObject::new()).expect(\"WASM can't be instantiated\")
}}\n",
                self.last_module,
                // We do this to ident four spaces back
                wast_string.replace("\n", "\n    ").replace("\"", "\\\""),
            )
            .as_str(),
        );
    }

    fn visit_assert_invalid(&mut self, module: &ModuleBinary) {
        let wasm_binary: Vec<u8> = module.clone().into_vec();
        // let wast_string = wasm2wat(wasm_binary).expect("Can't convert back to wasm");
        self.buffer.push_str(
            format!(
                "
#[test]
fn l{}_assert_invalid() {{
    let wasm_binary = {:?};
    let compilation = compile(wasm_binary.to_vec());
    assert!(compilation.is_err(), \"WASM should not compile as is invalid\");
}}\n",
                self.last_line,
                wasm_binary,
                // We do this to ident four spaces back
                // String::from_utf8_lossy(&wasm_binary),
                // wast_string.replace("\n", "\n    "),
            )
            .as_str(),
        );
    }
    fn visit_assert_malformed(&mut self, module: &ModuleBinary) {
        let wasm_binary: Vec<u8> = module.clone().into_vec();
        // let wast_string = wasm2wat(wasm_binary).expect("Can't convert back to wasm");
        self.buffer.push_str(
            format!(
                "
#[test]
fn l{}_assert_malformed() {{
    let wasm_binary = {:?};
    let compilation = compile(wasm_binary.to_vec());
    assert!(compilation.is_err(), \"WASM should not compile as is malformed\");
}}\n",
                self.last_line,
                wasm_binary,
                // We do this to ident four spaces back
                // String::from_utf8_lossy(&wasm_binary),
                // wast_string.replace("\n", "\n    "),
            )
            .as_str(),
        );
    }

    fn visit_assert_return(&mut self, action: &Action, expected: &Vec<Value>) {
        match action {
            Action::Invoke {
                module,
                field,
                args,
            } => {
                let func_return = if expected.len() > 0 {
                    format!(" -> {}", wabt2rust_type(&expected[0]))
                } else {
                    "".to_string()
                };
                let expected_result = if expected.len() > 0 {
                    wabt2rust_value(&expected[0])
                } else {
                    "()".to_string()
                };
                // We map the arguments provided into the raw Arguments provided
                // to libffi
                let mut args_types: Vec<String> = args.iter().map(wabt2rust_type).collect();
                args_types.push("&VmCtx".to_string());
                let mut args_values: Vec<String> = args.iter().map(wabt2rust_value).collect();
                args_values.push("&vm_context".to_string());
                self.buffer.push_str(
                    format!(
                        "#[test]
fn l{}_assert_return_invoke() {{
    let ResultObject {{ mut instance, module }} = create_module_{}();
    let func_index = match module.info.exports.get({:?}) {{
        Some(&Export::Function(index)) => index,
        _ => panic!(\"Function not found\"),
    }};
    let invoke_fn: fn({}){} = get_instance_function!(instance, func_index);
    let vm_context = instance.generate_context();
    let result = invoke_fn({});
    assert_eq!(result, {});
}}\n",
                        self.last_line,
                        self.last_module,
                        field,
                        args_types.join(", "),
                        func_return,
                        args_values.join(", "),
                        expected_result,
                    )
                    .as_str(),
                );
            }
            _ => {}
        };
    }

    fn visit_command(&mut self, cmd: &CommandKind) {
        match cmd {
            CommandKind::Module { module, name } => {
                self.visit_module(module, name);
                // c.module(module.into_vec(), name);
            }
            CommandKind::AssertReturn { action, expected } => {
                self.visit_assert_return(action, expected);
            }
            CommandKind::AssertReturnCanonicalNan { action } => {
                // c.assert_return_canonical_nan(action);
            }
            CommandKind::AssertReturnArithmeticNan { action } => {
                // c.assert_return_arithmetic_nan(action);
            }
            CommandKind::AssertTrap { action, message: _ } => {
                // c.assert_trap(action);
            }
            CommandKind::AssertInvalid { module, message: _ } => {
                self.visit_assert_invalid(module);
            }
            CommandKind::AssertMalformed { module, message: _ } => {
                self.visit_assert_malformed(module);
                // c.assert_malformed(module.into_vec());
            }
            CommandKind::AssertUninstantiable { module, message: _ } => {
                // c.assert_uninstantiable(module.into_vec());
            }
            CommandKind::AssertExhaustion { action } => {
                // c.assert_exhaustion(action);
            }
            CommandKind::AssertUnlinkable { module, message: _ } => {
                // c.assert_unlinkable(module.into_vec());
            }
            CommandKind::Register { name, as_name } => {
                // c.register(name, as_name);
            }
            CommandKind::PerformAction(action) => {
                // c.action(action);
            }
        }
    }
    fn finalize(self) -> String {
        self.buffer
    }
}

fn wast_to_rust(wast_filepath: &str) -> String {
    let wast_filepath = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), wast_filepath);
    let path = PathBuf::from(&wast_filepath);
    let script_name: String = String::from(path.file_stem().unwrap().to_str().unwrap());
    let rust_test_filepath = format!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/spectests/{}.rs"),
        script_name.clone().as_str()
    );

    let wast_modified = fs::metadata(&wast_filepath)
        .expect("Can't get wast file metadata")
        .modified()
        .expect("Can't get wast file modified date");
    let should_modify = match fs::metadata(&rust_test_filepath) {
        Ok(m) => {
            m.modified()
                .expect("Can't get rust test file modified date")
                < wast_modified
        }
        Err(_) => true,
    };

    // panic!("SOULD MODIFY {:?} {:?}", should_modify, rust_test_filepath);

    if should_modify {
        let mut generator = WastTestGenerator::new(&path);
        generator.consume();
        let generated_script = generator.finalize();
        fs::write(&rust_test_filepath, generated_script.as_bytes()).unwrap();
    }
    script_name
}

fn main() {
    let rust_test_modpath = concat!(env!("CARGO_MANIFEST_DIR"), "/src/spectests/mod.rs");

    let mut modules: Vec<String> = Vec::new();
    modules.reserve_exact(TESTS.len());

    for test in TESTS.iter() {
        let module_name = wast_to_rust(test);
        modules.push(module_name);
    }

    let mut modfile_uses: Vec<String> = modules
        .iter()
        .map(|module| format!("mod {};", module))
        .collect();

    modfile_uses.insert(0, BANNER.to_string());
    // We add an empty line
    modfile_uses.push("".to_string());

    let modfile: String = modfile_uses.join("\n");
    let source = fs::read(&rust_test_modpath).unwrap();
    // We only modify the mod file if has changed
    if source != modfile.as_bytes() {
        fs::write(&rust_test_modpath, modfile.as_bytes()).unwrap();
    }
    // panic!(modfile);
}

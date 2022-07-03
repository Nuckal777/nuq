use std::{
    io::{Read, Write},
    process::{Command, ExitStatus, Stdio},
};

const BINARY_PATH: &str = env!("CARGO_BIN_EXE_nuq");

#[test]
fn yaml_stdio_to_raw_string() {
    let (exit, output) = spawn_nuq(&["-r", "-i", "yaml", ".key"], b"key: test");
    assert!(exit.success());
    assert_eq!(output, "test\n");
}

#[test]
fn yaml_file_to_raw_string() {
    std::fs::write("./mock.yaml", "key: test").expect("failed to create mock.yaml");
    let (exit, output) = spawn_nuq(&["-r", ".key", "mock.yaml"], b"key: test");
    std::fs::remove_file("./mock.yaml").expect("failed to remove .yaml");
    assert!(exit.success());
    assert_eq!(output, "test\n");
}

fn spawn_nuq(args: &[&str], input: &[u8]) -> (ExitStatus, String) {
    let mut handle = Command::new(BINARY_PATH)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to launch nicator process.");
    let mut stdin = handle.stdin.take().unwrap();
    stdin
        .write_all(input)
        .expect("Failed to write to nicator stdin.");
    drop(stdin);
    let mut stdout = handle.stdout.take().unwrap();
    let mut output = Vec::<u8>::new();
    stdout
        .read_to_end(&mut output)
        .expect("Failed to read nicator output.");
    let output = String::from_utf8(output).expect("Output is invalid utf8.");
    (
        handle.wait().expect("Failed to await nicator process."),
        output,
    )
}

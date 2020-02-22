mod frontend;

fn main() {
    let file = std::env::args().nth(1).unwrap();
    let ast = frontend::process(&file).unwrap();
    println!("{}", serde_json::to_string_pretty(&ast).unwrap());
}

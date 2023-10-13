use concord_v2_parser::parser::{parse_stream, Input};

extern crate concord_v2_parser;

fn main() {
    let src = r#"
    flows:
      default:
        - log: "Hello, ${name}"
    "#;

    let mut input = Input::try_from(src).unwrap();
    for doc in parse_stream(&mut input).unwrap() {
        println!("{doc:?}");
    }
}

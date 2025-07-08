use concord_v2_parser::parser::{parse_stream, Input};

#[test]
fn complex() {
    let mut input = Input::try_from(include_str!("data/complex.concord.yaml")).unwrap();
    let result = parse_stream(&mut input).unwrap();
    dbg!(result);
}

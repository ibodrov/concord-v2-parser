use concord_v2_parser::input::Input;
use concord_v2_parser::parser::parse_stream;

#[test]
fn complex() {
    let mut input = Input::try_from(include_str!("data/complex.concord.yaml")).unwrap();
    while let Ok((ev, marker)) = input.try_next() {
        dbg!(ev, marker);
    }
    let mut input = Input::try_from(include_str!("data/complex.concord.yaml")).unwrap();
    let result = parse_stream(&mut input).unwrap();
    dbg!(result);
}

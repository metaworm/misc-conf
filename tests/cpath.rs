use misc_conf::cpath::*;

#[test]
fn test() {
    let cp = CPathBuf::parse("//abc[def~1234]/def").unwrap();
    println!("{cp:?}");
}

#[test]
fn test2() {
    let cp = CPathBuf::parse("//'abc def'").unwrap();
    println!("{cp:?}");
}

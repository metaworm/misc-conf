use std::path::Path;

use misc_conf::apache::Apache;
use misc_conf::ast::Config;

fn parse(path: impl AsRef<Path>) -> Config<Apache> {
    let path = path.as_ref();
    println!("parsing: {:?}", path);
    let cfg = Config::<Apache>::parse(path.to_path_buf()).unwrap();
    if path.ends_with("test.conf") {
        println!("{cfg:?}");
    }
    cfg
}

#[test]
fn parse_all() {
    for path in glob::glob("tests/apache/**/*.conf").unwrap().flatten() {
        let _res = parse(&path);
    }
}

#[test]
fn verify_result() {
    let cfg = parse("tests/apache/confcase/string.conf");
    let files = cfg.root.query("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].args[0], "\\.ht*");
    assert_eq!(cfg.root.query("MultiLineString")[0].args[0], "abc\ndef");
}

#[test]
fn include() {
    let mut cfg = parse("tests/apache/confcase/include.conf");
    cfg.resolve_include(None, None).unwrap();

    println!("{:#?}", cfg.root);
    assert_eq!(cfg.root_directives()[0].name, "DefaultRuntimeDir");
}

#[test]
fn string() {
    let cfg = parse("tests/apache/confcase/string.conf");
    println!("{:#?}", cfg.root);

    let res = cfg.root.query("Files");
    assert!(res[0].args[0] == "\\.ht*");
}

#[test]
fn cpath() {
    use misc_conf::cpath::*;

    let cfg = parse("tests/apache/extra/httpd-vhosts.conf");
    // println!("{:#?}", cfg.root);

    let res = cfg
        .root
        .cpath_query(&CPathBuf::parse("//ServerAdmin").unwrap());
    println!("{res:#?}");
}

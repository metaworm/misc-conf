use std::path::Path;

use misc_conf::ast::Config;
use misc_conf::nginx::Nginx;

fn parse(path: impl AsRef<Path>) -> Config<Nginx> {
    let path = path.as_ref();
    println!("parsing: {:?}", path);
    let cfg = Config::<Nginx>::parse(path.to_path_buf()).unwrap();
    if path.ends_with("test.conf") {
        println!("{cfg:?}");
    }
    cfg
}

#[test]
fn parse_all() {
    for path in glob::glob("tests/nginx/**/*.conf").unwrap().flatten() {
        let _res = parse(&path);
    }
}

#[test]
fn verify() {
    let conf = parse("tests/nginx/few_locations.conf");
    let res = conf.root.query("http/server/location/add_header");
    println!("{res:#?}");
}

#[test]
fn include() {
    let mut conf = parse("tests/nginx/include.conf");
    conf.resolve_include(None);
    let conf2 = parse("tests/nginx/index.conf");
    assert_eq!(conf.root, conf2.root);
}

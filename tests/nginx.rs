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
    conf.resolve_include(None, None).unwrap();
    let conf2 = parse("tests/nginx/index.conf");
    assert_eq!(conf.root, conf2.root);
}

#[test]
fn lua() {
    let conf = parse("tests/nginx/lua.conf");
    let d = conf.root.query("http/lua_shared_dict").pop().unwrap();
    assert_eq!(d.name, "lua_shared_dict");
    assert_eq!(d.args[0], "ocsp_response_cache");
    assert_eq!(d.args[1], "5M");
}

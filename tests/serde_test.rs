use mlua::{Lua, LuaSerdeExt, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Menu {
    path: Vec<String>,
}

#[test]
fn test_serde_repeated_string() {
    let lua = Lua::new();
    let menu = Menu {
        path: vec!["One".to_string(), "Two".to_string()],
    };
    lua.globals()
        .set("menu", lua.to_value(&menu).unwrap())
        .unwrap();
    lua.load(
        r#"
            assert(menu.path[1] == "One")
            assert(menu.path[2] == "Two")
        "#,
    )
    .exec()
    .unwrap();

    let val = lua.globals().get::<_, Value>("menu").unwrap();
    assert_eq!(lua.from_value::<Menu>(val).unwrap(), menu);
}

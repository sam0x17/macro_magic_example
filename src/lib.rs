use macros_crate::*;

make_item_const!(foreign_crate::StructTwo);

#[test]
fn test_make_item_const() {
    assert_eq!(ITEM_SRC, "struct MyStruct { field1 : bool, }");
}

mod first_mod {
    #[macro_magic::export_tokens(StructOne)]
    struct MyStruct {
        field1: usize,
    }
}

mod second_mod {
    #[macro_magic::export_tokens(StructTwo)]
    struct MyStruct {
        field1: bool,
    }
}

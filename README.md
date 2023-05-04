## End-to-end Example 

Let's say we have three crates, our proc macro crate, called *`macros_crate`*, a crate that has items we are interested in importing, called *`foreign_crate`*, and the crate where we want to use these macros to import items from the `foreign_crate`, called the *`user_crate`*.

The `foreign_crate` just needs `macro_magic` as a dependency without any features enabled and does not need to rely on any other crates.

The `macros_crate` just needs `macro_magic` as a dependency with the `proc_support` feature enabled and does not need to rely on any other crates (though it will probably reference `syn` directly, etc).

The `user_crate` needs `macros_crate` (or a crate that re-exports `macros_crate`) as a dependency, and the `foreign_crate` as a dependency so it can reference the item(s) we are interested in by path. It also needs `macro_magic` (with no extra features) as a dependency, but this is not required if you use a slightly more complicated setup with a re-export crate, which I briefly go over further down.

### Understanding `#[export_tokens]`

Within `foreign_crate`, let's suppose there are two sub-modules that each contain a struct called `MyStruct`, and we want to import tokens for both of these `MyStruct` structs (I am doing it this way so the names collide intentionally). Also note that nothing is public (I want to demonstrate that `macro_magic` doesn't care about visibility and can work around it without you having to change the visibility of the items you are exporting):

```rust
// foreign_crate/src/lib.rs
mod first_mod {
    #[macro_magic::export_tokens]
    struct MyStruct {
        field1: usize,
    }
}

mod second_mod {
    #[macro_magic::export_tokens]
    struct MyStruct {
        field1: bool,
    }
}
```

If we try to attach `#[export_tokens]` to _both_ of the `MyStruct`s like above, we will get an error like the following:

```
error[E0428]: the name `__export_tokens_tt_my_struct` is defined multiple times
 --> foreign_crate/src/lib.rs:9:5
  |
2 |     #[macro_magic::export_tokens]
  |     ----------------------------- previous definition of the macro `__export_tokens_tt_my_struct` here
...
9 |     #[macro_magic::export_tokens]
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `__export_tokens_tt_my_struct` redefined here
```

The reason reveals a little bit about how `macro_magic` work, which is why I picked this example. From the error we can see that it is complaining that `__export_tokens_tt_my_struct` is defiend twice. Let's change things around so we just do `#[export_tokens]` on the _first_ `MyStruct` and take a look at the expansion to understand why:

```rust
// foreign_crate/src/lib.rs
mod first_mod {
    #[macro_magic::export_tokens]
    struct MyStruct {
        field1: usize,
    }
}

mod second_mod {
    // #[macro_magic::export_tokens]
    struct MyStruct {
        field1: bool,
    }
}
```
When we run cargo expand, we just get this, which is a bit confusing as we don't see our mysterious `__export_tokens_tt_my_struct` item at all:
```
cd foreign_crate
cargo expand
```
```rust
#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
mod first_mod {
    #[allow(unused)]
    struct MyStruct {
        field1: usize,
    }
}
mod second_mod {
    struct MyStruct {
        field1: bool,
    }
}
```
It turns out by default, `cargo expand` removes a lot of things like unused macros from the printed expansion, so running with `--ugly` will fix this so we can finally solve our mystery:
```
cargo expand --ugly
```
yields:
```rust
mod first_mod {
    #[macro_export]
    macro_rules! __export_tokens_tt_my_struct {
        ($(::)? $($tokens_var : ident)::*, $(::)? $($callback : ident)::*, $extra : expr) =>
        {
            $($callback)::*!
            {
                $($tokens_var)::*, struct MyStruct { field1 : usize, },
                $extra
            }
        };
        ($(::)? $($tokens_var : ident)::*, $(::)? $($callback : ident)::*) =>
        {
            $($callback)::*!
            { $($tokens_var)::*, struct MyStruct { field1 : usize, } }
        };
    }
    #[allow(unused)]
    struct MyStruct {
        field1: usize,
    }
}

mod second_mod {
    struct MyStruct {
        field1: bool,
    }
}
```

In this expansion we can see the meat of how `macro_magic` actually works. The `#[export_tokens]` macro expands to a decl macro named `__export_tokens_tt_[name]` where `name` is the `snake_case` version of the ident of the item you are exporting. You will notice if you attach `#[export_tokens]` to an item that _doesn't_ have a well-defined ident that is the "name" of the item, such as a `use` statement or something like that, you will get an error complaining that you need to explicitly epecify an export name. Adding an explicit export name also happens to be the solution to our little collision problem, however let's first go through what this `__export_tokens_tt_my_struct` macro is doing.

The decl macro has two arms, both of which do roughly the same thing except for the `$extra` variable, so let's zoom in on the second, simpler arm:
```rust
($(::)? $($tokens_var : ident)::*, $(::)? $($callback : ident)::*) =>
{
    $($callback)::*!
    { $($tokens_var)::*, struct MyStruct { field1 : usize, } }
};
```
This decl macro takes two comma separated arguments that both evaluate to paths (the long, complicated way). This then expands to the `$callback` being called as a macro with the first argument and the tokens for the item we are exporting (i.e. the tokens for `MyStruct`) as the second argument.

Thus `#[export_tokens]` produces a decl macro that essentially takes in a callback as input and expands to the callback being called with the tokens for the item we care about as part of the input along with some other book-keeping information.

The reason we use `: ident` and not `: path` here is a long, complicated and sad story, but the short version is decl macros have a lot of weird restrictions on when they can and cannot appear in item and expr context, and we have to fake things by using idents here because for some undocumented reason `path` arguments severely limit where you can use a decl macro (and I only know about this because a core rust dev tipped me off that this would work as a work-around). This is one scenario where `macro_magic` really saves you from dealing with a headache. Thus a simplified version of this decl macro might have looked like:

```rust
macro_rules! __export_tokens_tt_my_struct {
    ($tokens_var: path, $callback: path) => {
        $callback! { $tokens_var, #item_tokens }
    }
}
```
and this is a good mental model to use for how `#[export_tokens]` works, even if the reality is a little bit more complicated.

So to resolve our collision issue above, we can specify an export name for one (or both) of the `MyStruct` items. The only requirement is that the `snake_case` of this name must be unique in the current module and in the crate root (because of how decl macro exports work). So let's go with `StructOne` and `StructTwo` for export names. It turns out this compiles:

```rust
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

```
I leave it as an exercise for the reader to explore the cargo expansion when both `export_tokens` calls are present ;)

### Writing an `#[import_tokens_proc]` proc macro

Now let's write a proc macro in our `macros_crate` that can make use of exported items. Our intent is that this proc macro will be able to be called with `foreign_crate::first_mod::StructOne` or `foreign_crate::second_mod::StructTwo` as an argument, and internally the proc macro will receive the tokens for `StructOne` or `StructTwo` (or _any_ item that has been marked with `#[export_tokens]` for us to do something with.

To keep things simple, we will call our proc macro `print_foreign_item!(path)` and the macro will simpl `println!()` the tokens for the specified item and expand to an empty `TokenStream`.

Our `Cargo.toml` for `macros_crate` should look like this:
```toml
[package]
name = "macros_crate"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
macro_magic = { version = "0.3", features = ["proc_support"] }
```

We can write the proc macro in `macros_crate/src/lib.rs` like this:

```rust
use macro_magic::import_tokens_proc;
use proc_macro::TokenStream;

#[import_tokens_proc]
#[proc_macro]
pub fn print_foreign_item(tokens: TokenStream) -> TokenStream {
    println!("{}", tokens.to_string());
    "".parse().unwrap()
}
```

Let's look at the source code for [import_tokens_proc_internal](https://docs.rs/macro_magic/latest/macro_magic/mm_core/fn.import_tokens_proc_internal.html) to better understand how the above might expand. One confusing thing to remember is that `#[import_tokens_proc]` is an attribute macro that expands _within_ your proc macro crate, yielding a chain of proc macros that call each other to achieve the desired behavior. Normally it would be impossible to generate proc macros with proc macros, but because we are inside a proc macro crate, this peculiar ability is available to us.

I have added line comments to better label what different pieces do:
```rust
Ok(quote! {
    // whatever attributes were attached to your proc macro function definition
    #(#orig_attrs)
    *
    // defines a new proc macro with a signature that matches that of your original
    // proc macro function definition. The body is automatically replaced with some
    // custom code that is part of the expansion.
    pub #orig_sig {
        // brings syn and some other things into scope
        use #mm_path::__private::*;
        use #mm_path::__private::quote::ToTokens;
        // parses the path of the item whose tokens we want to import
        let source_path = match syn::parse::<syn::Path>(#tokens_ident) {
            Ok(path) => path,
            Err(e) => return e.to_compile_error().into(),
        };
        // this outer proc macro will expand to the following, which is a call to
        // `forward_tokens!` which is the wrapper macro `macro_magic` uses to pass
        // a `source_path` to an `#[export_tokens]`-generated decl_macro, along
        // with some book-keeping information and `#inner_macro_ident` which matches
        // the second proc macro defined below
        quote::quote! {
            #mm_override_path::forward_tokens! {
                #pound source_path,
                #inner_macro_ident,
                #mm_override_path
            }
        }.into()
    }

    // a second, hidden "innner" proc macro is created with variable names matching your
    // original proc macro and all the original statements of your original proc macro.
    // This proc macro is what the `#[export_tokens]` decl macro will call when it expands
    // after being called by `forward_tokens`
    #[doc(hidden)]
    #[proc_macro]
    pub #inner_sig {
        #(#orig_stmts)
        *
    }
})
```

Fully expanded, this looks something like the following for our `print_foreign_item` example:
```rust
#[proc_macro]
pub fn print_foreign_item(tokens: TokenStream) -> TokenStream {
    use ::macro_magic::__private::quote::ToTokens;
    use ::macro_magic::__private::*;
    let source_path = match syn::parse::<syn::Path>(tokens) {
        Ok(path) => path,
        Err(e) => return e.to_compile_error().into(),
    };
    {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::__private::push_colon2(&mut _s);
        ::quote::__private::push_ident(&mut _s, "macro_magic");
        ::quote::__private::push_colon2(&mut _s);
        ::quote::__private::push_ident(&mut _s, "forward_tokens");
        ::quote::__private::push_bang(&mut _s);
        ::quote::__private::push_group(&mut _s, ::quote::__private::Delimiter::Brace, {
            let mut _s = ::quote::__private::TokenStream::new();
            ::quote::ToTokens::to_tokens(&source_path, &mut _s);
            ::quote::__private::push_comma(&mut _s);
            ::quote::__private::push_ident(
                &mut _s,
                "__import_tokens_proc_print_foreign_item_inner",
            );
            ::quote::__private::push_comma(&mut _s);
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "macro_magic");
            _s
        });
        _s
    }
    .into()
}
#[doc(hidden)]
#[proc_macro]
pub fn __import_tokens_proc_print_foreign_item_inner(tokens: TokenStream) -> TokenStream {
    {
        ::std::io::_print(format_args!("{0}\n", tokens.to_string()));
    };
    "".parse().unwrap()
}
```

So when you call the macro via something like `print_foreign_item!(foreign_crate::first_mod::StructOne)`, the expansion of `forward_tokens!` is such that you end up with the following macro expansion order within whatever crate is making use of your `print_foreign_item!` macro:

```
print_foreign_item!(some::path)
  => foreign_crate::__export_tokens_tt_my_struct!([some args])
       => __import_tokens_proc_print_foreign_item_inner(input: [item tokens])
```

### Using our macro within `user_crate`

Now that we've written out our macro, let's try using it from within `user_crate`.

The `Cargo.toml` for `user_crate` should look like the following:
```toml
[workspace]
members = ["foreign_crate", "macros_crate"]

[package]
name = "user_crate"
version = "0.1.0"
edition = "2021"

[dependencies]
macros_crate = { path = "macros_crate" }
foreign_crate = { path = "foreign_crate" }
macro_magic = { version = "0.3" }
```

Thus users of our macro only need to be able to bring our macro into scope and have access to the crate in which the items we are interested in (`foreign_crate`) as a dependency. With this simple setup, a no-features-required dependency on `macro_magic` is also required, however this requirement can be removed if you use a re-export crate to house your proc macros, re-export `macro_magic` within this crate, and tell `macro_magic` about this re-export path, which is documented [here](https://docs.rs/macro_magic/latest/macro_magic/attr.import_tokens_proc.html) in the notes about `MACRO_MAGIC_ROOT` at the end.

The `lib.rs` for `user_crate` could look like this:

```rust
use macros_crate::*;

print_foreign_item!(foreign_crate::StructOne);
```

Compiling, we will get the following output:
```
$ cargo build
   Compiling proc-macro2 v1.0.56
   Compiling unicode-ident v1.0.8
   Compiling quote v1.0.26
   Compiling syn v1.0.109
   Compiling syn v2.0.15
   Compiling derive-syn-parse v0.1.5
   Compiling macro_magic_core_macros v0.3.3
   Compiling macro_magic_core v0.3.3
   Compiling macro_magic_macros v0.3.3
   Compiling macro_magic v0.3.3
   Compiling foreign_crate v0.1.0 (/home/sam/workspace/macro_magic_example/foreign_crate)
   Compiling macros_crate v0.1.0 (/home/sam/workspace/macro_magic_example/macros_crate)
   Compiling user_crate v0.1.0 (/home/sam/workspace/macro_magic_example)
struct MyStruct { field1 : usize, }
    Finished dev [unoptimized + debuginfo] target(s) in 2.09s
````

Notice that as `user_crate` is compiled, we get some `println!` output showing the tokens for
the first `MyStruct`. So everything is working properly.

Here is what the expansion of the above looks like from within `user_crate`:

```rust
#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use macros_crate::*;
```

This makes sense. Our `println!` runs at compile-time and otherwise _nothing_ is actually expanding from our proc macro, so we essentially have an empty module.

To fix this, let's instead modify our proc macro to be `make_item_const`, which will emit a `const &'static str` called `ITEM_SRC` containing the string source code of our item:

```rust
use macro_magic::import_tokens_proc;
use proc_macro::TokenStream;
use quote::quote;

#[import_tokens_proc]
#[proc_macro]
pub fn make_item_const(tokens: TokenStream) -> TokenStream {
    let item_str = tokens.to_string();
    quote! {
        const ITEM_SRC: &'static str = #item_str;
    }
    .into()
}
```

And then we can modify `user_crate` as follows:
```rust
use macros_crate::*;

make_item_const!(foreign_crate::StructTwo);

#[test]
fn test_make_item_const() {
    assert_eq!(ITEM_SRC, "struct MyStruct { field1 : bool, }");
}
```

Now running `cargo test` on `user_crate`, we get:

```
$ cargo test
    Finished test [unoptimized + debuginfo] target(s) in 0.01s
     Running unittests src/lib.rs (target/debug/deps/user_crate-b9ee07f6bcb086c7)

running 1 test
test test_make_item_const ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

So the test passes. The `ITEM_SRC` constant is successfully generating at compile-time to contain the current source code for whatever `#[export_tokens]` item path you provide to our `make_item_const!` macro, in this case `StructTwo`.

Now let's see what cargo expand looks like within `user_crate`:

```rust
#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use macros_crate::*;

const ITEM_SRC: &'static str = "struct MyStruct { field1 : bool, }";
```

This time we see our constant generating, since our macro actually generates some tokens in the
output module.

### Importing/Exporting

One thing you might notice is we are doing `use macros_crate::*` for our import. This is a slick trick because it brings _all_ items in `macros_crate` into scope, including our secretly exported-at-the-root decl macros generated by `#[export_tokens]`, which is why this simple example works so well without any fuss. For this reason, `macro_magic` provides `#[use_proc]` and `#[use_attr]` attribute macros that you can attach to a regular `use` statement to import or re-export `macro_magic`-generated macros. Here is how you would do it with `user_crate` without relying on a glob import:

```rust
#[macro_magic::use_proc]
use macros_crate::make_item_const;

make_item_const!(foreign_crate::StructTwo);

#[test]
fn test_make_item_const() {
    assert_eq!(ITEM_SRC, "struct MyStruct { field1 : bool, }");
}
```

It turns out the `#[use_proc]` and `#[use_attr]` macros are quite simple. Here is the expansion for just the `#[use_proc]` portion of the above:

```rust
use macros_crate::make_item_const;
#[doc(hidden)]
use macros_crate::__import_tokens_proc_make_item_const_inner;
```

So what `#[use_proc]` really does is add an extra hidden `use` statement matching your provided use statement, but one that instead imports the hidden `__import_tokens_proc_make_item_const_inner` macro, since it needs to be in scope for `make_item_const` to work.

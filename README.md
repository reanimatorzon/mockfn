# covers #
Lightweight library allowing to mock functions in Rust

## Crate ##
[https://crates.io/crates/covers](https://crates.io/crates/covers)

## Usage ##
```rust
use covers::{mocked, mock};

#[mocked(mock_foo)]
fn foo(name: &str) -> String {
    format!("Response: Foo = {}", name)
}

fn mock_foo(another_name: &str) -> String {
    format!("Response: Mocked(Foo = {})", another_name)
}

#[mocked(module::mock_bar)]
fn bar(name: &str) -> String {
    format!("Response: Bar = {}", name)
}

pub struct Struct {}

mod module {
    use super::*;

    #[mock]
    pub fn mock_bar(name: &str) -> String {
        let original_function_result = _bar(name);
        format!("Response: Mocked({})", original_function_result)
    }

    pub fn yyy(this: Struct, name: &str) -> String {
        format!("Response: Mocked({})", name)
    }
}

impl Struct {
    #[mocked(Struct::mock_baz, scope = impl)]
    fn baz(name: &str) -> String {
        format!("Response: Baz = {}", name)
    }

    fn mock_baz(name: &str) -> String {
        format!("Response: Baz = {}", name)
    }

    #[mocked(module::yyy)]
    fn xxx(self, name: &str) -> String {
        format!("Response: Baz = {}", name)
    }
}
```

## Notes ##

### Use cases ###
* You can mock all types of functions with `#[mocked(mock_fn)]`:
    * inline functions (including ones which are inside modules)
    * struct functions (in this case you need to hint macro with `scope = impl`)
    * struct variant functions (use `this: Struct` or `_self: Struct` as the first argument instead of `self`)
    
* You can manually create and store mock functions:
    * inline
    * in separate modules (including `#[cfg(test)] mod tests {}`)
    * in structs implementation blocks
    
### Keep in mind ###
* `scope = impl` hint is required for static struct functions / methods
* There is no need in adding `scope = impl` struct variant's function, 
  it is set automatically for all functions with the first argument `self`
* Using `#[mock]` for all function created only for testing purposes is recommended 
  for the sake of performance
* Using `#[mock]` is strictly required when we use reference to an original function 
  inside. (Usually it is the same name function prepended by underscore `_`). Otherwise release build could fail.
* You can change a prefix of original function passing `features=["__"]` or `features=["_orig_"]`
  in `[dependencies]` block of `Cargo.toml` for `covers` crate. One underscore is default - `"_"`
 
NB: You can find lots of usage examples [here](https://github.com/reanimatorzon/covers/blob/master/covers_it/src/main.rs) -
in the crate of integration tests.     

 

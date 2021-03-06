//! **The crate stores the implementation of macros**
//!
//! Integration tests are outside in [`covers_it`](https://github.com/reanimatorzon/covers/tree/master/covers_it).
//!
//! @see [https://github.com/dtolnay/proc-macro-hack](https://github.com/dtolnay/proc-macro-hack)

use std::collections::HashMap;

use proc_macro::Delimiter::{Brace, Parenthesis};
use proc_macro::*;

use Stage::*;

#[cfg(all(feature = "__", feature = "_orig_"))]
panic!("only single prefix feature could be provided: '__' or '_orig_'. Note: '_' is default value");

#[cfg(all(not(feature = "__"), not(feature = "_orig_")))]
const ORIGINAL_FUNC_PREFIX: &str = "_";
#[cfg(feature = "__")]
const ORIGINAL_FUNC_PREFIX: &str = "__";
#[cfg(feature = "_orig_")]
const ORIGINAL_FUNC_PREFIX: &str = "_orig_";

#[derive(Clone, Copy)]
enum Stage {
    Start = 0,
    FnIdentFound = 1,
    FnNameFound = 2,
    FnArgsFound = 3,
    FnBodyFound = 4,
}

#[derive(Default)]
struct Params {
    reference: String,
    options: HashMap<String, String>,
}

/// Wraps the function below for calling another mock function
/// named according to the macro's argument when `#[cfg(debug_assertions)]`
/// enabled. Call original or mock function according to `#[cfg(test)]` flag.
///
/// Function signature should be the same as original: arguments, output.
///
/// In most cases you need to pass only the single required argument
/// fully-qualified reference to a mock function.
///
/// There only one exception when you need to hint
/// macro with `scope = impl` when you try to mock
/// static struct method (in `impl` block).
///
/// Usage
/// ======
/// ```
/// use covers::{mocked, mock};
///
/// #[mocked(mock_foo)]
/// fn foo(name: &str) -> String {
///     format!("Response: Foo = {}", name)
/// }
///
/// fn mock_foo(another_name: &str) -> String {
///     format!("Response: Mocked(Foo = {})", another_name)
/// }
///
/// #[mocked(module::mock_bar)]
/// fn bar(name: &str) -> String {
///     format!("Response: Bar = {}", name)
/// }
///
/// pub struct Struct {}
///
/// mod module {
///     use super::*;
///
///     #[mock]
///     pub fn mock_bar(name: &str) -> String {
///         let original_function_result = _bar(name);
///         format!("Response: Mocked({})", original_function_result)
///     }
///
///     pub fn yyy(this: Struct, name: &str) -> String {
///         format!("Response: Mocked({})", name)
///     }
/// }
///
/// impl Struct {
///     #[mocked(Struct::mock_baz, scope = impl)]
///     fn baz(name: &str) -> String {
///         format!("Response: Baz = {}", name)
///     }
///
///     fn mock_baz(name: &str) -> String {
///         format!("Response: Baz = {}", name)
///     }
///
///     #[mocked(module::yyy)]
///     fn xxx(self, name: &str) -> String {
///         format!("Response: Baz = {}", name)
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn mocked(args: TokenStream, input: TokenStream) -> TokenStream {
    if !(cfg!(debug_assertions) || cfg!(test)) {
        return input;
    }

    let args = parse_params(args);

    let mut stage = Start;

    let mut original = vec![];
    let mut signature = vec![];

    let mut fn_orig_name = String::new();
    let mut fn_args_string = String::new();

    // FIXME: dirty hack for 'Self::' prefix to functions inside 'impl' block.
    let mut is_impl_scope = false;

    for token in input {
        match &token {
            TokenTree::Ident(ident) if cmp(&stage, FnIdentFound) < 0 && ident.to_string() == "fn" => {
                stage = FnIdentFound;
                signature.push(token.clone());
                original.push(token);
            },
            TokenTree::Ident(ident) if cmp(&stage, FnIdentFound) == 0 => {
                stage = FnNameFound;

                signature.push(create_name_token("", ident));

                let new_token = create_name_token(ORIGINAL_FUNC_PREFIX, ident);
                fn_orig_name = new_token.to_string();
                original.push(new_token);
            },
            TokenTree::Group(group) if cmp(&stage, FnArgsFound) < 0 && group.delimiter() == Parenthesis => {
                stage = FnArgsFound;
                fn_args_string = parse_args(group);
                is_impl_scope = fn_args_string.starts_with("self,") || fn_args_string == "self";
                signature.push(token.clone());
                original.push(token);
            },
            TokenTree::Group(group) if cmp(&stage, FnBodyFound) < 0 && group.delimiter() == Brace => {
                stage = FnBodyFound;
                original.push(token);
            },
            _ => {
                if cmp(&stage, FnBodyFound) < 0 {
                    signature.push(token.clone());
                }
                original.push(token);
            },
        };
    }

    // FIXME: dirty hack for 'Self::' prefix to functions inside 'impl' block.
    is_impl_scope = is_impl_scope || args.options.get("scope").filter(|scope| *scope == "impl").is_some();

    let code = format!(
        r#"
        {fn_original}

        {signature} {{
            #[cfg(test)]
            return {fn_mock_name}{arguments};
            #[cfg(not(test))]
            return {fq}{fn_orig_name}{arguments};
        }}
        "#,
        fn_original = make_public(original.into_iter().collect())
            .into_iter()
            .collect::<TokenStream>(),
        fn_orig_name = fn_orig_name,
        fn_mock_name = args.reference,
        signature = signature.into_iter().collect::<TokenStream>(),
        arguments = format!("({})", fn_args_string),
        fq = if is_impl_scope { "Self::" } else { "" }
    );

    code.parse::<TokenStream>().unwrap().into_iter().collect()
}

/// Marks the following function to be built only for testing purposes
///
/// In other words it is prepended with `#[cfg(any(debug_assertions, test))]`.
///
/// * It is very useful to not compile mock functions for release.
/// * It makes function public - Can be disabled with `features = ["no-pub"]`
/// * It is **strictly** needed when we use reference to original logic of the
///   mocked function.
///
/// Example:
/// ```rust
/// #[mocked(mock_bar)]
/// fn bar(name: &str) -> String {
///     format!("Response: Bar = {}", name)
/// }
///
/// #[mock]
/// pub fn mock_bar(name: &str) -> String {
///     let original_function_result = _bar(name);
///     format!("Response: Mocked({})", original_function_result)
/// }
/// ```
#[proc_macro_attribute]
pub fn mock(_args: TokenStream, input: TokenStream) -> TokenStream {
    if cfg!(debug_assertions) || cfg!(test) {
        if cfg!(feature = "no-pub") {
            input
        } else {
            make_public(input)
        }
    } else {
        TokenStream::new()
    }
}

fn make_public(input: TokenStream) -> TokenStream {
    let mut result = vec![];
    let mut is_public = false;

    let mut iter = input.into_iter();
    while let Some(token) = iter.next() {
        match &token {
            TokenTree::Ident(ident) if ident.to_string() == "pub" => {
                is_public = true;
            },
            TokenTree::Ident(ident) if ident.to_string() == "fn" => {
                if !&is_public {
                    result.push(TokenTree::from(Ident::new("pub", ident.span())));
                }
                // push remaining
                result.push(token.to_owned());
                for token in iter {
                    result.push(token.to_owned());
                }
                break;
            },
            _ => (),
        }
        result.push(token.to_owned());
    }

    result.into_iter().collect()
}

fn parse_params(args: TokenStream) -> Params {
    let params = args.to_string();
    let mut params: Vec<&str> = params.split(',').map(|s| s.trim()).collect();
    assert!(
        !params.is_empty(),
        "At least fully-qualified reference to mock have to be provided!"
    );

    let mut response = Params::default();
    response.reference = params.remove(0).trim().to_string();
    for param in params {
        let entry: Vec<String> = param
            .split('=')
            .map(|s| s.trim().to_lowercase())
            .map(String::from)
            .collect();
        assert!(
            entry.len() == 2,
            "Extra parameters should be provided in `key = value` format!"
        );
        response.options.insert(entry[0].to_owned(), entry[1].to_owned());
    }
    response
}

fn create_name_token(prefix: &str, token: &Ident) -> TokenTree {
    TokenTree::from(Ident::new(&format!("{}{}", prefix, token.to_string()), token.span()))
}

fn parse_args(group: &Group) -> String {
    if group.stream().is_empty() {
        return "".to_string();
    }

    let mut vec = vec![];
    let mut args = vec![];

    for token in group.stream() {
        if let TokenTree::Punct(punct) = &token {
            if punct.to_string() == "," {
                args.push(parse_one_arg(&vec));
                vec.clear();
                continue;
            }
        }
        vec.push(token);
    }
    if !vec.is_empty() {
        args.push(parse_one_arg(&vec));
    }
    args.join(", ")
}

fn parse_one_arg(vec: &[TokenTree]) -> String {
    if vec.iter().last().unwrap().to_string() == "self" {
        "self".to_string()
    } else {
        vec[0].to_string()
    }
}

#[allow(clippy::clone_on_copy)]
fn cmp(current: &Stage, expected: Stage) -> i8 {
    (current.clone() as i8) - (expected as i8)
}

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitInt, Token, parse::{Parse, ParseStream}};

struct SolutionArgs {
    year: u16,
    day: u8,
    part: u8,
}

impl Parse for SolutionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let year: LitInt = input.parse()?;
        input.parse::<Token![,]>()?;
        let day: LitInt = input.parse()?;
        input.parse::<Token![,]>()?;
        let part: LitInt = input.parse()?;
        Ok(SolutionArgs {
            year: year.base10_parse()?,
            day: day.base10_parse()?,
            part: part.base10_parse()?,
        })
    }
}

#[proc_macro_attribute]
#[allow(non_snake_case)]
pub fn Solution(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SolutionArgs);
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let year = args.year;
    let day = args.day;
    let part = args.part;

    quote! {
        pub struct S;

        impl crate::solution::Solution for S {
            fn year(&self) -> u16 { #year }
            fn day(&self) -> u8 { #day }
            fn part(&self) -> u8 { #part }
            fn solve(&self, input: &str) -> String {
                #fn_name(input).to_string()
            }
        }

        inventory::submit!(&S as &dyn crate::solution::Solution);

        #func
    }.into()
}

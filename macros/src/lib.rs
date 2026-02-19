use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Parse, parse_macro_input, LitInt, Token};

struct SolutionArgs {
    year: LitInt,
    day: LitInt,
    part: LitInt,
}

impl Parse for SolutionArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let year = input.parse()?;
        input.parse::<Token![,]>()?;
        let day = input.parse()?;
        input.parse::<Token![,]>()?;
        let part = input.parse()?;
        Ok(SolutionArgs { year, day, part })
    }
}

#[proc_macro_attribute]
pub fn solution(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SolutionArgs);
    let body = parse_macro_input!(item as syn::ItemMod);

    let year = &args.year;
    let day = &args.day;
    let part = &args.part;

    let year_val: u16 = year.base10_parse().unwrap();
    let day_val: u8 = day.base10_parse().unwrap();
    let part_val: u8 = part.base10_parse().unwrap();

    let mod_name = format_ident!("solution_y{}_d{:02}_p{}", year_val, day_val, part_val);

    let content = match body.content {
        Some((_, items)) => items,
        None => {
            return syn::Error::new_spanned(body, "#[solution] module must have inline content")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        mod #mod_name {
            pub struct S;

            impl crate::Solution for S {
                fn year(&self) -> u16 { #year }
                fn day(&self) -> u8 { #day }
                fn part(&self) -> u8 { #part }
                fn solve(&self, input: &str) -> String {
                    solve(input).to_string()
                }
            }

            inventory::submit!(&S as &dyn crate::Solution);

            #(#content)*
        }
    };

    expanded.into()
}
